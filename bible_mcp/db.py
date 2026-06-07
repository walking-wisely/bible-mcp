import asyncio
import sqlite3
import struct
from pathlib import Path

import sqlite_vec


def _connect(db_path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(str(db_path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    return conn


def _floats_to_blob(floats: list[float]) -> bytes:
    return struct.pack(f"{len(floats)}f", *floats)


def _run(fn):
    """Run a blocking callable in a thread pool."""
    return asyncio.to_thread(fn)


# ---------------------------------------------------------------------------
# Public async API
# ---------------------------------------------------------------------------

async def vector_search(
    db_path: Path,
    embedding: list[float],
    limit: int,
    book_num: int | None = None,
) -> list[tuple[int, float]]:
    """Return [(verse_id, distance), ...] ordered by distance ascending."""
    def _query():
        conn = _connect(db_path)
        blob = _floats_to_blob(embedding)
        if book_num is not None:
            rows = conn.execute(
                """
                SELECT ve.rowid, ve.distance
                FROM verse_embeddings ve
                JOIN verses v ON v.id = ve.rowid
                WHERE v.book_num = ?
                  AND ve.embedding MATCH ?
                ORDER BY ve.distance
                LIMIT ?
                """,
                (book_num, blob, limit),
            ).fetchall()
        else:
            rows = conn.execute(
                """
                SELECT rowid, distance
                FROM verse_embeddings
                WHERE embedding MATCH ?
                ORDER BY distance
                LIMIT ?
                """,
                (blob, limit),
            ).fetchall()
        conn.close()
        return [(r[0], r[1]) for r in rows]

    return await _run(_query)


async def fts_search(
    db_path: Path,
    query: str,
    limit: int,
    book_num: int | None = None,
) -> list[tuple[int, float]]:
    """Return [(verse_id, bm25_score), ...] ordered by score descending (best first)."""
    def _query():
        conn = _connect(db_path)
        if book_num is not None:
            rows = conn.execute(
                """
                SELECT vf.rowid, bm25(verses_fts) AS score
                FROM verses_fts vf
                JOIN verses v ON v.id = vf.rowid
                WHERE verses_fts MATCH ?
                  AND v.book_num = ?
                ORDER BY score
                LIMIT ?
                """,
                (query, book_num, limit),
            ).fetchall()
        else:
            rows = conn.execute(
                """
                SELECT rowid, bm25(verses_fts) AS score
                FROM verses_fts
                WHERE verses_fts MATCH ?
                ORDER BY score
                LIMIT ?
                """,
                (query, limit),
            ).fetchall()
        conn.close()
        # bm25() returns negative values — lower = better match; flip for clarity
        return [(r[0], -r[1]) for r in rows]

    return await _run(_query)


def _rrf_merge(
    vector_hits: list[tuple[int, float]],
    fts_hits: list[tuple[int, float]],
    k: int = 60,
) -> list[tuple[int, float]]:
    """Reciprocal Rank Fusion over two ranked lists. Returns [(verse_id, rrf_score)]."""
    scores: dict[int, float] = {}
    for rank, (vid, _) in enumerate(vector_hits):
        scores[vid] = scores.get(vid, 0.0) + 1.0 / (rank + k)
    for rank, (vid, _) in enumerate(fts_hits):
        scores[vid] = scores.get(vid, 0.0) + 1.0 / (rank + k)
    return sorted(scores.items(), key=lambda x: x[1], reverse=True)


async def hybrid_search(
    db_path: Path,
    embedding: list[float],
    query: str,
    limit: int,
    book_num: int | None = None,
) -> list[tuple[int, float]]:
    """Merge vector + FTS results via RRF. Returns top `limit` (verse_id, score)."""
    fetch = limit * 2  # over-fetch so RRF has enough candidates
    vec_hits, fts_hits = await asyncio.gather(
        vector_search(db_path, embedding, fetch, book_num),
        fts_search(db_path, query, fetch, book_num),
    )
    merged = _rrf_merge(vec_hits, fts_hits)
    return merged[:limit]


async def get_verses_by_ids(
    db_path: Path,
    verse_ids: list[int],
) -> list[sqlite3.Row]:
    def _query():
        conn = _connect(db_path)
        placeholders = ",".join("?" * len(verse_ids))
        rows = conn.execute(
            f"SELECT id, book, book_num, chapter, verse, text FROM verses WHERE id IN ({placeholders})",
            verse_ids,
        ).fetchall()
        conn.close()
        # preserve the requested order
        order = {vid: i for i, vid in enumerate(verse_ids)}
        return sorted(rows, key=lambda r: order[r["id"]])

    return await _run(_query)


async def get_verse_by_ref(
    db_path: Path,
    book_num: int,
    chapter: int,
    verse: int,
) -> sqlite3.Row | None:
    def _query():
        conn = _connect(db_path)
        row = conn.execute(
            "SELECT id, book, book_num, chapter, verse, text FROM verses "
            "WHERE book_num = ? AND chapter = ? AND verse = ?",
            (book_num, chapter, verse),
        ).fetchone()
        conn.close()
        return row

    return await _run(_query)


async def get_passage_by_ref(
    db_path: Path,
    book_num: int,
    chapter: int,
    from_verse: int,
    to_verse: int,
) -> list[sqlite3.Row]:
    def _query():
        conn = _connect(db_path)
        rows = conn.execute(
            "SELECT id, book, book_num, chapter, verse, text FROM verses "
            "WHERE book_num = ? AND chapter = ? AND verse BETWEEN ? AND ? "
            "ORDER BY verse",
            (book_num, chapter, from_verse, to_verse),
        ).fetchall()
        conn.close()
        return rows

    return await _run(_query)


async def get_cross_refs(
    db_path: Path,
    verse_id: int,
    limit: int,
) -> list[tuple[int, float]]:
    """Return [(to_verse_id, weight), ...] sorted by weight desc."""
    def _query():
        conn = _connect(db_path)
        rows = conn.execute(
            "SELECT to_verse_id, weight FROM cross_references "
            "WHERE from_verse_id = ? ORDER BY weight DESC LIMIT ?",
            (verse_id, limit),
        ).fetchall()
        conn.close()
        return [(r[0], r[1]) for r in rows]

    return await _run(_query)


async def get_verse_embedding(
    db_path: Path,
    verse_id: int,
) -> list[float] | None:
    def _query():
        conn = _connect(db_path)
        row = conn.execute(
            "SELECT embedding FROM verse_embeddings WHERE rowid = ?",
            (verse_id,),
        ).fetchone()
        conn.close()
        if row is None:
            return None
        blob: bytes = row[0]
        n = len(blob) // 4
        return list(struct.unpack(f"{n}f", blob))

    return await _run(_query)
