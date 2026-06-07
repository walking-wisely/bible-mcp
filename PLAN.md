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
│   ├── mcp_server.py    # MCP stdio server — search_verses, similar_verses
│   ├── db.py            # SQLite connection, KNN search, FTS5, RRF merge
│   ├── books.py         # canonical book name list + fuzzy matching via difflib
│   ├── download.py      # R2 download, sha256 verify, cache logic
│   ├── ollama_client.py # list_models(), embed(text, model)
│   └── config.py        # read/write active model, db path — ~/.config/bible-mcp/config.json
├── seed/                # one-time offline tooling (not shipped)
│   ├── seed.py          # fetch WEB JSON → embed → write SQLite → upload R2
│   └── requirements.txt # ollama, sqlite-vec, requests, tqdm
├── pyproject.toml
├── README.md
├── PLAN.md
└── LICENSE.md
```

---

## Phases

### Phase 1 — Seed script (offline, run once by maintainer)

Already done — `seed/seed.py` exists and produces `bible-web-nomic.db`.

The database is uploaded to Cloudflare R2 with a `manifest.json`:
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

-- full-text search
CREATE VIRTUAL TABLE verses_fts USING fts5(
  text,
  content=verses,
  content_rowid=id
);
```

`verses_fts` is populated during seed alongside embeddings.

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

The `book` field in the response always reflects the resolved canonical name, so the model self-corrects if it passed a variant like "First Kings" and receives back "1 Kings".

#### `search_verses` — hybrid retrieval

1. Embed `query` via Ollama (`nomic-embed-text`)
2. Run KNN vector search → ranked list with scores
3. Run FTS5 BM25 search over `verses_fts` → ranked list
4. Merge both ranked lists with **Reciprocal Rank Fusion**: `score = 1 / (rank + 60)`, summed across systems
5. If `book` is provided, apply fuzzy match first (see below), then filter both queries by `book_num`
6. Return top `limit` results by merged RRF score

#### `similar_verses`

1. Resolve `book` via fuzzy match → canonical book name + `book_num`
2. Look up the embedding for `(book_num, chapter, verse)` directly from `verse_embeddings`
3. Run KNN with that embedding as the query vector
4. Return top `limit` results (excluding the input verse itself)

Note: `similar_verses` is vector-only — BM25 doesn't make sense for "find verses that feel like this one."

#### Book name fuzzy matching (`bible_mcp/books.py`)

- Maintain a list of 66 canonical book names (e.g. `"1 Kings"`, `"Song of Solomon"`)
- On any incoming `book` string, run `difflib.get_close_matches(book, canonical_names, n=1, cutoff=0.6)`
- If no match above threshold, return a clear error: `"Unknown book: '{book}'. Try a standard Bible book name."`
- Always include the resolved canonical name in the response so the model learns the correct form

#### Implementation details

- [ ] Set up `mcp` Python SDK with stdio transport
- [ ] `db.py` — async-friendly SQLite wrapper using `asyncio.to_thread` for all blocking calls
- [ ] `ollama_client.py` — `list_models()` and `embed(text, model)` via `ollama` Python client
- [ ] RRF merge implemented in pure Python over the two result lists

---

### Phase 3 — CLI (`bible_mcp/cli.py`)

Commands exposed via a `bible-mcp` entry point:

| Command | Description |
|---|---|
| `bible-mcp setup` | First-run wizard: detect Ollama, download DB, write Claude Code MCP config |
| `bible-mcp serve` | Start the MCP stdio server (this is what Claude Code invokes) |
| `bible-mcp status` | Show config, DB path, Ollama status |
| `bible-mcp update` | Re-check manifest and re-download DB if version changed |

**`bible-mcp setup` flow:**
1. Ping `localhost:11434` — detect Ollama, print link if missing
2. Download `bible-web-nomic.db` from R2 with progress bar (cache at `~/.local/share/bible-mcp/bible.db`)
3. Write `~/.config/bible-mcp/config.json` with DB path
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

**Two tools, focused scope.** `search_verses` for topic/concept queries, `similar_verses` for "more like this." No `get_verse` or `get_passage` in v1 — those are a different use case (lookup by known reference) and can be added when there's a concrete signal they're needed.

**Hybrid search via RRF.** Vector search alone drifts on exact phrases and proper nouns. BM25 alone misses synonyms and concepts. RRF merges both ranked lists with no ML and no weight tuning — `1/(rank+60)` is a robust default.

**`similar_verses` is vector-only.** BM25 similarity on text would just find verses with the same words, not the same meaning. Vector distance is the right metric here.

**Book names are fuzzy-matched, not validated strictly.** The model doesn't know the exact canonical form upfront. `difflib` handles "Gen", "Genesis", "First Kings", "1st Kings" etc. The resolved canonical name is always echoed back in the response so the model self-corrects.

**Human-readable references in the API contract.** `similar_verses` takes `(book, chapter, verse)` rather than an opaque `id`. The model can construct a call from any verse reference it already knows, and references survive re-seeds without any encoding scheme.

**Embedding model is fixed at seed time (`nomic-embed-text`).** Query-time embeddings must use the same model. v1 always uses `nomic-embed-text`.

**Database is downloaded, not bundled.** Package stays small. Cached at `~/.local/share/bible-mcp/bible.db`, version-checked against `manifest.json` on `setup`/`update`.

**`asyncio.to_thread` for SQLite.** `sqlite-vec` and FTS5 are synchronous. All DB calls run in a thread pool to keep the async MCP server responsive.

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
