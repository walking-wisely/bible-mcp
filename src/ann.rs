use crate::db;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

const SCALAR: ScalarKind = ScalarKind::F16;
const CONNECTIVITY: usize = 16;
const EXPANSION_ADD: usize = 128;
const EXPANSION_SEARCH: usize = 128;

struct Loaded {
    path: PathBuf,
    index_fingerprint: db::Fingerprint,
    index: Index,
    books: HashMap<u64, u32>,
}

static LOADED: Mutex<Option<Arc<Loaded>>> = Mutex::new(None);

pub fn index_path(db_path: &Path) -> PathBuf {
    let mut name = db_path.as_os_str().to_owned();
    name.push(".usearch");
    PathBuf::from(name)
}

fn disabled() -> bool {
    std::env::var("BIBLE_MCP_ANN").is_ok_and(|v| v == "off")
}

fn options(dimensions: usize) -> IndexOptions {
    IndexOptions {
        dimensions,
        metric: MetricKind::Cos,
        quantization: SCALAR,
        connectivity: CONNECTIVITY,
        expansion_add: EXPANSION_ADD,
        expansion_search: EXPANSION_SEARCH,
        multi: false,
    }
}

/// Builds the ANN index next to the database and returns the number of vectors indexed.
pub fn build(db_path: &Path) -> Result<usize> {
    let conn = db::open(db_path)?;
    let mut stmt = conn.prepare("SELECT rowid, embedding FROM verse_embeddings ORDER BY rowid")?;
    let mut rows = stmt.query([])?;
    let mut index: Option<Index> = None;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let vector: Vec<f32> = blob
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        if index.is_none() {
            let created = Index::new(&options(vector.len())).map_err(|e| anyhow!("{e}"))?;
            created.reserve(1024).map_err(|e| anyhow!("{e}"))?;
            index = Some(created);
        }
        let index = index.as_ref().expect("created above");
        if index.size() == index.capacity() {
            index
                .reserve(index.capacity() * 2)
                .map_err(|e| anyhow!("{e}"))?;
        }
        index.add(id as u64, &vector).map_err(|e| anyhow!("{e}"))?;
        count += 1;
    }
    let index = index.context("verse_embeddings is empty; nothing to index")?;

    let target = index_path(db_path);
    let mut tmp_name = target.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    index
        .save(tmp.to_str().context("index path is not valid UTF-8")?)
        .map_err(|e| anyhow!("{e}"))?;
    std::fs::rename(&tmp, &target)?;
    Ok(count)
}

fn load(db_path: &Path, path: &Path, fingerprint: db::Fingerprint) -> Result<Loaded> {
    let probe = Index::new(&options(1)).map_err(|e| anyhow!("{e}"))?;
    let path_str = path.to_str().context("index path is not valid UTF-8")?;
    probe.view(path_str).map_err(|e| anyhow!("{e}"))?;
    let index = Index::new(&options(probe.dimensions())).map_err(|e| anyhow!("{e}"))?;
    drop(probe);
    // Windows cannot replace a memory-mapped file, so rebuilding the index would fail while serving.
    if cfg!(windows) {
        index.load(path_str).map_err(|e| anyhow!("{e}"))?;
    } else {
        index.view(path_str).map_err(|e| anyhow!("{e}"))?;
    }
    index.change_expansion_search(EXPANSION_SEARCH);

    let conn = db::open(db_path)?;
    let mut stmt = conn.prepare("SELECT id, book_num FROM verses")?;
    let books = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, u32>(1)?))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    Ok(Loaded {
        path: path.to_path_buf(),
        index_fingerprint: fingerprint,
        index,
        books,
    })
}

fn current(db_path: &Path) -> Result<Option<Arc<Loaded>>> {
    let path = index_path(db_path);
    let Ok(index_meta) = std::fs::metadata(&path) else {
        return Ok(None);
    };
    let db_meta = std::fs::metadata(db_path)?;
    let (Ok(index_time), Ok(db_time)) = (index_meta.modified(), db_meta.modified()) else {
        return Ok(None);
    };
    if index_time < db_time {
        return Ok(None);
    }
    let fingerprint = (index_meta.len(), Some(index_time));

    let mut slot = LOADED.lock().map_err(|_| anyhow!("ann cache poisoned"))?;
    if let Some(loaded) = slot.as_ref() {
        if loaded.path == path && loaded.index_fingerprint == fingerprint {
            return Ok(Some(loaded.clone()));
        }
    }
    let loaded = Arc::new(load(db_path, &path, fingerprint)?);
    *slot = Some(loaded.clone());
    Ok(Some(loaded))
}

/// Returns `None` when no usable index exists, so callers fall back to exact search.
pub fn search(
    db_path: &Path,
    embedding: &[f32],
    limit: usize,
    book_num: Option<u32>,
) -> Result<Option<Vec<(i64, f64)>>> {
    if disabled() {
        return Ok(None);
    }
    let Some(loaded) = current(db_path)? else {
        return Ok(None);
    };
    if embedding.len() != loaded.index.dimensions() {
        return Ok(None);
    }
    let matches = match book_num {
        Some(book) => loaded.index.filtered_search(embedding, limit, |key| {
            loaded.books.get(&key) == Some(&book)
        }),
        None => loaded.index.search(embedding, limit),
    }
    .map_err(|e| anyhow!("{e}"))?;
    Ok(Some(
        matches
            .keys
            .into_iter()
            .zip(matches.distances)
            .map(|(key, distance)| (key as i64, distance as f64))
            .collect(),
    ))
}
