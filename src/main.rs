use bible_mcp::{config, download, embed, mcp_server};

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
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
    write_mcp_config()?;

    println!("\nSetup complete!");
    println!("Add the MCP server to your Claude Code config with:");
    println!(
        r#"  {{ "mcpServers": {{ "bible": {{ "command": "bible-mcp", "args": ["serve"] }} }} }}"#
    );
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
        println!("Update available (version {}). Downloading...", manifest.version);
        download::download_db(&cfg.db_path, &manifest).await?;
        println!("Update complete.");
    } else {
        println!("Database is up to date (version {}).", manifest.version);
    }
    Ok(())
}

fn write_mcp_config() -> Result<()> {
    // best-effort: write the MCP entry to Claude Code's config if we can find it
    let config_path = dirs::config_dir()
        .map(|d| d.join("Claude").join("claude_desktop_config.json"));

    if let Some(path) = config_path {
        if path.exists() {
            println!(
                "\nFound Claude config at {}. Add the entry manually if not already present.",
                path.display()
            );
        }
    }
    Ok(())
}
