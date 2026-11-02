# Bible MCP

> Jesus is King.

An MCP (Model Context Protocol) server that gives AI assistants search over the whole Bible: find verses by meaning, look up a reference, read a passage, or follow cross-references. The text is the World English Bible, which is public domain.

You can use it two ways:

- **Hosted**: add a URL to your AI app. Nothing to install.
- **Local**: download one program, run `bible-mcp setup` once, and it runs on your computer.

## Use the hosted server

The server address is `https://bible-mcp.ivan-dutov.com/mcp`. Add it to any MCP client that supports remote servers over streamable HTTP, for example:

```sh
claude mcp add --transport http bible https://bible-mcp.ivan-dutov.com/mcp
```

Search queries are limited to 200 characters.

## Run it locally

1. Download the archive for your system from the [releases page](https://github.com/walking-wisely/bible-mcp/releases) and unpack it. Keep everything in the folder together: the `onnxruntime` library must stay next to the `bible-mcp` program.
2. Run setup:

   ```sh
   ./bible-mcp setup
   ```

   Setup downloads the Bible database (about 120 MB), builds a search index, downloads the embedding model (about 550 MB, stored in your user cache folder under `bible-mcp/models`), and adds the server to the Claude Desktop config. Run `bible-mcp status` to see where each piece lives.
3. Restart Claude Desktop.

For Claude Code, add it yourself instead (use the full path to the program):

```sh
claude mcp add bible -- /full/path/to/bible-mcp serve
```

### Platforms

Release archives are built for Windows (x64), Linux (x64, glibc) and macOS on Apple silicon. On macOS the program is not code-signed yet, so the first run may be blocked: open System Settings → Privacy & Security and choose "Open Anyway", or run `xattr -d com.apple.quarantine bible-mcp`.

An internet connection is needed for setup. After that, the local server works offline unless you configure a hosted embedding provider.

## Tools

| Tool | Arguments | What it does |
| --- | --- | --- |
| `search_verses` | `query`, `limit?`, `book?` | Finds verses by meaning and keywords. `score` runs from 0 to 1 and is a ranking, not a percentage. |
| `similar_verses` | `book`, `chapter`, `verse`, `limit?` | Finds related verses by combining curated cross-references with similar wording and meaning. Uses the same 0 to 1 `score`. |
| `get_verse` | `book`, `chapter`, `verse` | Returns one verse. |
| `get_passage` | `book`, `chapter`, `fromVerse`, `toVerse` | Returns a run of verses from one chapter. |

Book names are forgiving: `John`, `Jn`, `1 Cor`, `I Kings`, `First Kings` and `Song of Songs` all work.

## Commands

| Command | Purpose |
| --- | --- |
| `bible-mcp setup` | Download the database and model, build the index, configure Claude Desktop |
| `bible-mcp serve` | Run the server over stdio (what MCP clients launch) |
| `bible-mcp serve-http --bind ADDR` | Run the server over streamable HTTP at `/mcp` |
| `bible-mcp update` | Fetch a newer database if one is published |
| `bible-mcp build-index` | Rebuild the search index from the database |
| `bible-mcp status` | Show paths and what is installed |

## Running a server for others

`serve-http` listens on the address you give it and serves MCP at `/mcp`. Put it behind a reverse proxy or Cloudflare for TLS and rate limiting; the server itself has no authentication. It handles at most 100 requests at once and answers further ones with `503` and `Retry-After: 1`; `search_verses` rejects queries longer than 200 characters.

| Setting | Purpose |
| --- | --- |
| `BIBLE_MCP_DB_PATH` | Where the database is stored (overrides the config file) |
| `BIBLE_MCP_MANIFEST_URL` | Location of the database manifest used by `setup` and `update` |
| `FIREWORKS_API_KEY` | Embed queries through Fireworks instead of the local model |
| `BIBLE_MCP_MODEL_DIR` | Load the embedding model from a local folder |
| `ORT_DYLIB_PATH` | Use an ONNX Runtime library from another location |
| `BIBLE_MCP_ANN=off` | Disable the search index and use exact search |

The server refuses to start without a database, stops within 10 seconds of SIGTERM or Ctrl-C, and can be probed by calling `initialize` on `/mcp`.

## Bible text and data

The text is the World English Bible (public domain). Cross-references come from [OpenBible.info](https://www.openbible.info/labs/cross-references/) under CC-BY 4.0. Embeddings use [nomic-embed-text-v1.5](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) (Apache-2.0). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

Released under the [Bible MCP License](LICENSE.md), a permissive license that requires keeping the copyright notice, a link to this project, and the acknowledgment "Jesus is King."
