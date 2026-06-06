# Bible MCP

> Jesus is King.

An MCP (Model Context Protocol) server that gives AI-driven applications like Claude Code and Codex semantic search over the Bible. Ask a question, get the most relevant verses back — powered by local embeddings and SQLite.

## How it works

1. On first launch the app downloads a pre-built SQLite database (~50 MB) containing all ~31K World English Bible verses and their vector embeddings.
2. When an AI tool calls `search_verses`, your query is embedded locally via Ollama and a cosine similarity search is run against the database — no internet required after the initial download.
3. The MCP server runs as a background process managed by the Tauri desktop app.

## Features

- Semantic verse search (`search_verses`)
- Direct verse lookup (`get_verse`)
- Passage retrieval (`get_passage`)
- Model selector — choose any Ollama embedding model installed on your machine
- Pre-built WEB (World English Bible) database, freely distributable
- Tiny installer (~8 MB), database downloaded once and cached locally

## Requirements

- [Ollama](https://ollama.com) running locally with at least one embedding model pulled (e.g. `ollama pull nomic-embed-text`)

## Installation

Download the installer for your platform from the [releases page](https://github.com/walking-wisely/holy-blocker/releases).

On first launch the app will:
1. Check for Ollama
2. Let you pick an embedding model
3. Download the Bible database from Cloudflare R2
4. Write the MCP server entry to your Claude Code config automatically

## MCP tools

| Tool | Description |
|---|---|
| `search_verses(query, limit?)` | Semantic search — returns the most relevant verses |
| `get_verse(book, chapter, verse)` | Fetch a single verse by reference |
| `get_passage(book, chapter, from, to)` | Fetch a range of verses |

## Bible version

Uses the **World English Bible (WEB)** — public domain, no restrictions, modern readable English.

## License

[Holy Blocker License](LICENSE.md) — see license file for terms.
