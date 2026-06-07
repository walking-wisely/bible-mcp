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
    Connection::open(db_path)
        .with_context(|| format!("opening database at {}", db_path.display()))
}

fn floats_to_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_floats(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn collect_id_score(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<Vec<(i64, f64)>> {
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
pub fn rrf_merge(
    vector_hits: &[(i64, f64)],
    fts_hits: &[(i64, f64)],
) -> Vec<(i64, f64)> {
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
        collect_id_score(
            &conn,
            "SELECT ve.rowid, ve.distance
             FROM verse_embeddings ve
             JOIN verses v ON v.id = ve.rowid
             WHERE v.book_num = ?1
               AND ve.embedding MATCH ?2
             ORDER BY ve.distance
             LIMIT ?3",
            params![bnum, blob, lim],
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

pub async fn get_cross_refs(db_path: PathBuf, verse_id: i64, limit: usize) -> Result<Vec<(i64, f64)>> {
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

    #[test]
    fn rrf_merge_correct_order() {
        // id=20 gets highest score (ranks well in both lists)
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
}
