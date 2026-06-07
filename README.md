# Bible MCP

> Jesus is King.

A self-contained MCP (Model Context Protocol) server that gives AI tools like Claude Code semantic search over the entire Bible. No external runtime dependencies — just download one binary and run `bible-mcp setup`.

## How it works

1. `bible-mcp setup` downloads a pre-built SQLite database (~50 MB) with all ~31K World English Bible verses and their vector embeddings, then writes the MCP server entry to your Claude Code config.
2. When an AI tool calls `search_verses`, your query is embedded locally in-process (via `fastembed` / ONNX) — no Ollama, no internet required after the initial setup.
3. Claude Code spawns `bible-mcp serve` directly as a stdio MCP process.

## Features

- Hybrid semantic + keyword search (`search_verses`) via vector KNN + FTS5 BM25, merged with Reciprocal Rank Fusion
- Theologically-aware similar verse lookup (`similar_verses`) — curated cross-references first, vector similarity to fill gaps
- Direct verse lookup (`get_verse`) and passage retrieval (`get_passage`)
- Fuzzy book name matching — "Gen", "First Kings", "1st Cor" all resolve correctly
- Pre-built WEB (World English Bible) database, freely distributable
- Single binary, no Python/Node/Ollama required

## Requirements

None. Download the binary for your platform and run it.

On first run, `bible-mcp setup` will download:
- The Bible database (~50 MB) from Cloudflare R2
- The `nomic-embed-text` embedding model (~130 MB, cached in `~/.cache/fastembed/`)

Internet access is only needed for this initial setup.

## Installation

Download the binary for your platform from the [releases page](https://github.com/walking-wisely/bible-mcp/releases), then:

```sh
bible-mcp setup
```

That's it. Restart Claude Code and the `bible` MCP server will be active.

## MCP tools

| Tool | Description |
| --- | --- |
| `search_verses(query, limit?, book?)` | Hybrid semantic + keyword search over all ~31K verses |
| `similar_verses(book, chapter, verse, limit?)` | Find theologically related verses |
| `get_verse(book, chapter, verse)` | Fetch a single verse by reference |
| `get_passage(book, chapter, from_verse, to_verse)` | Fetch a contiguous range of verses |

## Bible version

Uses the **World English Bible (WEB)** — public domain, no restrictions, modern readable English.

## License

[Holy Blocker License](LICENSE.md) — see license file for terms.
