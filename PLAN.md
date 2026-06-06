# Implementation Plan

## Overview

A Tauri desktop app that manages a local MCP server providing semantic Bible search to AI tools (Claude Code, Codex, etc.). The app is minimal — a system tray icon with a settings window. The MCP server runs as an async task inside the same Rust binary — no sidecar, no subprocess, no extra runtime.

---

## Stack

| Layer | Technology |
|---|---|
| Desktop app + MCP server | Tauri (Rust) — single binary |
| MCP protocol | `rmcp` (official Rust MCP SDK) |
| Async runtime | Tokio (bundled with Tauri) |
| Vector store | SQLite + `sqlite-vec` via `rusqlite` |
| Embeddings (runtime) | Ollama local HTTP API via `ollama-rs` |
| Embeddings (pre-built db) | `nomic-embed-text` via Ollama (run once by maintainer) |
| Bible data | World English Bible (WEB), public domain |
| Database hosting | Cloudflare R2 (free tier) |

---

## Project structure

```
bible-mcp/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs          # Tauri setup, tray icon, spawn tasks
│   │   ├── mcp/
│   │   │   ├── mod.rs       # MCP server entry — rmcp stdio transport
│   │   │   ├── tools.rs     # search_verses, get_verse, get_passage handlers
│   │   │   └── schema.rs    # MCP tool input/output types
│   │   ├── db/
│   │   │   ├── mod.rs       # SQLite connection pool, migrations
│   │   │   ├── download.rs  # R2 download, sha256 verify, cache logic
│   │   │   └── search.rs    # vec_distance_cosine KNN query
│   │   ├── ollama/
│   │   │   └── mod.rs       # list_models(), embed(text, model) via ollama-rs
│   │   └── config.rs        # read/write active model, db path, MCP port
│   ├── Cargo.toml
│   └── tauri.conf.json
├── ui/                      # settings WebView (plain HTML/CSS/JS)
│   └── index.html           # model selector, status, first-run wizard
├── seed/                    # one-time offline tooling (not shipped)
│   ├── seed.py              # fetch WEB JSON → embed → write SQLite → upload R2
│   └── requirements.txt     # ollama, sqlite-vec, requests, tqdm
├── README.md
├── PLAN.md
└── LICENSE.md
```

---

## Phases

### Phase 1 — Seed script (offline, run once by maintainer)

A standalone Rust binary in `seed/` — not shipped to users.

- [ ] Download WEB verse JSON (`thiagobodruk/bible` or eBible.org)
- [ ] Parse into flat list: `{ book, book_num, chapter, verse, text }`
- [ ] Batch embed all ~31K verses via Ollama (`nomic-embed-text`, 384-dim)
- [ ] Write to SQLite with `sqlite-vec` virtual table and KNN index
- [ ] Compute `sha256` of the resulting file
- [ ] Upload `bible-web-nomic.db` to Cloudflare R2 (public bucket)
- [ ] Publish `manifest.json` to R2:
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

CREATE VIRTUAL TABLE verse_embeddings USING vec0(
  embedding float[384]
);
```

Python packages: `ollama`, `sqlite-vec`, `requests`, `tqdm`, `hashlib` (stdlib)

---

### Phase 2 — MCP server (inside Tauri, `src-tauri/src/mcp/`)

- [ ] Set up `rmcp` with stdio transport (Claude Code connects via subprocess or local socket)
- [ ] Implement tool handlers in `tools.rs`:
  - `search_verses(query: string, limit?: number)` — embed query via Ollama → KNN search → return verses with references
  - `get_verse(book: string, chapter: number, verse: number)` — direct lookup
  - `get_passage(book: string, chapter: number, from: number, to: number)` — range lookup
- [ ] `db/search.rs` — `spawn_blocking` wrapper around rusqlite KNN query (keeps Tokio happy)
- [ ] `ollama/mod.rs` — `list_models()` for UI, `embed()` for query-time embedding
- [ ] Read active embedding model from `config.rs` (written by settings UI)

---

### Phase 3 — Tauri app (`src-tauri/src/main.rs` + `ui/`)

- [ ] System tray icon with menu: Open Settings / Quit
- [ ] On startup: `tokio::spawn` the MCP server task alongside the Tauri event loop
- [ ] First-run wizard (WebView `ui/index.html`):
  - Ping `localhost:11434` — detect Ollama, show link if missing
  - Populate model selector from `ollama-rs` `list_models()`
  - Show download progress bar for `bible.db` (streamed via `reqwest`, progress sent to WebView via Tauri events)
  - Write MCP server entry to Claude Code config on completion:
    ```json
    {
      "mcpServers": {
        "bible": {
          "command": "<path_to_this_binary>",
          "args": ["--mcp"]
        }
      }
    }
    ```
- [ ] Settings window:
  - Model selector (persisted to `config.rs`)
  - MCP status (running / error)
  - Re-download database button
- [ ] `--mcp` CLI flag — when launched with this flag, skip Tauri UI entirely and run only the MCP stdio server. This is what Claude Code invokes.

---

### Phase 4 — Packaging & distribution

- [ ] Configure `tauri.conf.json`: `.msi` (Windows), `.dmg` (Mac), `.AppImage` (Linux)
- [ ] GitHub Actions release workflow — build all three platforms, attach to GitHub release
- [ ] Code signing (optional, avoids OS security warnings on Mac/Windows)

---

## Key decisions

**Single binary, two modes.** The same binary is both the Tauri desktop app and the MCP server. When launched normally it shows the tray icon and manages everything. When launched with `--mcp` it runs headless as a pure stdio MCP server — this is what gets registered in Claude Code's config.

**Embedding model is fixed at seed time (`nomic-embed-text`).** Query-time embeddings must use the same model. The model selector in the UI is reserved for a future re-seed feature — not v1.

**Database is downloaded, not bundled.** Installer stays ~8 MB. Cached at `{app_data_dir}/bible.db`, version-checked against `manifest.json` on each launch, re-downloaded only when version changes.

**`spawn_blocking` for SQLite.** `rusqlite` is synchronous. All DB calls are wrapped in `tokio::task::spawn_blocking` to avoid blocking the async runtime.

---

## Crate dependencies (`src-tauri/Cargo.toml`)

```toml
[dependencies]
tauri       = { version = "2", features = ["tray-icon"] }
rmcp        = { version = "0.1", features = ["server", "transport-io"] }
tokio       = { version = "1", features = ["full"] }
rusqlite    = { version = "0.31", features = ["bundled"] }
sqlite-vec  = "0.0.1"
ollama-rs   = "0.2"
reqwest     = { version = "0.12", features = ["stream", "json"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
sha2        = "0.10"
dirs        = "5"
```

---

## Out of scope (v1)

- Multiple Bible translations
- User-supplied Bible data
- Cloud sync / remote MCP transport
- Switching embedding models (requires re-seeding ~31K verses)
