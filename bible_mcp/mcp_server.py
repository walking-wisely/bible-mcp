import asyncio
from pathlib import Path

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import Tool, TextContent

from . import config, db, ollama_client
from .books import resolve_book


def _verse_row_to_dict(row, score: float | None = None) -> dict:
    d = {
        "reference": f"{row['book']} {row['chapter']}:{row['verse']}",
        "book": row["book"],
        "chapter": row["chapter"],
        "verse": row["verse"],
        "text": row["text"],
    }
    if score is not None:
        d["score"] = round(score, 4)
    return d


def create_server() -> Server:
    server = Server("bible-mcp")

    @server.list_tools()
    async def list_tools() -> list[Tool]:
        return [
            Tool(
                name="search_verses",
                description=(
                    "Hybrid semantic + keyword search over all ~31K Bible verses (World English Bible). "
                    "Returns the most relevant verses for a topic, theme, concept, or phrase. "
                    "Optionally filter by book."
                ),
                inputSchema={
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Topic, phrase, or concept to search for"},
                        "limit": {"type": "integer", "default": 5, "description": "Number of results (1–20)"},
                        "book": {"type": "string", "description": "Optional Bible book name to restrict results"},
                    },
                    "required": ["query"],
                },
            ),
            Tool(
                name="similar_verses",
                description=(
                    "Find verses similar to a given verse. Uses human-curated cross-references first, "
                    "then vector similarity to fill remaining slots. "
                    "Surfaces theologically intentional connections, not just word overlap."
                ),
                inputSchema={
                    "type": "object",
                    "properties": {
                        "book": {"type": "string", "description": "Bible book name"},
                        "chapter": {"type": "integer"},
                        "verse": {"type": "integer"},
                        "limit": {"type": "integer", "default": 5, "description": "Number of results (1–20)"},
                    },
                    "required": ["book", "chapter", "verse"],
                },
            ),
            Tool(
                name="get_verse",
                description="Retrieve a single Bible verse by exact reference.",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "book": {"type": "string"},
                        "chapter": {"type": "integer"},
                        "verse": {"type": "integer"},
                    },
                    "required": ["book", "chapter", "verse"],
                },
            ),
            Tool(
                name="get_passage",
                description="Retrieve a contiguous range of verses from one chapter.",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "book": {"type": "string"},
                        "chapter": {"type": "integer"},
                        "from_verse": {"type": "integer"},
                        "to_verse": {"type": "integer"},
                    },
                    "required": ["book", "chapter", "from_verse", "to_verse"],
                },
            ),
        ]

    @server.call_tool()
    async def call_tool(name: str, arguments: dict) -> list[TextContent]:
        cfg = config.load()
        db_path = Path(cfg["db_path"])
        model = cfg["embed_model"]

        if name == "search_verses":
            query: str = arguments["query"]
            limit: int = min(max(int(arguments.get("limit", 5)), 1), 20)
            book_str: str | None = arguments.get("book")

            book_num: int | None = None
            if book_str:
                book_num, _ = resolve_book(book_str)

            embedding = await asyncio.to_thread(ollama_client.embed, query, model)
            hits = await db.hybrid_search(db_path, embedding, query, limit, book_num)

            verse_ids = [vid for vid, _ in hits]
            score_map = {vid: score for vid, score in hits}
            rows = await db.get_verses_by_ids(db_path, verse_ids)

            results = [_verse_row_to_dict(r, score_map[r["id"]]) for r in rows]
            import json
            return [TextContent(type="text", text=json.dumps(results, indent=2))]

        elif name == "similar_verses":
            book_str: str = arguments["book"]
            chapter: int = int(arguments["chapter"])
            verse_num: int = int(arguments["verse"])
            limit: int = min(max(int(arguments.get("limit", 5)), 1), 20)

            book_num, canonical_book = resolve_book(book_str)
            src_row = await db.get_verse_by_ref(db_path, book_num, chapter, verse_num)
            if src_row is None:
                import json
                return [TextContent(type="text", text=json.dumps({"error": f"{canonical_book} {chapter}:{verse_num} not found."}))]

            src_id = src_row["id"]

            # cross-refs first
            xrefs = await db.get_cross_refs(db_path, src_id, limit)
            xref_ids = [vid for vid, _ in xrefs]
            xref_weights = {vid: w for vid, w in xrefs}

            remaining = limit - len(xref_ids)
            vec_ids: list[int] = []
            if remaining > 0:
                src_embedding = await db.get_verse_embedding(db_path, src_id)
                if src_embedding:
                    vec_hits = await db.vector_search(db_path, src_embedding, limit + 1)
                    seen = set(xref_ids) | {src_id}
                    vec_ids = [vid for vid, _ in vec_hits if vid not in seen][:remaining]

            all_ids = xref_ids + vec_ids
            rows_by_id = {r["id"]: r for r in await db.get_verses_by_ids(db_path, all_ids)}

            results = []
            for vid in xref_ids:
                if vid in rows_by_id:
                    results.append(_verse_row_to_dict(rows_by_id[vid], xref_weights[vid]))
            for vid in vec_ids:
                if vid in rows_by_id:
                    results.append(_verse_row_to_dict(rows_by_id[vid]))

            import json
            return [TextContent(type="text", text=json.dumps(results, indent=2))]

        elif name == "get_verse":
            book_num, canonical_book = resolve_book(arguments["book"])
            chapter = int(arguments["chapter"])
            verse_num = int(arguments["verse"])
            row = await db.get_verse_by_ref(db_path, book_num, chapter, verse_num)
            import json
            if row is None:
                return [TextContent(type="text", text=json.dumps({"error": f"{canonical_book} {chapter}:{verse_num} not found."}))]
            return [TextContent(type="text", text=json.dumps(_verse_row_to_dict(row), indent=2))]

        elif name == "get_passage":
            book_num, canonical_book = resolve_book(arguments["book"])
            chapter = int(arguments["chapter"])
            from_verse = int(arguments["from_verse"])
            to_verse = int(arguments["to_verse"])
            rows = await db.get_passage_by_ref(db_path, book_num, chapter, from_verse, to_verse)
            import json
            results = [_verse_row_to_dict(r) for r in rows]
            return [TextContent(type="text", text=json.dumps(results, indent=2))]

        else:
            import json
            return [TextContent(type="text", text=json.dumps({"error": f"Unknown tool: {name}"}))]

    return server


async def run_stdio() -> None:
    server = create_server()
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())
