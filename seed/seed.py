"""
Seed script — run once offline by the maintainer.

Downloads the World English Bible from TehShrike/world-english-bible (66 per-book
JSON files), embeds all ~31K verses via nomic-embed-text (768-dim), writes a SQLite
database with sqlite-vec, computes SHA-256, and uploads to Cloudflare R2.

Embedding backends (--backend):
  sentence-transformers  Use HuggingFace model directly — works on Kaggle/Colab GPU.
                         This is the recommended backend for bulk seeding.
  ollama                 Use a local Ollama instance — good for local dev/testing.

Usage (Kaggle / any machine with a GPU):
    pip install -r requirements.txt
    python seed.py [--db bible-web-nomic.db] [--backend sentence-transformers]

Usage (local with Ollama):
    ollama pull nomic-embed-text
    python seed.py --backend ollama

Environment variables for upload (loaded from .env):
    R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_BUCKET
"""

import argparse
import hashlib
import json
import sqlite3
import struct
import sys
from pathlib import Path

import requests
import sqlite_vec
from dotenv import load_dotenv
from tqdm import tqdm

load_dotenv(Path(__file__).parent / ".env")

BASE_URL = "https://raw.githubusercontent.com/TehShrike/world-english-bible/master/json"
HF_MODEL = "nomic-ai/nomic-embed-text-v1"   # sentence-transformers backend
OLLAMA_MODEL = "nomic-embed-text"            # ollama backend
EMBED_DIM = 768
BATCH_SIZE = 64   # larger batches are fine on GPU

# (book_num, canonical_name, filename_slug) — 66 books in canonical order
BOOKS = [
    (1,  "Genesis",          "genesis"),
    (2,  "Exodus",           "exodus"),
    (3,  "Leviticus",        "leviticus"),
    (4,  "Numbers",          "numbers"),
    (5,  "Deuteronomy",      "deuteronomy"),
    (6,  "Joshua",           "joshua"),
    (7,  "Judges",           "judges"),
    (8,  "Ruth",             "ruth"),
    (9,  "1 Samuel",         "1samuel"),
    (10, "2 Samuel",         "2samuel"),
    (11, "1 Kings",          "1kings"),
    (12, "2 Kings",          "2kings"),
    (13, "1 Chronicles",     "1chronicles"),
    (14, "2 Chronicles",     "2chronicles"),
    (15, "Ezra",             "ezra"),
    (16, "Nehemiah",         "nehemiah"),
    (17, "Esther",           "esther"),
    (18, "Job",              "job"),
    (19, "Psalms",           "psalms"),
    (20, "Proverbs",         "proverbs"),
    (21, "Ecclesiastes",     "ecclesiastes"),
    (22, "Song of Solomon",  "songofsolomon"),
    (23, "Isaiah",           "isaiah"),
    (24, "Jeremiah",         "jeremiah"),
    (25, "Lamentations",     "lamentations"),
    (26, "Ezekiel",          "ezekiel"),
    (27, "Daniel",           "daniel"),
    (28, "Hosea",            "hosea"),
    (29, "Joel",             "joel"),
    (30, "Amos",             "amos"),
    (31, "Obadiah",          "obadiah"),
    (32, "Jonah",            "jonah"),
    (33, "Micah",            "micah"),
    (34, "Nahum",            "nahum"),
    (35, "Habakkuk",         "habakkuk"),
    (36, "Zephaniah",        "zephaniah"),
    (37, "Haggai",           "haggai"),
    (38, "Zechariah",        "zechariah"),
    (39, "Malachi",          "malachi"),
    (40, "Matthew",          "matthew"),
    (41, "Mark",             "mark"),
    (42, "Luke",             "luke"),
    (43, "John",             "john"),
    (44, "Acts",             "acts"),
    (45, "Romans",           "romans"),
    (46, "1 Corinthians",    "1corinthians"),
    (47, "2 Corinthians",    "2corinthians"),
    (48, "Galatians",        "galatians"),
    (49, "Ephesians",        "ephesians"),
    (50, "Philippians",      "philippians"),
    (51, "Colossians",       "colossians"),
    (52, "1 Thessalonians",  "1thessalonians"),
    (53, "2 Thessalonians",  "2thessalonians"),
    (54, "1 Timothy",        "1timothy"),
    (55, "2 Timothy",        "2timothy"),
    (56, "Titus",            "titus"),
    (57, "Philemon",         "philemon"),
    (58, "Hebrews",          "hebrews"),
    (59, "James",            "james"),
    (60, "1 Peter",          "1peter"),
    (61, "2 Peter",          "2peter"),
    (62, "1 John",           "1john"),
    (63, "2 John",           "2john"),
    (64, "3 John",           "3john"),
    (65, "Jude",             "jude"),
    (66, "Revelation",       "revelation"),
]


def fetch_book(slug: str) -> list[dict]:
    url = f"{BASE_URL}/{slug}.json"
    r = requests.get(url, timeout=30)
    r.raise_for_status()
    return r.json()


def parse_book(tokens: list[dict], book_num: int, book_name: str) -> list[dict]:
    """Extract verses from a book's token array.

    The JSON mixes structural tokens ("paragraph start", "paragraph end", etc.)
    with actual text tokens ("paragraph text"). A single verse can appear as
    multiple consecutive "paragraph text" tokens with the same chapterNumber and
    verseNumber but different sectionNumber — concatenate those.
    """
    # key: (chapter, verse) → accumulated text parts
    verse_parts: dict[tuple[int, int], list[str]] = {}
    verse_order: list[tuple[int, int]] = []

    for token in tokens:
        if token.get("type") != "paragraph text":
            continue
        ch = token["chapterNumber"]
        v = token["verseNumber"]
        text = token["value"].strip()
        key = (ch, v)
        if key not in verse_parts:
            verse_parts[key] = []
            verse_order.append(key)
        verse_parts[key].append(text)

    verses = []
    for (ch, v) in verse_order:
        combined = " ".join(verse_parts[(ch, v)])
        verses.append(
            {
                "book": book_name,
                "book_num": book_num,
                "chapter": ch,
                "verse": v,
                "text": combined,
            }
        )
    return verses


def download_all_verses(cache_dir: Path | None = None) -> list[dict]:
    all_verses: list[dict] = []
    for book_num, book_name, slug in tqdm(BOOKS, desc="Downloading books"):
        if cache_dir:
            cached = cache_dir / f"{slug}.json"
            if cached.exists():
                tokens = json.loads(cached.read_text(encoding="utf-8"))
            else:
                tokens = fetch_book(slug)
                cached.write_text(json.dumps(tokens), encoding="utf-8")
        else:
            tokens = fetch_book(slug)

        all_verses.extend(parse_book(tokens, book_num, book_name))

    print(f"Total verses parsed: {len(all_verses):,}")
    return all_verses


def format_embed_text(v: dict) -> str:
    # nomic-embed-text expects a task prefix for asymmetric retrieval;
    # "search_document:" is used at index time, "search_query:" at query time.
    return f"search_document: {v['book']} {v['chapter']}:{v['verse']} — {v['text']}"


def floats_to_blob(floats) -> bytes:
    floats = list(floats)
    return struct.pack(f"{len(floats)}f", *floats)


# ---------------------------------------------------------------------------
# Embedding backends
# ---------------------------------------------------------------------------

def make_embedder(backend: str):
    """Return a callable(texts) -> list[list[float]] for the chosen backend."""
    if backend == "sentence-transformers":
        from sentence_transformers import SentenceTransformer
        model = SentenceTransformer(HF_MODEL, trust_remote_code=True)
        print(f"Loaded {HF_MODEL} via sentence-transformers (device: {model.device})")

        def embed(texts: list[str]) -> list[list[float]]:
            vecs = model.encode(texts, normalize_embeddings=True)
            return vecs.tolist()

        return embed

    elif backend == "ollama":
        import ollama as _ollama

        def embed(texts: list[str]) -> list[list[float]]:
            return _ollama.embed(model=OLLAMA_MODEL, input=texts).embeddings

        return embed

    else:
        raise ValueError(f"Unknown backend: {backend!r}. Choose 'sentence-transformers' or 'ollama'.")


def create_db(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(str(path))
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)

    conn.executescript(
        f"""
        CREATE TABLE IF NOT EXISTS verses (
            id       INTEGER PRIMARY KEY,
            book     TEXT    NOT NULL,
            book_num INTEGER NOT NULL,
            chapter  INTEGER NOT NULL,
            verse    INTEGER NOT NULL,
            text     TEXT    NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS verse_embeddings USING vec0(
            embedding float[{EMBED_DIM}]
        );
        """
    )
    conn.commit()
    return conn


def insert_verses(conn: sqlite3.Connection, verses: list[dict]) -> None:
    conn.executemany(
        "INSERT INTO verses (book, book_num, chapter, verse, text) "
        "VALUES (:book, :book_num, :chapter, :verse, :text)",
        verses,
    )
    conn.commit()
    print(f"Inserted {len(verses):,} verse rows.")


def embed_and_insert(conn: sqlite3.Connection, verses: list[dict], embedder) -> None:
    print(f"Embedding {len(verses):,} verses in batches of {BATCH_SIZE} …")
    batches = [verses[i : i + BATCH_SIZE] for i in range(0, len(verses), BATCH_SIZE)]

    row_id = 1
    for batch in tqdm(batches, unit="batch"):
        texts = [format_embed_text(v) for v in batch]
        embeddings = embedder(texts)
        conn.executemany(
            "INSERT INTO verse_embeddings (rowid, embedding) VALUES (?, ?)",
            [(row_id + i, floats_to_blob(emb)) for i, emb in enumerate(embeddings)],
        )
        row_id += len(batch)

    conn.commit()
    print("Embeddings inserted.")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def upload_to_r2(db_path: Path, sha256: str) -> None:
    import os
    import boto3

    account_id = os.environ["R2_ACCOUNT_ID"]
    access_key = os.environ["R2_ACCESS_KEY_ID"]
    secret_key = os.environ["R2_SECRET_ACCESS_KEY"]
    bucket = os.environ["R2_BUCKET"]

    endpoint = f"https://{account_id}.r2.cloudflarestorage.com"
    s3 = boto3.client(
        "s3",
        endpoint_url=endpoint,
        aws_access_key_id=access_key,
        aws_secret_access_key=secret_key,
        region_name="auto",
    )

    db_key = db_path.name
    print(f"Uploading {db_path} → s3://{bucket}/{db_key} …")
    s3.upload_file(str(db_path), bucket, db_key)

    public_url = f"https://pub-REPLACE_ME.r2.dev/{db_key}"  # update after bucket is public
    manifest = {
        "version": "1.0.0",
        "sha256": sha256,
        "url": public_url,
        "embed_model": EMBED_MODEL,
        "embed_dim": EMBED_DIM,
    }
    print("Uploading manifest.json …")
    s3.put_object(
        Bucket=bucket,
        Key="manifest.json",
        Body=json.dumps(manifest, indent=2).encode(),
        ContentType="application/json",
    )
    print("Upload complete.")
    print(f"manifest.json: {manifest}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Seed bible-mcp SQLite database")
    parser.add_argument("--db", default="bible-web-nomic.db", help="Output SQLite file")
    parser.add_argument("--upload", action="store_true", help="Upload to Cloudflare R2 after seeding")
    parser.add_argument(
        "--upload-only",
        action="store_true",
        help="Skip download/embed — just upload an existing .db file to R2",
    )
    parser.add_argument(
        "--cache-dir",
        metavar="DIR",
        help="Cache downloaded book JSONs in this directory to avoid re-fetching",
    )
    parser.add_argument(
        "--backend",
        default="sentence-transformers",
        choices=["sentence-transformers", "ollama"],
        help="Embedding backend (default: sentence-transformers)",
    )
    args = parser.parse_args()

    db_path = Path(args.db)

    if args.upload_only:
        if not db_path.exists():
            print(f"Error: {db_path} not found. Run without --upload-only first.")
            sys.exit(1)
        digest = sha256_file(db_path)
        size_mb = db_path.stat().st_size / 1_048_576
        print(f"Uploading existing {db_path} ({size_mb:.1f} MB), sha256={digest}")
        upload_to_r2(db_path, digest)
        return

    cache_dir = Path(args.cache_dir) if args.cache_dir else None
    if cache_dir:
        cache_dir.mkdir(parents=True, exist_ok=True)

    verses = download_all_verses(cache_dir=cache_dir)

    print(f"Creating database at {db_path} …")
    if db_path.exists():
        db_path.unlink()
    conn = create_db(db_path)

    insert_verses(conn, verses)
    embedder = make_embedder(args.backend)
    embed_and_insert(conn, verses, embedder)
    conn.close()

    digest = sha256_file(db_path)
    size_mb = db_path.stat().st_size / 1_048_576
    print(f"\nDone! {db_path} ({size_mb:.1f} MB), sha256={digest}")

    if args.upload:
        upload_to_r2(db_path, digest)
    else:
        print("\nRun with --upload-only to push the existing .db to R2.")
        print(f'sha256="{digest}"')


if __name__ == "__main__":
    main()
