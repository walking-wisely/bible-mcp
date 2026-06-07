# Implementation Plan

## Overview

A self-contained Rust binary that acts as both a CLI setup tool and an MCP stdio server for semantic Bible search. No external runtime dependencies — embeddings run in-process via `fastembed`, the database is downloaded once from Cloudflare R2. Claude Code (or any MCP host) spawns the binary directly.

---

## Prerequisites (runtime)

None. The binary is self-contained. On first run it downloads the pre-built database (~50 MB) and the embedding model (~130 MB, cached locally). Internet access required for first run only.

---

## Stack

| Layer | Technology |
|---|---|
| CLI + MCP server | Rust (single binary) |
| MCP protocol | `rmcp` (official Rust MCP SDK) |
| Async runtime | `tokio` |
| Vector store | SQLite + `sqlite-vec` |
| Full-text search | SQLite FTS5 (built-in) |
| Cross-references | OpenBible.info dataset (~340K curated verse pairs) |
| Hybrid ranking | Reciprocal Rank Fusion (RRF, pure Rust) |
| Embeddings (runtime) | `fastembed-rs` — runs `nomic-embed-text` in-process via ONNX |
| Embeddings (pre-built db) | `nomic-embed-text` via Ollama (run once by maintainer, seed script) |
| Fuzzy matching | `strsim` |
| HTTP + download | `reqwest` |
| Config + serialisation | `serde` + `serde_json` |
| Platform dirs | `dirs` |
| Progress bars | `indicatif` |
| Bible data | World English Bible (WEB), public domain |
| Database hosting | Cloudflare R2 (free tier) |
| Seed script | Python (offline, maintainer only — not shipped) |

---

## Project structure

```
bible-mcp/
├── src/
│   ├── main.rs          # CLI entry point (clap subcommands)
│   ├── mcp_server.rs    # MCP stdio server — search_verses, similar_verses, get_verse, get_passage
│   ├── db.rs            # SQLite connection, KNN search, FTS5, RRF merge, cross-ref lookup
│   ├── books.rs         # canonical book name list + fuzzy matching via strsim
│   ├── download.rs      # R2 download, sha256 verify, cache logic
│   ├── embed.rs         # fastembed wrapper — load model, embed query
│   └── config.rs        # read/write config — ~/.config/bible-mcp/config.json
├── tests/
│   ├── db_tests.rs      # integration tests — real in-memory SQLite
│   ├── books_tests.rs   # unit tests — fuzzy matching, canonical resolution
│   ├── embed_tests.rs   # integration tests — real model, smoke-test output shape
│   └── mcp_tests.rs     # integration tests — MCP tool calls end-to-end
├── seed/                # one-time offline tooling (not shipped)
│   ├── seed.py          # fetch WEB JSON → embed → write SQLite → seed cross-refs → upload R2
│   └── requirements.txt # sentence-transformers, ollama, sqlite-vec, requests, tqdm, boto3
├── Cargo.toml
├── README.md
├── PLAN.md
└── LICENSE.md
```

---

## Testing approach

All tests follow **AAA** (Arrange / Act / Assert). New behaviour is written test-first (TDD): write the failing test, then the implementation.

### Unit tests (`src/*.rs` inline `#[cfg(test)]`)

Fast, no I/O. Cover pure logic:
- `books.rs` — fuzzy match returns correct canonical name, handles variants, returns error on no match
- `db.rs` — RRF merge produces correct ordering given two ranked lists
- `config.rs` — round-trip serialisation, defaults applied correctly

### Integration tests (`tests/`)

Allowed to do I/O, but use fixtures not production data:
- `db_tests.rs` — spin up in-memory SQLite, insert a handful of verses + embeddings, assert KNN and FTS5 results are correct
- `books_tests.rs` — exhaustive variant table (`"Gen"`, `"First Kings"`, `"1st Kings"`, etc.)
- `embed_tests.rs` — load real model, embed a short string, assert output is a `Vec<f32>` of length 768
- `mcp_tests.rs` — drive the MCP server over stdin/stdout with a real (test) database, assert tool responses match expected JSON shape

### What is not tested

- The seed script (Python, one-off, maintainer-only)
- Ollama (not used at runtime)
- Network download (mocked at the `reqwest` boundary)

---

## Phases

### Phase 1 — Seed script (offline, run once by maintainer)

`seed/seed.py` produces `bible-web-nomic.db` and uploads it to Cloudflare R2 with a `manifest.json`:
```json
{ "version": "1.0.0", "sha256": "...", "url": "https://pub-xxx.r2.dev/bible-web-nomic.db" }
```

SQLite schema:
```sql
CREATE TABLE verses (
  id       INTEGER PRIMARY KEY,
  book     TEXT    NOT NULL,
  book_num INTEGER NOT NULL,
  chapter  INTEGER NOT NULL,
  verse    INTEGER NOT NULL,
  text     TEXT    NOT NULL
);

-- vector search
CREATE VIRTUAL TABLE verse_embeddings USING vec0(
  embedding float[768]
);

-- full-text / BM25 search
CREATE VIRTUAL TABLE verses_fts USING fts5(
  text,
  content=verses,
  content_rowid=id
);

-- human-curated cross-references (OpenBible.info, ~340K rows)
CREATE TABLE cross_references (
  from_verse_id  INTEGER NOT NULL REFERENCES verses(id),
  to_verse_id    INTEGER NOT NULL REFERENCES verses(id),
  weight         REAL    NOT NULL,  -- 0.0–1.0 confidence from the dataset
  PRIMARY KEY (from_verse_id, to_verse_id)
);
CREATE INDEX idx_xref_from ON cross_references(from_verse_id);
```

**Seed steps:**
1. Download 66 WEB book JSONs from `TehShrike/world-english-bible`
2. Parse + insert ~31K verse rows into `verses`
3. Batch-embed via `nomic-embed-text`, insert into `verse_embeddings`
4. Populate `verses_fts` (content table — mirrors `verses.text`, no extra storage)
5. Download OpenBible cross-references TSV (`cross_references.txt`)
6. Resolve each `book chapter:verse` reference to a `verses.id`, insert into `cross_references`
7. Compute SHA-256, upload DB + manifest to R2

**Cross-reference source:** `https://a.openbible.info/data/cross-references.zip`
Format: `From verse TAB To verse TAB Votes` (votes normalised to 0–1 weight).

---

### Phase 2 — MCP server (`src/mcp_server.rs`)

#### Tools exposed

```
search_verses(query: str, limit: int = 5, book: str | None = None) → verses
similar_verses(book: str, chapter: int, verse: int, limit: int = 5) → verses
get_verse(book: str, chapter: int, verse: int) → verse
get_passage(book: str, chapter: int, from_verse: int, to_verse: int) → verses
```

`search_verses` and `similar_verses` return a list of:
```json
{ "reference": "John 3:16", "book": "John", "chapter": 3, "verse": 16, "text": "...", "score": 0.91 }
```

`get_verse` and `get_passage` return the same shape without `score` — direct lookup, no ranking.

The `book` field always reflects the resolved canonical name, so the model self-corrects if it passed a variant like "First Kings" and receives back "1 Kings".

#### `search_verses` — hybrid retrieval

1. Embed `query` via `fastembed` (`nomic-embed-text`, in-process)
2. Run KNN vector search → ranked list
3. Run FTS5 BM25 search over `verses_fts` → ranked list
4. Merge with **Reciprocal Rank Fusion**: `score = 1 / (rank + 60)`, summed across both systems
5. If `book` is provided, apply fuzzy match first, then filter both queries by `book_num`
6. Return top `limit` results by merged RRF score

#### `similar_verses` — cross-ref first, vector fallback

1. Resolve `book` via fuzzy match → canonical name + `book_num`
2. Look up `cross_references` for the given verse — high-quality human-curated links
3. Look up the verse's stored embedding, run KNN to find vector neighbours
4. Merge: cross-ref hits first (sorted by `weight` desc), vector-only hits pad the remainder up to `limit`
5. Exclude the input verse from results

This makes `similar_verses` meaningfully different from a second `search_verses` call — it surfaces theologically intentional connections rather than just semantic proximity.

#### `get_verse` / `get_passage` — direct lookup

Simple `SELECT` by `(book_num, chapter, verse)` with fuzzy book matching. No scoring.

#### Book name fuzzy matching (`src/books.rs`)

- 66 canonical names (e.g. `"1 Kings"`, `"Song of Solomon"`)
- `strsim::jaro_winkler` across all candidates, take best match above threshold
- On no match: `"Unknown book: '{book}'. Try a standard Bible book name."`
- Resolved canonical name always echoed in response

#### Implementation details

- [x] Wire `rmcp` with stdio transport, register four tools
- [x] `db.rs` — async SQLite via `tokio::task::spawn_blocking` for all blocking calls
- [x] `embed.rs` — lazy-init `fastembed::TextEmbedding` behind `Mutex`, download model on first call
- [x] RRF merge in pure Rust
- [x] Cross-ref + vector merge logic in `db.rs`
- [x] `get_verse` / `get_passage` — direct lookups by `(book_num, chapter, verse)`
- [ ] `mcp_tests.rs` — end-to-end tool call tests over stdin/stdout with an in-memory DB

---

### Phase 3 — CLI (`src/main.rs`)

| Command | Description | Status |
|---|---|---|
| `bible-mcp setup` | First-run wizard: download embedding model + DB, write Claude Code MCP config | ✅ scaffolded |
| `bible-mcp serve` | Start the MCP stdio server (what Claude Code invokes) | ✅ done |
| `bible-mcp status` | Show config, DB path, embedding model cache status | ✅ done |
| `bible-mcp update` | Re-check manifest and re-download DB if version changed | ✅ done |

**`bible-mcp setup` flow:**
1. Download `bible-web-nomic.db` from R2 with progress bar → `~/.local/share/bible-mcp/bible.db`
2. Trigger `fastembed` model download → `~/.cache/fastembed/` (if not already cached)
3. Write `~/.config/bible-mcp/config.json`
4. Write MCP server entry to Claude Code config:
   ```json
   {
     "mcpServers": {
       "bible": {
         "command": "bible-mcp",
         "args": ["serve"]
       }
     }
   }
   ```

**Outstanding — `setup` on Windows:**
- `fastembed = { version = "5", features = ["ort-load-dynamic"] }` is used so the binary can
  build on `x86_64-pc-windows-gnu`. At runtime the `onnxruntime.dll` must be on `PATH` or
  `ORT_DYLIB_PATH` must be set.
- [ ] `setup` should auto-download `onnxruntime.dll` from the ORT GitHub releases and place it
  next to the binary (or in `~/.local/share/bible-mcp/`), then set `ORT_DYLIB_PATH` in
  `config.json` so `serve` exports it before spawning the ONNX session.

---

### Phase 4 — Packaging & distribution

- [x] `Cargo.toml` with `[lib]` + `[[bin]] name = "bible-mcp"`
- [ ] GitHub Actions CI — `cargo test` on every push (all three platforms)
- [ ] GitHub Actions release workflow — cross-compile for `x86_64-unknown-linux-musl`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` on tag
- [ ] Upload binaries to GitHub Releases
- [ ] README install instructions: download binary for your platform, run `bible-mcp setup`
- [ ] `setup` bundles / downloads correct `onnxruntime` shared lib per platform

---

## Key decisions

**Rust, single binary.** Target users are non-technical. "Download one file, run it" is the only acceptable install story. No Python runtime, no Ollama, no pip.

**`fastembed-rs` instead of Ollama.** `fastembed` runs `nomic-embed-text` in-process via ONNX Runtime. Model downloads automatically to `~/.cache/fastembed/` on first use (~130 MB, quantized). Removes the hardest prerequisite.

**Four tools, complete coverage.** `search_verses` for topic/concept queries, `similar_verses` for "more like this," `get_verse` for a single known reference, `get_passage` for a range. All share fuzzy book matching and the same response shape.

**Hybrid search via RRF.** Vector search drifts on exact phrases and proper nouns; BM25 misses synonyms. RRF merges both ranked lists without ML or weight tuning — `1/(rank+60)` is a robust default.

**`similar_verses` blends cross-refs + vector.** Human-curated cross-references (OpenBible) are high-precision but sparse. Vector KNN fills the gaps. Cross-ref hits are surfaced first, sorted by confidence weight.

**Book names are fuzzy-matched.** The model doesn't know canonical forms upfront. `strsim::jaro_winkler` handles "Gen", "Genesis", "First Kings", "1st Kings". Resolved name echoed back for self-correction.

**Human-readable API contract.** All tools take `(book, chapter, verse)` not opaque ids. References survive re-seeds.

**Embedding model fixed at seed time (`nomic-embed-text`).** Query-time embeddings must match. v1 always uses `nomic-embed-text`.

**Database downloaded, not bundled.** Cached at `~/.local/share/bible-mcp/bible.db`, version-checked on `setup`/`update`.

**Seed script stays Python.** It runs once, offline, by the maintainer. No reason to port it.

**TDD + AAA throughout.** Every new behaviour gets a failing test first. Tests are the executable specification.

---

## Rust dependencies (`Cargo.toml`)

```toml
[dependencies]
tokio        = { version = "1", features = ["full"] }
rmcp         = { version = "0.1", features = ["server", "transport-io"] }
rusqlite     = { version = "0.31", features = ["bundled"] }
sqlite-vec   = "0.1"
# fastembed v5 + ort-load-dynamic: loads onnxruntime.dll at runtime so the
# binary can be built on any host (including Windows GNU) without linking
# against MSVC-only ONNX Runtime static libs.
fastembed    = { version = "5", features = ["ort-load-dynamic"] }
reqwest      = { version = "0.12", features = ["stream"] }
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
clap         = { version = "4", features = ["derive"] }
indicatif    = "0.17"
strsim       = "0.11"
dirs         = "5"
sha2         = "0.10"
anyhow       = "1"
```

---

## Out of scope (v1)

- Multiple Bible translations
- User-supplied Bible data
- Cloud sync / remote MCP transport
- Switching embedding models (requires re-seeding ~31K verses)
- GUI / system tray
- Clustering / thematic grouping
- Reading plan generation
