import json
from pathlib import Path

_CONFIG_DIR = Path.home() / ".config" / "bible-mcp"
_CONFIG_FILE = _CONFIG_DIR / "config.json"
_DEFAULT_DB = Path.home() / ".local" / "share" / "bible-mcp" / "bible.db"

_DEFAULTS = {
    "db_path": str(_DEFAULT_DB),
    "embed_model": "nomic-embed-text",
}


def load() -> dict:
    if _CONFIG_FILE.exists():
        data = json.loads(_CONFIG_FILE.read_text(encoding="utf-8"))
        return {**_DEFAULTS, **data}
    return dict(_DEFAULTS)


def save(cfg: dict) -> None:
    _CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    _CONFIG_FILE.write_text(json.dumps(cfg, indent=2), encoding="utf-8")


def db_path() -> Path:
    return Path(load()["db_path"])


def embed_model() -> str:
    return load()["embed_model"]
