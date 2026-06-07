use bible_mcp::db::{
    self, fts_search_sync, get_passage_by_ref_sync, get_verse_by_ref_sync, get_verses_by_ids_sync,
    rrf_merge, vector_search_sync,
};
use rusqlite::Connection;
use tempfile::NamedTempFile;

fn seed_db() -> NamedTempFile {
    db::init_sqlite_vec();
    let file = NamedTempFile::new().unwrap();
    let conn = Connection::open(file.path()).unwrap();

    conn.execute_batch(
        "
        CREATE TABLE verses (
          id INTEGER PRIMARY KEY,
          book TEXT NOT NULL,
          book_num INTEGER NOT NULL,
          chapter INTEGER NOT NULL,
          verse INTEGER NOT NULL,
          text TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE verse_embeddings USING vec0(embedding float[4]);
        CREATE VIRTUAL TABLE verses_fts USING fts5(text, content=verses, content_rowid=id);
        CREATE TABLE cross_references (
          from_verse_id INTEGER NOT NULL,
          to_verse_id INTEGER NOT NULL,
          weight REAL NOT NULL,
          PRIMARY KEY (from_verse_id, to_verse_id)
        );
        ",
    )
    .unwrap();

    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (1, 'John', 43, 3, 16, 'For God so loved the world')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (2, 'Romans', 45, 8, 28, 'All things work together for good')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (3, 'John', 43, 3, 17, 'For God sent his Son')",
        [],
    ).unwrap();

    // tiny 4-dim fake embeddings so we can test KNN
    let e1: Vec<u8> = [1.0f32, 0.0, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    let e2: Vec<u8> = [0.0f32, 1.0, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    let e3: Vec<u8> = [0.9f32, 0.1, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (1, ?1)",
        rusqlite::params![e1],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (2, ?1)",
        rusqlite::params![e2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (3, ?1)",
        rusqlite::params![e3],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (1, 'For God so loved the world')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (2, 'All things work together for good')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (3, 'For God sent his Son')",
        [],
    )
    .unwrap();

    file
}

#[test]
fn rrf_merge_produces_correct_order() {
    let vec_hits = vec![(10i64, 0.1), (20, 0.2), (30, 0.3)];
    let fts_hits = vec![(20i64, 0.9), (30, 0.8), (10, 0.7)];
    let merged = rrf_merge(&vec_hits, &fts_hits);
    assert_eq!(merged[0].0, 20);
    assert_eq!(merged[1].0, 10);
    assert_eq!(merged[2].0, 30);
}

#[test]
fn fts_search_returns_hits() {
    let db = seed_db();
    let hits = fts_search_sync(db.path(), "God", 5, None).unwrap();
    assert!(!hits.is_empty(), "expected FTS hits for 'God'");
    let ids: Vec<i64> = hits.iter().map(|&(id, _)| id).collect();
    assert!(ids.contains(&1) || ids.contains(&3));
}

#[test]
fn vector_search_returns_hits() {
    let db = seed_db();
    let query_emb = [1.0f32, 0.0, 0.0, 0.0];
    let hits = vector_search_sync(db.path(), &query_emb, 3, None).unwrap();
    assert!(!hits.is_empty());
    // id=1 should be closest to [1,0,0,0]
    assert_eq!(hits[0].0, 1);
}

#[test]
fn get_verse_by_ref_found() {
    let db = seed_db();
    let v = get_verse_by_ref_sync(db.path(), 43, 3, 16)
        .unwrap()
        .unwrap();
    assert_eq!(v.book, "John");
    assert!(v.text.contains("loved"));
}

#[test]
fn get_verse_by_ref_not_found() {
    let db = seed_db();
    let v = get_verse_by_ref_sync(db.path(), 43, 99, 99).unwrap();
    assert!(v.is_none());
}

#[test]
fn get_passage_returns_range() {
    let db = seed_db();
    let rows = get_passage_by_ref_sync(db.path(), 43, 3, 16, 17).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].verse, 16);
    assert_eq!(rows[1].verse, 17);
}

#[test]
fn get_verses_by_ids_preserves_order() {
    let db = seed_db();
    let ids = vec![3i64, 1, 2];
    let rows = get_verses_by_ids_sync(db.path(), &ids).unwrap();
    assert_eq!(rows[0].id, 3);
    assert_eq!(rows[1].id, 1);
    assert_eq!(rows[2].id, 2);
}
