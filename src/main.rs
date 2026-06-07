use bible_mcp::{config, download, embed, mcp_server};

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "bible-mcp", version, about = "Bible MCP server and CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// First-run setup: download the database and embedding model
    Setup,
    /// Start the MCP stdio server
    Serve,
    /// Show current configuration and status
    Status,
    /// Re-check the manifest and update the database if a new version exists
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    bible_mcp::db::init_sqlite_vec();
    let cli = Cli::parse();
    match cli.command {
        Command::Setup => cmd_setup().await,
        Command::Serve => cmd_serve().await,
        Command::Status => cmd_status(),
        Command::Update => cmd_update().await,
    }
}

async fn cmd_setup() -> Result<()> {
    let cfg = config::load()?;
    let db_path = cfg.db_path.clone();

    println!("Setting up bible-mcp...");
    println!("Database will be stored at: {}", db_path.display());

    download::ensure_db(&db_path).await?;

    println!("Pre-loading embedding model (this may take a minute on first run)...");
    tokio::task::spawn_blocking(|| embed::embed_query("warm up"))
        .await
        .map_err(|e| anyhow::anyhow!("join: {}", e))??;

    config::save(&cfg)?;
    let config_path = write_mcp_config()?;

    println!("\nSetup complete!");
    match config_path {
        Some(path) => println!("Claude config updated at: {}", path.display()),
        None => {
            println!("Could not locate a Claude Desktop config directory automatically.");
            println!("Add this MCP server entry manually:");
            println!(
                r#"  {{ "mcpServers": {{ "bible": {{ "command": "bible-mcp", "args": ["serve"] }} }} }}"#
            );
        }
    }
    Ok(())
}

async fn cmd_serve() -> Result<()> {
    let cfg = config::load()?;
    let server = mcp_server::BibleServer {
        db_path: cfg.db_path,
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = config::load()?;
    println!("Database path : {}", cfg.db_path.display());
    println!(
        "Database exists: {}",
        if cfg.db_path.exists() { "yes" } else { "no" }
    );
    let cache = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fastembed");
    println!("Embedding cache: {}", cache.display());
    println!(
        "Embedding cached: {}",
        if cache.exists() { "yes" } else { "no" }
    );
    Ok(())
}

async fn cmd_update() -> Result<()> {
    let cfg = config::load()?;
    println!("Checking for database updates...");
    let manifest = download::fetch_manifest().await?;
    if download::db_needs_update(&cfg.db_path, &manifest) {
        println!(
            "Update available (version {}). Downloading...",
            manifest.version
        );
        download::download_db(&cfg.db_path, &manifest).await?;
        println!("Update complete.");
    } else {
        println!("Database is up to date (version {}).", manifest.version);
    }
    Ok(())
}

fn write_mcp_config() -> Result<Option<PathBuf>> {
    let Some(path) = claude_config_path() else {
        return Ok(None);
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        "{}".to_string()
    };

    let merged = merge_mcp_config(&existing)?;
    std::fs::write(&path, serde_json::to_string_pretty(&merged)?)?;
    Ok(Some(path))
}

fn claude_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Claude").join("claude_desktop_config.json"))
}

fn merge_mcp_config(existing: &str) -> Result<serde_json::Value> {
    let mut root = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(existing)?
    };

    if !root.is_object() {
        root = json!({});
    }

    let server_entry = json!({
        "command": "bible-mcp",
        "args": ["serve"]
    });

    let obj = root
        .as_object_mut()
        .expect("root must be object after normalization");
    let mcp_servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
    if !mcp_servers.is_object() {
        *mcp_servers = json!({});
    }
    mcp_servers
        .as_object_mut()
        .expect("mcpServers must be object after normalization")
        .insert("bible".to_string(), server_entry);

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_mcp_config_adds_server_to_empty_object() {
        let merged = merge_mcp_config("{}").unwrap();
        assert_eq!(merged["mcpServers"]["bible"]["command"], "bible-mcp");
        assert_eq!(merged["mcpServers"]["bible"]["args"], json!(["serve"]));
    }

    #[test]
    fn merge_mcp_config_preserves_other_servers() {
        let merged = merge_mcp_config(
            r#"{"mcpServers":{"other":{"command":"demo","args":["serve"]}},"theme":"dark"}"#,
        )
        .unwrap();

        assert_eq!(merged["theme"], "dark");
        assert_eq!(merged["mcpServers"]["other"]["command"], "demo");
        assert_eq!(merged["mcpServers"]["bible"]["command"], "bible-mcp");
    }
}
