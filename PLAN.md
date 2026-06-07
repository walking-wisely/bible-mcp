# Implementation Plan

## Overview

A Python CLI tool that manages a local MCP server providing semantic Bible search to AI tools (Claude Code, Codex, etc.). The CLI handles first-run setup (downloading the pre-built database, detecting Ollama, writing the Claude Code MCP config). The MCP server runs as a stdio process — Claude Code spawns it directly.

---

## Stack

| Layer | Technology |
|---|---|
| CLI + MCP server | Python |
| MCP protocol | `mcp` (official Python MCP SDK) |
| Async runtime | `asyncio` (stdlib) |
| Vector store | SQLite + `sqlite-vec` |
| Full-text search | SQLite FTS5 (built-in) |
| Cross-references | OpenBible.info dataset (~340K curated verse pairs) |
| Hybrid ranking | Reciprocal Rank Fusion (RRF, implemented in Python) |
| Embeddings (runtime) | Ollama local HTTP API via `ollama` Python client |
| Embeddings (pre-built db) | `nomic-embed-text` via Ollama (run once by maintainer) |
| Bible data | World English Bible (WEB), public domain |
| Database hosting | Cloudflare R2 (free tier) |

---

## Project structure

```
bible-mcp/
├── bible_mcp/
│   ├── __init__.py
│   ├── cli.py           # CLI entry point — setup, status, serve commands
│   ├── mcp_server.py    # MCP stdio server — search_verses, similar_verses, get_verse, get_passage
│   ├── db.py            # SQLite connection, KNN search, FTS5, RRF merge, cross-ref lookup
│   ├── books.py         # canonical book name list + fuzzy matching via difflib
│   ├── download.py      # R2 download, sha256 verify, cache logic
│   ├── ollama_client.py # list_models(), embed(text, model)
│   └── config.py        # read/write active model, db path — ~/.config/bible-mcp/config.json
├── seed/                # one-time offline tooling (not shipped)
│   ├── seed.py          # fetch WEB JSON → embed → write SQLite → seed cross-refs → upload R2
│   └── requirements.txt # sentence-transformers, ollama, sqlite-vec, requests, tqdm, boto3
├── pyproject.toml
├── README.md
├── PLAN.md
└── LICENSE.md
```

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

### Phase 2 — MCP server (`bible_mcp/mcp_server.py`)

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

1. Embed `query` via Ollama (`nomic-embed-text`)
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

#### Book name fuzzy matching (`bible_mcp/books.py`)

- 66 canonical names (e.g. `"1 Kings"`, `"Song of Solomon"`)
- `difflib.get_close_matches(book, canonical_names, n=1, cutoff=0.6)`
- On no match: `"Unknown book: '{book}'. Try a standard Bible book name."`
- Resolved canonical name always echoed in response

#### Implementation details

- [ ] Set up `mcp` Python SDK with stdio transport
- [ ] `db.py` — async-friendly SQLite wrapper using `asyncio.to_thread` for all blocking calls
- [ ] `ollama_client.py` — `list_models()` and `embed(text, model)` via `ollama` Python client
- [ ] RRF merge in pure Python
- [ ] Cross-ref + vector merge logic in `db.py`
- [ ] `get_verse` / `get_passage` — direct lookups by `(book_num, chapter, verse)`

---

### Phase 3 — CLI (`bible_mcp/cli.py`)

| Command | Description |
|---|---|
| `bible-mcp setup` | First-run wizard: detect Ollama, download DB, write Claude Code MCP config |
| `bible-mcp serve` | Start the MCP stdio server (what Claude Code invokes) |
| `bible-mcp status` | Show config, DB path, Ollama status |
| `bible-mcp update` | Re-check manifest and re-download DB if version changed |

**`bible-mcp setup` flow:**
1. Ping `localhost:11434` — detect Ollama, print link if missing
2. Download `bible-web-nomic.db` from R2 with progress bar → `~/.local/share/bible-mcp/bible.db`
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

---

### Phase 4 — Packaging & distribution

- [ ] `pyproject.toml` with `[project.scripts] bible-mcp = "bible_mcp.cli:main"`
- [ ] Publish to PyPI — users install with `pipx install bible-mcp`
- [ ] GitHub Actions release workflow — run tests, publish to PyPI on tag

---

## Key decisions

**Four tools, complete coverage.** `search_verses` for topic/concept queries, `similar_verses` for "more like this," `get_verse` for a single known reference, `get_passage` for a range. All share fuzzy book matching and the same response shape.

**Hybrid search via RRF.** Vector search drifts on exact phrases and proper nouns; BM25 misses synonyms. RRF merges both ranked lists without ML or weight tuning — `1/(rank+60)` is a robust default.

**`similar_verses` blends cross-refs + vector.** Human-curated cross-references (OpenBible) are high-precision but sparse — not every verse has them. Vector KNN fills the gaps. Cross-ref hits are surfaced first, sorted by confidence weight.

**`similar_verses` is not BM25.** BM25 on verse text finds verses sharing words, not meaning. Vector distance and curated cross-refs are both better signals for "feel like this one."

**Book names are fuzzy-matched.** The model doesn't know canonical forms upfront. `difflib` handles "Gen", "Genesis", "First Kings", "1st Kings". Resolved name echoed back for self-correction.

**Human-readable API contract.** All tools take `(book, chapter, verse)` not opaque ids. References survive re-seeds; the model can construct calls from any known reference.

**Embedding model fixed at seed time (`nomic-embed-text`).** Query-time embeddings must match. v1 always uses `nomic-embed-text`.

**Database downloaded, not bundled.** Cached at `~/.local/share/bible-mcp/bible.db`, version-checked on `setup`/`update`.

**`asyncio.to_thread` for SQLite.** All blocking DB calls run in a thread pool.

---

## Python dependencies (`pyproject.toml`)

```toml
[project]
dependencies = [
  "mcp>=1.0",
  "ollama>=0.3",
  "sqlite-vec>=0.1",
  "httpx>=0.27",
  "click>=8.0",
  "tqdm>=4.0",
]
```

`difflib` is stdlib — no extra dependency for fuzzy book matching.

---

## Out of scope (v1)

- Multiple Bible translations
- User-supplied Bible data
- Cloud sync / remote MCP transport
- Switching embedding models (requires re-seeding ~31K verses)
- GUI / system tray
- Clustering / thematic grouping
- Reading plan generation
