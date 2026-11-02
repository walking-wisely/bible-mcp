use bible_mcp::{ann, db};
use rusqlite::Connection;
use std::path::Path;
use tempfile::TempDir;

const DIM: usize = 16;
const VERSES: usize = 300;

fn vector(seed: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..DIM)
        .map(|i| {
            (((seed * 31 + i * 17) % 97) as f32 / 97.0) - 0.5 + (seed % 7) as f32 * 0.01 * i as f32
        })
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

fn blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn seed_db(dir: &Path) -> std::path::PathBuf {
    db::init_sqlite_vec();
    let path = dir.join("bible.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE verses (id INTEGER PRIMARY KEY, book TEXT NOT NULL, book_num INTEGER NOT NULL,
           chapter INTEGER NOT NULL, verse INTEGER NOT NULL, text TEXT NOT NULL);
         CREATE VIRTUAL TABLE verse_embeddings USING vec0(embedding float[{DIM}]);"
    ))
    .unwrap();
    for id in 1..=VERSES {
        let book_num = (id % 5 + 1) as i64;
        conn.execute(
            "INSERT INTO verses VALUES (?1, 'B', ?2, 1, ?1, 'text')",
            rusqlite::params![id as i64, book_num],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO verse_embeddings (rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![id as i64, blob(&vector(id))],
        )
        .unwrap();
    }
    path
}

fn exact(path: &Path, q: &[f32], limit: usize, book: Option<u32>) -> Vec<i64> {
    let conn = Connection::open(path).unwrap();
    let mut all: Vec<(f64, i64)> = Vec::new();
    for id in 1..=VERSES {
        if book.is_some_and(|b| (id % 5 + 1) as u32 != b) {
            continue;
        }
        let v = vector(id);
        let dot: f32 = v.iter().zip(q).map(|(a, b)| a * b).sum();
        all.push((1.0 - dot as f64, id as i64));
    }
    drop(conn);
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    all.into_iter().take(limit).map(|x| x.1).collect()
}

#[test]
fn no_index_falls_back_to_none() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    assert!(ann::search(&path, &vector(3), 5, None).unwrap().is_none());
}

#[test]
fn indexed_search_matches_exact_top_results() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    assert_eq!(ann::build(&path).unwrap(), VERSES);
    for seed in [3usize, 77, 150, 299] {
        let q = vector(seed);
        let hits = ann::search(&path, &q, 5, None).unwrap().unwrap();
        let ids: Vec<i64> = hits.iter().map(|h| h.0).collect();
        assert_eq!(ids[0], seed as i64);
        let expected = exact(&path, &q, 5, None);
        let overlap = ids.iter().filter(|i| expected.contains(i)).count();
        assert!(overlap >= 4, "seed {seed}: {ids:?} vs {expected:?}");
    }
}

#[test]
fn book_filter_only_returns_that_book() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    ann::build(&path).unwrap();
    let hits = ann::search(&path, &vector(10), 8, Some(3))
        .unwrap()
        .unwrap();
    assert_eq!(hits.len(), 8);
    assert!(hits.iter().all(|(id, _)| (*id as usize % 5 + 1) == 3));
}

#[test]
fn vector_search_sync_uses_index_when_present() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    let before = db::vector_search_sync(&path, &vector(42), 3, None).unwrap();
    ann::build(&path).unwrap();
    let after = db::vector_search_sync(&path, &vector(42), 3, None).unwrap();
    assert_eq!(before[0].0, 42);
    assert_eq!(after[0].0, 42);
}

#[test]
fn index_older_than_database_is_ignored() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    ann::build(&path).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let conn = Connection::open(&path).unwrap();
    conn.execute("UPDATE verses SET text = 'changed' WHERE id = 1", [])
        .unwrap();
    drop(conn);
    assert!(ann::search(&path, &vector(3), 5, None).unwrap().is_none());
}

#[test]
fn dimension_mismatch_falls_back() {
    let dir = TempDir::new().unwrap();
    let path = seed_db(dir.path());
    ann::build(&path).unwrap();
    assert!(ann::search(&path, &[1.0, 0.0], 5, None).unwrap().is_none());
}
