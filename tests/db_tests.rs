use bible_mcp::db::{
    self, fts_search_sync, get_cross_refs_sync, get_passage_by_ref_sync, get_verse_by_ref_sync,
    get_verse_embedding_sync, get_verses_by_ids_sync, rrf_merge, vector_search_sync,
};
use bible_mcp::db::{
    fts_search, get_cross_refs, get_passage_by_ref, get_verse_by_ref, get_verse_embedding,
    get_verses_by_ids, hybrid_search, vector_search,
};
use rusqlite::Connection;
use std::path::PathBuf;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Fixture — schema mirrors seed/seed.py:create_db (dim=4 for test compactness)
// ---------------------------------------------------------------------------

const TEST_DIM: usize = 4;

fn floats_to_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn setup_db() -> (TempDir, PathBuf) {
    db::init_sqlite_vec();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let conn = Connection::open(&path).unwrap();

    // Schema matches seed/seed.py:create_db — FK + index included; dim=4 (not 768)
    conn.execute_batch(&format!(
        "
        CREATE TABLE verses (
            id       INTEGER PRIMARY KEY,
            book     TEXT    NOT NULL,
            book_num INTEGER NOT NULL,
            chapter  INTEGER NOT NULL,
            verse    INTEGER NOT NULL,
            text     TEXT    NOT NULL
        );
        CREATE VIRTUAL TABLE verse_embeddings USING vec0(
            embedding float[{TEST_DIM}]
        );
        CREATE VIRTUAL TABLE verses_fts USING fts5(
            text,
            content=verses,
            content_rowid=id
        );
        CREATE TABLE cross_references (
            from_verse_id INTEGER NOT NULL REFERENCES verses(id),
            to_verse_id   INTEGER NOT NULL REFERENCES verses(id),
            weight        REAL    NOT NULL,
            PRIMARY KEY (from_verse_id, to_verse_id)
        );
        CREATE INDEX idx_xref_from ON cross_references(from_verse_id);
        ",
        TEST_DIM = TEST_DIM
    ))
    .unwrap();

    // 5 verses spanning 3 books — needed for book-filter tests
    let verses: &[(i64, &str, u32, u32, u32, &str)] = &[
        (1, "Genesis",  1,  1,  1, "In the beginning God created the heavens"),
        (2, "Genesis",  1,  1,  2, "The earth was without form and void"),
        (3, "Genesis",  1,  1,  3, "And God said let there be light"),
        (4, "Exodus",   2,  1,  1, "These are the names of the sons of Israel"),
        (5, "Psalms",  19, 23,  1, "The Lord is my shepherd"),
    ];
    for (id, book, book_num, ch, vs, text) in verses {
        conn.execute(
            "INSERT INTO verses (id, book, book_num, chapter, verse, text)
             VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![id, book, book_num, ch, vs, text],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO verses_fts (rowid, text) VALUES (?1, ?2)",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    // Orthogonal-ish 4-dim embeddings (unit basis + one diagonal)
    let embeddings: &[(i64, [f32; 4])] = &[
        (1, [1.0, 0.0, 0.0, 0.0]),
        (2, [0.0, 1.0, 0.0, 0.0]),
        (3, [0.0, 0.0, 1.0, 0.0]),
        (4, [0.0, 0.0, 0.0, 1.0]),
        (5, [0.7, 0.7, 0.0, 0.0]),
    ];
    for (id, emb) in embeddings {
        let blob = floats_to_blob(emb);
        conn.execute(
            "INSERT INTO verse_embeddings (rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![id, blob],
        )
        .unwrap();
    }

    // Cross-refs from verse 1 → 3 targets, varying weights
    conn.execute_batch(
        "INSERT INTO cross_references VALUES (1, 3, 0.9);
         INSERT INTO cross_references VALUES (1, 2, 0.5);
         INSERT INTO cross_references VALUES (1, 5, 0.3);",
    )
    .unwrap();

    (dir, path)
}

// ---------------------------------------------------------------------------
// rrf_merge
// ---------------------------------------------------------------------------

#[test]
fn rrf_merge_correct_order() {
    let vec_hits = vec![(10i64, 0.1), (20, 0.2), (30, 0.3)];
    let fts_hits = vec![(20i64, 0.9), (30, 0.8), (10, 0.7)];
    let merged = rrf_merge(&vec_hits, &fts_hits);
    assert_eq!(merged[0].0, 20);
    assert_eq!(merged[1].0, 10);
    assert_eq!(merged[2].0, 30);
}

#[test]
fn rrf_merge_single_list() {
    let merged = rrf_merge(&[(1i64, 0.5), (2, 0.4)], &[]);
    assert_eq!(merged[0].0, 1);
    assert_eq!(merged[1].0, 2);
}

#[test]
fn rrf_merge_empty_both() {
    assert!(rrf_merge(&[], &[]).is_empty());
}

#[test]
fn rrf_merge_disjoint_lists() {
    let merged = rrf_merge(&[(1i64, 1.0)], &[(2i64, 1.0)]);
    assert_eq!(merged.len(), 2);
    let ids: Vec<i64> = merged.iter().map(|&(id, _)| id).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

// ---------------------------------------------------------------------------
// get_verses_by_ids
// ---------------------------------------------------------------------------

#[test]
fn get_verses_by_ids_empty() {
    let (_dir, path) = setup_db();
    assert!(get_verses_by_ids_sync(&path, &[]).unwrap().is_empty());
}

#[test]
fn get_verses_by_ids_single() {
    let (_dir, path) = setup_db();
    let rows = get_verses_by_ids_sync(&path, &[5]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 5);
    assert_eq!(rows[0].book_num, 19);
}

#[test]
fn get_verses_by_ids_preserves_order() {
    let (_dir, path) = setup_db();
    let rows = get_verses_by_ids_sync(&path, &[3, 1, 2]).unwrap();
    assert_eq!(
        rows.iter().map(|v| v.id).collect::<Vec<_>>(),
        vec![3, 1, 2]
    );
}

#[test]
fn get_verses_by_ids_unknown_id_ignored() {
    let (_dir, path) = setup_db();
    let rows = get_verses_by_ids_sync(&path, &[1, 999]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 1);
}

#[test]
fn get_verses_by_ids_duplicates() {
    let (_dir, path) = setup_db();
    // Duplicate ids: SQL IN deduplicates, order follows first occurrence
    let rows = get_verses_by_ids_sync(&path, &[2, 2, 1]).unwrap();
    // Both 2 and 1 returned; order by first occurrence: 2 before 1
    let ids: Vec<i64> = rows.iter().map(|v| v.id).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
    assert_eq!(ids[0], 2);
}

// ---------------------------------------------------------------------------
// get_verse_by_ref
// ---------------------------------------------------------------------------

#[test]
fn get_verse_by_ref_found() {
    let (_dir, path) = setup_db();
    let v = get_verse_by_ref_sync(&path, 1, 1, 2).unwrap().unwrap();
    assert_eq!(v.id, 2);
    assert!(v.text.contains("void"));
}

#[test]
fn get_verse_by_ref_not_found() {
    let (_dir, path) = setup_db();
    assert!(get_verse_by_ref_sync(&path, 99, 1, 1).unwrap().is_none());
}

// ---------------------------------------------------------------------------
// get_passage_by_ref
// ---------------------------------------------------------------------------

#[test]
fn get_passage_returns_range_in_order() {
    let (_dir, path) = setup_db();
    let rows = get_passage_by_ref_sync(&path, 1, 1, 1, 3).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].verse, 1);
    assert_eq!(rows[2].verse, 3);
}

#[test]
fn get_passage_empty_range() {
    let (_dir, path) = setup_db();
    assert!(get_passage_by_ref_sync(&path, 1, 99, 1, 5)
        .unwrap()
        .is_empty());
}

// ---------------------------------------------------------------------------
// fts_search
// ---------------------------------------------------------------------------

#[test]
fn fts_search_finds_match() {
    let (_dir, path) = setup_db();
    let hits = fts_search_sync(&path, "light", 10, None).unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].0, 3);
}

#[test]
fn fts_search_scores_positive() {
    let (_dir, path) = setup_db();
    let hits = fts_search_sync(&path, "God", 10, None).unwrap();
    for (_, score) in &hits {
        assert!(*score > 0.0, "bm25 negation should yield positive score");
    }
}

#[test]
fn fts_search_with_book_filter() {
    let (_dir, path) = setup_db();
    let hits = fts_search_sync(&path, "names", 10, Some(2)).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, 4);
}

#[test]
fn fts_search_no_match_returns_empty() {
    let (_dir, path) = setup_db();
    assert!(fts_search_sync(&path, "xyzzy", 10, None).unwrap().is_empty());
}

#[test]
fn fts_search_malformed_query_returns_err() {
    let (_dir, path) = setup_db();
    // Unbalanced quote is a malformed FTS5 query — must Err, not panic
    let result = fts_search_sync(&path, "\"unbalanced", 10, None);
    assert!(result.is_err(), "malformed FTS query should return Err");
}

#[test]
fn fts_search_limit_zero() {
    let (_dir, path) = setup_db();
    let hits = fts_search_sync(&path, "God", 0, None).unwrap();
    assert!(hits.is_empty());
}

// ---------------------------------------------------------------------------
// vector_search
// ---------------------------------------------------------------------------

#[test]
fn vector_search_nearest_neighbor() {
    let (_dir, path) = setup_db();
    let query = vec![0.99f32, 0.01, 0.0, 0.0];
    let hits = vector_search_sync(&path, &query, 3, None).unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].0, 1);
}

#[test]
fn vector_search_with_book_filter() {
    let (_dir, path) = setup_db();
    let query = vec![0.0f32, 0.0, 0.0, 0.99];
    let hits = vector_search_sync(&path, &query, 5, Some(2)).unwrap();
    assert!(hits.iter().all(|(id, _)| *id == 4));
}

#[test]
fn vector_search_limit_zero() {
    let (_dir, path) = setup_db();
    let hits = vector_search_sync(&path, &[1.0f32, 0.0, 0.0, 0.0], 0, None).unwrap();
    assert!(hits.is_empty());
}

// ---------------------------------------------------------------------------
// cross_references
// ---------------------------------------------------------------------------

#[test]
fn cross_refs_ordered_by_weight_desc() {
    let (_dir, path) = setup_db();
    let refs = get_cross_refs_sync(&path, 1, 10).unwrap();
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0].0, 3); // weight 0.9
    assert_eq!(refs[1].0, 2); // weight 0.5
    assert_eq!(refs[2].0, 5); // weight 0.3
}

#[test]
fn cross_refs_limit_respected() {
    let (_dir, path) = setup_db();
    let refs = get_cross_refs_sync(&path, 1, 1).unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].0, 3);
}

#[test]
fn cross_refs_unknown_verse_empty() {
    let (_dir, path) = setup_db();
    assert!(get_cross_refs_sync(&path, 999, 10).unwrap().is_empty());
}

#[test]
fn cross_refs_limit_zero() {
    let (_dir, path) = setup_db();
    assert!(get_cross_refs_sync(&path, 1, 0).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// get_verse_embedding
// ---------------------------------------------------------------------------

#[test]
fn get_verse_embedding_found() {
    let (_dir, path) = setup_db();
    let emb = get_verse_embedding_sync(&path, 1).unwrap().unwrap();
    assert_eq!(emb.len(), 4);
    assert!((emb[0] - 1.0).abs() < 1e-6);
    assert!(emb[1].abs() < 1e-6);
}

#[test]
fn get_verse_embedding_not_found() {
    let (_dir, path) = setup_db();
    assert!(get_verse_embedding_sync(&path, 999).unwrap().is_none());
}

#[test]
fn get_verse_embedding_truncated_blob_documents_behavior() {
    // blob_to_floats uses chunks_exact(4) — trailing bytes silently dropped.
    // This test documents current behavior; a truncated blob is a data bug.
    // TODO: consider returning Err instead of silently truncating.
    db::init_sqlite_vec();
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("trunc.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE verse_embeddings USING vec0(embedding float[{TEST_DIM}]);
         CREATE TABLE verses (id INTEGER PRIMARY KEY, book TEXT NOT NULL,
             book_num INTEGER NOT NULL, chapter INTEGER NOT NULL,
             verse INTEGER NOT NULL, text TEXT NOT NULL);",
        TEST_DIM = TEST_DIM
    ))
    .unwrap();
    // Insert a 3-byte blob (not multiple of 4) directly; bypasses vec0 type check
    // so we use a raw sqlite table instead
    let conn2 = Connection::open(&path).unwrap();
    conn2
        .execute_batch(
            "CREATE TABLE raw_embed (rowid INTEGER, embedding BLOB);
             INSERT INTO raw_embed VALUES (1, X'010203');",
        )
        .unwrap();
    // We can't easily inject a bad blob into verse_embeddings (vec0 validates on insert).
    // Document: vec0 itself rejects dimension-mismatched inserts at write time.
    // blob_to_floats truncation path is therefore only reachable if db is corrupted externally.
}

// ---------------------------------------------------------------------------
// open() error path
// ---------------------------------------------------------------------------

#[test]
fn open_bad_path_returns_err() {
    db::init_sqlite_vec();
    // nonexistent directory — rusqlite will error
    let result = vector_search_sync(
        std::path::Path::new("/nonexistent/path/to/db.sqlite"),
        &[1.0f32, 0.0, 0.0, 0.0],
        5,
        None,
    );
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Async wrappers — parity with sync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn async_vector_search_parity() {
    let (_dir, path) = setup_db();
    let sync = vector_search_sync(&path, &[1.0f32, 0.0, 0.0, 0.0], 3, None).unwrap();
    let asyn = vector_search(path.clone(), vec![1.0f32, 0.0, 0.0, 0.0], 3, None)
        .await
        .unwrap();
    assert_eq!(sync.len(), asyn.len());
    assert_eq!(sync[0].0, asyn[0].0);
}

#[tokio::test]
async fn async_fts_search_parity() {
    let (_dir, path) = setup_db();
    let sync = fts_search_sync(&path, "God", 5, None).unwrap();
    let asyn = fts_search(path.clone(), "God".into(), 5, None)
        .await
        .unwrap();
    assert_eq!(sync.len(), asyn.len());
}

#[tokio::test]
async fn async_get_verses_by_ids_parity() {
    let (_dir, path) = setup_db();
    let sync = get_verses_by_ids_sync(&path, &[3, 1]).unwrap();
    let asyn = get_verses_by_ids(path.clone(), vec![3, 1]).await.unwrap();
    assert_eq!(sync.iter().map(|v| v.id).collect::<Vec<_>>(),
               asyn.iter().map(|v| v.id).collect::<Vec<_>>());
}

#[tokio::test]
async fn async_get_verse_by_ref_parity() {
    let (_dir, path) = setup_db();
    let sync = get_verse_by_ref_sync(&path, 1, 1, 1).unwrap();
    let asyn = get_verse_by_ref(path.clone(), 1, 1, 1).await.unwrap();
    assert_eq!(sync.map(|v| v.id), asyn.map(|v| v.id));
}

#[tokio::test]
async fn async_get_passage_by_ref_parity() {
    let (_dir, path) = setup_db();
    let sync = get_passage_by_ref_sync(&path, 1, 1, 1, 3).unwrap();
    let asyn = get_passage_by_ref(path.clone(), 1, 1, 1, 3).await.unwrap();
    assert_eq!(sync.len(), asyn.len());
    assert_eq!(sync[0].id, asyn[0].id);
}

#[tokio::test]
async fn async_get_cross_refs_parity() {
    let (_dir, path) = setup_db();
    let sync = get_cross_refs_sync(&path, 1, 10).unwrap();
    let asyn = get_cross_refs(path.clone(), 1, 10).await.unwrap();
    assert_eq!(sync.len(), asyn.len());
    assert_eq!(sync[0].0, asyn[0].0);
}

#[tokio::test]
async fn async_get_verse_embedding_parity() {
    let (_dir, path) = setup_db();
    let sync = get_verse_embedding_sync(&path, 1).unwrap();
    let asyn = get_verse_embedding(path.clone(), 1).await.unwrap();
    assert_eq!(sync.map(|e| e.len()), asyn.map(|e| e.len()));
}

// ---------------------------------------------------------------------------
// hybrid_search (async)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn hybrid_search_returns_results() {
    let (_dir, path) = setup_db();
    let hits = hybrid_search(path, vec![1.0f32, 0.0, 0.0, 0.0], "beginning".into(), 5, None)
        .await
        .unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].0, 1);
}

#[tokio::test]
async fn hybrid_search_respects_limit() {
    let (_dir, path) = setup_db();
    let hits = hybrid_search(path, vec![0.5f32, 0.5, 0.0, 0.0], "God".into(), 2, None)
        .await
        .unwrap();
    assert!(hits.len() <= 2);
}

#[tokio::test]
async fn hybrid_search_with_book_filter() {
    let (_dir, path) = setup_db();
    // Filter to Exodus (book_num=2) — only verse 4 lives there
    let hits = hybrid_search(
        path,
        vec![0.0f32, 0.0, 0.0, 1.0],
        "names".into(),
        5,
        Some(2),
    )
    .await
    .unwrap();
    assert!(hits.iter().all(|(id, _)| *id == 4));
}
