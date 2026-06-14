use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Verse {
    pub id: i64,
    pub book: String,
    pub book_num: u32,
    pub chapter: u32,
    pub verse: u32,
    pub text: String,
}

/// Call once at process startup to register the sqlite-vec extension globally.
pub fn init_sqlite_vec() {
    use rusqlite::ffi::sqlite3_auto_extension;
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
}

fn open(db_path: &Path) -> Result<Connection> {
    Connection::open(db_path).with_context(|| format!("opening database at {}", db_path.display()))
}

fn floats_to_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_floats(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn collect_id_score(
    conn: &Connection,
    sql: &str,
    p: impl rusqlite::Params,
) -> Result<Vec<(i64, f64)>> {
    let mut stmt = conn.prepare(sql)?;
    let mut out = Vec::new();
    let mut rows = stmt.query(p)?;
    while let Some(row) = rows.next()? {
        out.push((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?));
    }
    Ok(out)
}

fn collect_verses(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<Vec<Verse>> {
    let mut stmt = conn.prepare(sql)?;
    let mut out = Vec::new();
    let mut rows = stmt.query(p)?;
    while let Some(row) = rows.next()? {
        out.push(Verse {
            id: row.get(0)?,
            book: row.get(1)?,
            book_num: row.get(2)?,
            chapter: row.get(3)?,
            verse: row.get(4)?,
            text: row.get(5)?,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// RRF merge (pure Rust, no I/O)
// ---------------------------------------------------------------------------

/// Merge two ranked lists via Reciprocal Rank Fusion (k=60).
pub fn rrf_merge(vector_hits: &[(i64, f64)], fts_hits: &[(i64, f64)]) -> Vec<(i64, f64)> {
    const K: f64 = 60.0;
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for (rank, &(id, _)) in vector_hits.iter().enumerate() {
        *scores.entry(id).or_default() += 1.0 / (rank as f64 + K);
    }
    for (rank, &(id, _)) in fts_hits.iter().enumerate() {
        *scores.entry(id).or_default() += 1.0 / (rank as f64 + K);
    }
    let mut merged: Vec<(i64, f64)> = scores.into_iter().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    merged
}

// ---------------------------------------------------------------------------
// Blocking DB helpers
// ---------------------------------------------------------------------------

pub fn vector_search_sync(
    db_path: &Path,
    embedding: &[f32],
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    let conn = open(db_path)?;
    let blob = floats_to_blob(embedding);
    let lim = limit as i64;
    if let Some(bnum) = book_num {
        let k = (limit.max(1) * 20) as i64;
        collect_id_score(
            &conn,
            "SELECT candidate.rowid, candidate.distance
             FROM (
               SELECT rowid, distance
               FROM verse_embeddings
               WHERE embedding MATCH ?1
                 AND k = ?2
             ) candidate
             JOIN verses v ON v.id = candidate.rowid
             WHERE v.book_num = ?3
             ORDER BY candidate.distance
             LIMIT ?4",
            params![blob, k, bnum, lim],
        )
    } else {
        collect_id_score(
            &conn,
            "SELECT rowid, distance
             FROM verse_embeddings
             WHERE embedding MATCH ?1
             ORDER BY distance
             LIMIT ?2",
            params![blob, lim],
        )
    }
}

pub fn fts_search_sync(
    db_path: &Path,
    query: &str,
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    let conn = open(db_path)?;
    let lim = limit as i64;
    let raw: Vec<(i64, f64)> = if let Some(bnum) = book_num {
        collect_id_score(
            &conn,
            "SELECT vf.rowid, bm25(verses_fts) AS score
             FROM verses_fts vf
             JOIN verses v ON v.id = vf.rowid
             WHERE verses_fts MATCH ?1
               AND v.book_num = ?2
             ORDER BY score
             LIMIT ?3",
            params![query, bnum, lim],
        )?
    } else {
        collect_id_score(
            &conn,
            "SELECT rowid, bm25(verses_fts) AS score
             FROM verses_fts
             WHERE verses_fts MATCH ?1
             ORDER BY score
             LIMIT ?2",
            params![query, lim],
        )?
    };
    // bm25() returns negative values — negate so higher = better
    Ok(raw.into_iter().map(|(id, s)| (id, -s)).collect())
}

pub fn get_verses_by_ids_sync(db_path: &Path, ids: &[i64]) -> Result<Vec<Verse>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let conn = open(db_path)?;
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, book, book_num, chapter, verse, text FROM verses WHERE id IN ({})",
        placeholders
    );
    let mut rows = collect_verses(&conn, &sql, rusqlite::params_from_iter(ids.iter()))?;

    let order: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    rows.sort_by_key(|v| order.get(&v.id).copied().unwrap_or(usize::MAX));
    Ok(rows)
}

pub fn get_verse_by_ref_sync(
    db_path: &Path,
    book_num: u32,
    chapter: u32,
    verse: u32,
) -> Result<Option<Verse>> {
    let conn = open(db_path)?;
    let rows = collect_verses(
        &conn,
        "SELECT id, book, book_num, chapter, verse, text FROM verses
         WHERE book_num = ?1 AND chapter = ?2 AND verse = ?3",
        params![book_num, chapter, verse],
    )?;
    Ok(rows.into_iter().next())
}

pub fn get_passage_by_ref_sync(
    db_path: &Path,
    book_num: u32,
    chapter: u32,
    from_verse: u32,
    to_verse: u32,
) -> Result<Vec<Verse>> {
    let conn = open(db_path)?;
    collect_verses(
        &conn,
        "SELECT id, book, book_num, chapter, verse, text FROM verses
         WHERE book_num = ?1 AND chapter = ?2 AND verse BETWEEN ?3 AND ?4
         ORDER BY verse",
        params![book_num, chapter, from_verse, to_verse],
    )
}

pub fn get_cross_refs_sync(db_path: &Path, verse_id: i64, limit: usize) -> Result<Vec<(i64, f64)>> {
    let conn = open(db_path)?;
    collect_id_score(
        &conn,
        "SELECT to_verse_id, weight FROM cross_references
         WHERE from_verse_id = ?1 ORDER BY weight DESC LIMIT ?2",
        params![verse_id, limit as i64],
    )
}

pub fn get_verse_embedding_sync(db_path: &Path, verse_id: i64) -> Result<Option<Vec<f32>>> {
    let conn = open(db_path)?;
    let mut stmt = conn.prepare("SELECT embedding FROM verse_embeddings WHERE rowid = ?1")?;
    let mut rows = stmt.query(params![verse_id])?;
    match rows.next()? {
        Some(row) => {
            let blob: Vec<u8> = row.get(0)?;
            Ok(Some(blob_to_floats(&blob)))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Async wrappers (spawn_blocking)
// ---------------------------------------------------------------------------

macro_rules! blocking {
    ($($body:tt)*) => {
        tokio::task::spawn_blocking(move || { $($body)* })
            .await
            .map_err(|e| anyhow::anyhow!("join error: {}", e))?
    };
}

pub async fn vector_search(
    db_path: PathBuf,
    embedding: Vec<f32>,
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    blocking!(vector_search_sync(&db_path, &embedding, limit, book_num))
}

pub async fn fts_search(
    db_path: PathBuf,
    query: String,
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    blocking!(fts_search_sync(&db_path, &query, limit, book_num))
}

pub async fn hybrid_search(
    db_path: PathBuf,
    embedding: Vec<f32>,
    query: String,
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    let fetch = limit * 2;
    let db1 = db_path.clone();
    let emb = embedding.clone();
    let q = query.clone();
    let (vec_hits, fts_hits) = tokio::join!(
        vector_search(db1, emb, fetch, book_num),
        fts_search(db_path, q, fetch, book_num),
    );
    let merged = rrf_merge(&vec_hits?, &fts_hits?);
    Ok(merged.into_iter().take(limit).collect())
}

pub async fn get_verses_by_ids(db_path: PathBuf, ids: Vec<i64>) -> Result<Vec<Verse>> {
    blocking!(get_verses_by_ids_sync(&db_path, &ids))
}

pub async fn get_verse_by_ref(
    db_path: PathBuf,
    book_num: u32,
    chapter: u32,
    verse: u32,
) -> Result<Option<Verse>> {
    blocking!(get_verse_by_ref_sync(&db_path, book_num, chapter, verse))
}

pub async fn get_passage_by_ref(
    db_path: PathBuf,
    book_num: u32,
    chapter: u32,
    from_verse: u32,
    to_verse: u32,
) -> Result<Vec<Verse>> {
    blocking!(get_passage_by_ref_sync(
        &db_path, book_num, chapter, from_verse, to_verse
    ))
}

pub async fn get_cross_refs(
    db_path: PathBuf,
    verse_id: i64,
    limit: usize,
) -> Result<Vec<(i64, f64)>> {
    blocking!(get_cross_refs_sync(&db_path, verse_id, limit))
}

pub async fn get_verse_embedding(db_path: PathBuf, verse_id: i64) -> Result<Option<Vec<f32>>> {
    blocking!(get_verse_embedding_sync(&db_path, verse_id))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ---------------------------------------------------------------------------
    // Fixture
    // ---------------------------------------------------------------------------

    fn setup_db() -> (TempDir, PathBuf) {
        init_sqlite_vec();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let conn = Connection::open(&path).unwrap();

        conn.execute_batch(
            "CREATE TABLE verses (
                id       INTEGER PRIMARY KEY,
                book     TEXT NOT NULL,
                book_num INTEGER NOT NULL,
                chapter  INTEGER NOT NULL,
                verse    INTEGER NOT NULL,
                text     TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE verses_fts USING fts5(text, content='verses', content_rowid='id');
            CREATE TABLE cross_references (
                from_verse_id INTEGER NOT NULL,
                to_verse_id   INTEGER NOT NULL,
                weight        REAL NOT NULL
            );",
        )
        .unwrap();

        // sqlite-vec virtual table
        conn.execute_batch(
            "CREATE VIRTUAL TABLE verse_embeddings USING vec0(
                embedding FLOAT[4]
            );",
        )
        .unwrap();

        // Seed verses
        let verses: &[(i64, &str, u32, u32, u32, &str)] = &[
            (1, "Genesis",  1, 1, 1, "In the beginning God created the heavens"),
            (2, "Genesis",  1, 1, 2, "The earth was without form and void"),
            (3, "Genesis",  1, 1, 3, "And God said let there be light"),
            (4, "Exodus",   2, 1, 1, "These are the names of the sons of Israel"),
            (5, "Psalms",  19, 23, 1, "The Lord is my shepherd"),
        ];
        for (id, book, book_num, ch, vs, text) in verses {
            conn.execute(
                "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (?1,?2,?3,?4,?5,?6)",
                params![id, book, book_num, ch, vs, text],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO verses_fts (rowid, text) VALUES (?1, ?2)",
                params![id, text],
            )
            .unwrap();
        }

        // Seed embeddings — simple orthogonal-ish unit-ish vectors
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
                params![id, blob],
            )
            .unwrap();
        }

        // Seed cross-references
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
        let vec_hits = vec![(1i64, 0.5), (2, 0.4)];
        let merged = rrf_merge(&vec_hits, &[]);
        assert_eq!(merged[0].0, 1);
        assert_eq!(merged[1].0, 2);
    }

    #[test]
    fn rrf_merge_empty_both() {
        assert!(rrf_merge(&[], &[]).is_empty());
    }

    #[test]
    fn rrf_merge_disjoint_lists() {
        // id=1 rank-0 in vec, id=2 rank-0 in fts → equal scores
        let merged = rrf_merge(&[(1i64, 1.0)], &[(2i64, 1.0)]);
        assert_eq!(merged.len(), 2);
        // scores identical; both present
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
        let result = get_verses_by_ids_sync(&path, &[]).unwrap();
        assert!(result.is_empty());
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
        // Request in reverse order — should come back in requested order
        let rows = get_verses_by_ids_sync(&path, &[3, 1, 2]).unwrap();
        assert_eq!(rows.iter().map(|v| v.id).collect::<Vec<_>>(), vec![3, 1, 2]);
    }

    #[test]
    fn get_verses_by_ids_unknown_id_ignored() {
        let (_dir, path) = setup_db();
        let rows = get_verses_by_ids_sync(&path, &[1, 999]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
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
        let rows = get_passage_by_ref_sync(&path, 1, 99, 1, 5).unwrap();
        assert!(rows.is_empty());
    }

    // ---------------------------------------------------------------------------
    // fts_search
    // ---------------------------------------------------------------------------

    #[test]
    fn fts_search_finds_match() {
        let (_dir, path) = setup_db();
        let hits = fts_search_sync(&path, "light", 10, None).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0, 3); // "let there be light" → verse id 3
    }

    #[test]
    fn fts_search_scores_positive() {
        let (_dir, path) = setup_db();
        let hits = fts_search_sync(&path, "God", 10, None).unwrap();
        // bm25 negated → all scores > 0
        for (_, score) in &hits {
            assert!(*score > 0.0, "score should be positive after negation");
        }
    }

    #[test]
    fn fts_search_with_book_filter() {
        let (_dir, path) = setup_db();
        // "God" appears in Genesis (book_num=1) and Psalms (19); filter to Exodus (2)
        let hits = fts_search_sync(&path, "names", 10, Some(2)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 4);
    }

    #[test]
    fn fts_search_no_match_returns_empty() {
        let (_dir, path) = setup_db();
        let hits = fts_search_sync(&path, "xyzzy", 10, None).unwrap();
        assert!(hits.is_empty());
    }

    // ---------------------------------------------------------------------------
    // vector_search
    // ---------------------------------------------------------------------------

    #[test]
    fn vector_search_nearest_neighbor() {
        let (_dir, path) = setup_db();
        // Query close to embedding of verse 1 → [1,0,0,0]
        let query = vec![0.99f32, 0.01, 0.0, 0.0];
        let hits = vector_search_sync(&path, &query, 3, None).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0, 1);
    }

    #[test]
    fn vector_search_with_book_filter() {
        let (_dir, path) = setup_db();
        // verse 1 and 2 and 3 are Genesis (book_num=1); verse 4 is Exodus (2)
        // Query pointing at verse 4's embedding
        let query = vec![0.0f32, 0.0, 0.0, 0.99];
        let hits = vector_search_sync(&path, &query, 5, Some(2)).unwrap();
        // Only Exodus verse (id=4) should appear
        assert!(hits.iter().all(|(id, _)| *id == 4));
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

    // ---------------------------------------------------------------------------
    // verse_embedding
    // ---------------------------------------------------------------------------

    #[test]
    fn get_verse_embedding_found() {
        let (_dir, path) = setup_db();
        let emb = get_verse_embedding_sync(&path, 1).unwrap().unwrap();
        assert_eq!(emb.len(), 4);
        assert!((emb[0] - 1.0).abs() < 1e-6);
        assert!((emb[1]).abs() < 1e-6);
    }

    #[test]
    fn get_verse_embedding_not_found() {
        let (_dir, path) = setup_db();
        assert!(get_verse_embedding_sync(&path, 999).unwrap().is_none());
    }

    // ---------------------------------------------------------------------------
    // hybrid_search (async)
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn hybrid_search_returns_results() {
        let (_dir, path) = setup_db();
        let query = vec![1.0f32, 0.0, 0.0, 0.0];
        let hits = hybrid_search(path, query, "beginning".to_string(), 5, None)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        // verse 1 should rank highly (matches both vector and FTS)
        assert_eq!(hits[0].0, 1);
    }

    #[tokio::test]
    async fn hybrid_search_respects_limit() {
        let (_dir, path) = setup_db();
        let query = vec![0.5f32, 0.5, 0.0, 0.0];
        let hits = hybrid_search(path, query, "God".to_string(), 2, None)
            .await
            .unwrap();
        assert!(hits.len() <= 2);
    }
}
