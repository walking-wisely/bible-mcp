use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

// Windows refuses to overwrite a memory-mapped file, which would block `update` while `serve` runs.
const MMAP_BYTES: i64 = if cfg!(windows) { 0 } else { 1 << 30 };

pub(crate) type Fingerprint = (u64, Option<SystemTime>);

struct Cached {
    path: PathBuf,
    fingerprint: Fingerprint,
    conn: Connection,
}

thread_local! {
    static CACHED: RefCell<Option<Cached>> = const { RefCell::new(None) };
}

/// A connection borrowed from the calling thread's cache and returned to it
/// on drop. Tokio's blocking threads are long-lived, so each one keeps a warm
/// connection (and its prepared statements) instead of reopening per call.
pub(crate) struct PooledConn {
    path: PathBuf,
    fingerprint: Fingerprint,
    conn: Option<Connection>,
}

impl Deref for PooledConn {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn.as_ref().expect("connection present until drop")
    }
}

impl Drop for PooledConn {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let entry = Cached {
                path: std::mem::take(&mut self.path),
                fingerprint: self.fingerprint,
                conn,
            };
            let _ = CACHED.try_with(|slot| *slot.borrow_mut() = Some(entry));
        }
    }
}

fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    Ok((meta.len(), meta.modified().ok()))
}

pub(crate) fn open(db_path: &Path) -> Result<PooledConn> {
    let fingerprint = fingerprint(db_path)?;
    let reused = CACHED
        .try_with(|slot| {
            let mut slot = slot.borrow_mut();
            match slot.take() {
                Some(c) if c.path == db_path && c.fingerprint == fingerprint => Some(c.conn),
                _ => None,
            }
        })
        .ok()
        .flatten();
    let conn = match reused {
        Some(conn) => conn,
        None => {
            let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("opening database at {}", db_path.display()))?;
            conn.pragma_update(None, "mmap_size", MMAP_BYTES)?;
            conn
        }
    };
    Ok(PooledConn {
        path: db_path.to_path_buf(),
        fingerprint,
        conn: Some(conn),
    })
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
    let mut stmt = conn.prepare_cached(sql)?;
    let mut out = Vec::new();
    let mut rows = stmt.query(p)?;
    while let Some(row) = rows.next()? {
        out.push((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?));
    }
    Ok(out)
}

fn collect_verses(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<Vec<Verse>> {
    let mut stmt = conn.prepare_cached(sql)?;
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
///
/// Scores are scaled to 0..=1: 1.0 means first in both lists. Ties keep the
/// order of first appearance, so entries from `first` win over `second`.
pub fn rrf_merge(first: &[(i64, f64)], second: &[(i64, f64)]) -> Vec<(i64, f64)> {
    const K: f64 = 60.0;
    let mut merged: Vec<(i64, f64)> = Vec::new();
    let mut index: HashMap<i64, usize> = HashMap::new();
    for list in [first, second] {
        for (rank, &(id, _)) in list.iter().enumerate() {
            let share = K / (rank as f64 + K) / 2.0;
            match index.get(&id) {
                Some(&i) => merged[i].1 += share,
                None => {
                    index.insert(id, merged.len());
                    merged.push((id, share));
                }
            }
        }
    }
    merged.sort_by(|a, b| b.1.total_cmp(&a.1));
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
    if let Some(hits) = crate::ann::search(db_path, embedding, limit, book_num)? {
        return Ok(hits);
    }
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

pub(crate) fn fts_match_expr(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\""))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" OR "))
    }
}

pub fn fts_search_sync(
    db_path: &Path,
    query: &str,
    limit: usize,
    book_num: Option<u32>,
) -> Result<Vec<(i64, f64)>> {
    let Some(query) = fts_match_expr(query) else {
        return Ok(vec![]);
    };
    let query = query.as_str();
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
    let mut stmt =
        conn.prepare_cached("SELECT embedding FROM verse_embeddings WHERE rowid = ?1")?;
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
    fn rrf_merge_scores_are_scaled_to_one() {
        let hits = vec![(1i64, 0.0), (2, 0.0)];
        let both = rrf_merge(&hits, &hits);
        assert!((both[0].1 - 1.0).abs() < 1e-12);
        assert!(both[1].1 < both[0].1);
        let single = rrf_merge(&hits, &[]);
        assert!((single[0].1 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rrf_merge_ties_favour_the_first_list() {
        let merged = rrf_merge(&[(1i64, 0.0)], &[(2i64, 0.0)]);
        assert_eq!(merged[0].0, 1);
    }

    #[test]
    fn rrf_merge_single_list() {
        let vec_hits = vec![(1i64, 0.5), (2, 0.4)];
        let merged = rrf_merge(&vec_hits, &[]);
        assert_eq!(merged[0].0, 1);
        assert_eq!(merged[1].0, 2);
    }
}

#[cfg(test)]
mod fts_expr_tests {
    use super::fts_match_expr;

    #[test]
    fn punctuation_is_stripped_and_words_are_quoted() {
        assert_eq!(
            fts_match_expr("God's love?").as_deref(),
            Some("\"God\" OR \"s\" OR \"love\"")
        );
        assert_eq!(
            fts_match_expr("John 3:16").as_deref(),
            Some("\"John\" OR \"3\" OR \"16\"")
        );
    }

    #[test]
    fn operators_cannot_escape_quoting() {
        assert_eq!(
            fts_match_expr("NOT \"x\" AND (y*)").as_deref(),
            Some("\"NOT\" OR \"x\" OR \"AND\" OR \"y\"")
        );
    }

    #[test]
    fn empty_or_symbol_only_queries_yield_none() {
        assert_eq!(fts_match_expr("  ?!  "), None);
        assert_eq!(fts_match_expr(""), None);
    }
}
