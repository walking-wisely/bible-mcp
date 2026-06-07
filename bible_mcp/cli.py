import asyncio
import hashlib
import json
from pathlib import Path

import click
import httpx
from tqdm import tqdm

from . import config

MANIFEST_URL = "https://pub-ecf6466e0d38411caea06a57e4b46164.r2.dev/manifest.json"
CLAUDE_CODE_MCP_ENTRY = {
    "command": "bible-mcp",
    "args": ["serve"],
}


def _detect_ollama() -> bool:
    try:
        r = httpx.get("http://localhost:11434/", timeout=3)
        return r.status_code < 500
    except Exception:
        return False


def _fetch_manifest() -> dict:
    r = httpx.get(MANIFEST_URL, timeout=30, follow_redirects=True)
    r.raise_for_status()
    return r.json()


def _download_db(url: str, dest: Path, expected_sha256: str) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    with httpx.stream("GET", url, timeout=300, follow_redirects=True) as r:
        r.raise_for_status()
        total = int(r.headers.get("content-length", 0))
        hasher = hashlib.sha256()
        with open(dest, "wb") as f, tqdm(
            total=total or None,
            unit="B",
            unit_scale=True,
            desc=dest.name,
        ) as bar:
            for chunk in r.iter_bytes(chunk_size=65536):
                f.write(chunk)
                hasher.update(chunk)
                bar.update(len(chunk))
    digest = hasher.hexdigest()
    if digest != expected_sha256:
        dest.unlink(missing_ok=True)
        raise click.ClickException(f"SHA-256 mismatch: expected {expected_sha256}, got {digest}")


def _write_claude_config() -> Path:
    # Claude Code stores its config at ~/.claude/claude_code_config.json (or similar).
    # The user-level MCP config lives at ~/.claude/config.json.
    config_path = Path.home() / ".claude" / "config.json"
    existing: dict = {}
    if config_path.exists():
        try:
            existing = json.loads(config_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            pass

    mcp_servers: dict = existing.setdefault("mcpServers", {})
    mcp_servers["bible"] = CLAUDE_CODE_MCP_ENTRY
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(json.dumps(existing, indent=2), encoding="utf-8")
    return config_path


@click.group()
def main() -> None:
    """bible-mcp — local MCP server for semantic Bible search."""


@main.command()
def setup() -> None:
    """Download the Bible database and write the Claude Code MCP config."""
    click.echo("=== bible-mcp setup ===\n")

    # 1. Database
    cfg = config.load()
    db_path = Path(cfg["db_path"])
    try:
        manifest = _fetch_manifest()
    except Exception as exc:
        raise click.ClickException(f"Could not fetch manifest: {exc}") from exc

    if db_path.exists():
        click.echo(f"✓ Database already present at {db_path}")
    else:
        click.echo(f"Downloading Bible database to {db_path} …")
        _download_db(manifest["url"], db_path, manifest["sha256"])
        click.echo("✓ Database downloaded")

    # 3. Config
    config.save({**cfg, "db_version": manifest.get("version", "unknown")})

    # 4. Claude Code MCP entry
    claude_cfg_path = _write_claude_config()
    click.echo(f"✓ MCP entry written to {claude_cfg_path}")

    click.echo("\nSetup complete! Restart Claude Code to activate the 'bible' MCP server.")


@main.command()
def serve() -> None:
    """Start the MCP stdio server (invoked by Claude Code)."""
    from .mcp_server import run_stdio
    asyncio.run(run_stdio())


@main.command()
def status() -> None:
    """Show current config, DB status, and Ollama availability."""
    cfg = config.load()
    db_path = Path(cfg["db_path"])

    click.echo(f"Config file : {Path.home() / '.config' / 'bible-mcp' / 'config.json'}")
    click.echo(f"DB path     : {db_path}")
    click.echo(f"DB exists   : {'yes' if db_path.exists() else 'no'}")
    click.echo(f"Embed model : {cfg['embed_model']}")
    click.echo(f"DB version  : {cfg.get('db_version', 'unknown')}")
    click.echo(f"Ollama      : {'running' if _detect_ollama() else 'not detected'}")


@main.command()
def update() -> None:
    """Re-check manifest; re-download DB if the version has changed."""
    cfg = config.load()
    db_path = Path(cfg["db_path"])

    try:
        manifest = _fetch_manifest()
    except Exception as exc:
        raise click.ClickException(f"Could not fetch manifest: {exc}") from exc

    current = cfg.get("db_version", "")
    latest = manifest.get("version", "")

    if current == latest and db_path.exists():
        click.echo(f"Already up to date (version {current}).")
        return

    click.echo(f"Updating from version {current!r} to {latest!r} …")
    _download_db(manifest["url"], db_path, manifest["sha256"])
    config.save({**cfg, "db_version": latest})
    click.echo("✓ Database updated.")
