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
    /// Start the MCP server over streamable HTTP
    ServeHttp {
        /// Address to listen on. Defaults to loopback; put a reverse proxy in front for public access.
        #[arg(long, default_value = "127.0.0.1:8000")]
        bind: std::net::SocketAddr,
    },
    /// Show current configuration and status
    Status,
    /// Re-check the manifest and update the database if a new version exists
    Update,
    /// Run one or more embeddings and print per-call timing, for load testing
    BenchEmbed {
        #[arg(default_value = "In the beginning God created the heavens and the earth")]
        text: String,
        /// Number of concurrent embed calls to run within this process
        #[arg(short, long, default_value_t = 1)]
        concurrency: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    bible_mcp::db::init_sqlite_vec();
    let cli = Cli::parse();
    match cli.command {
        Command::Setup => cmd_setup().await,
        Command::Serve => cmd_serve().await,
        Command::ServeHttp { bind } => cmd_serve_http(bind).await,
        Command::Status => cmd_status(),
        Command::Update => cmd_update().await,
        Command::BenchEmbed { text, concurrency } => cmd_bench_embed(text, concurrency).await,
    }
}

async fn cmd_setup() -> Result<()> {
    let cfg = config::load()?;
    let db_path = cfg.db_path.clone();

    println!("Setting up bible-mcp...");
    println!("Database will be stored at: {}", db_path.display());

    download::ensure_db(&db_path).await?;

    println!("Pre-loading embedding model (this may take a minute on first run)...");
    embed::embed_query_async("warm up").await?;

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
    let server = mcp_server::BibleServer::new(cfg.db_path);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn cmd_serve_http(bind: std::net::SocketAddr) -> Result<()> {
    let cfg = config::load()?;
    eprintln!(
        "bible-mcp: serving MCP over streamable HTTP at http://{bind}{}",
        bible_mcp::http::MCP_PATH
    );
    bible_mcp::http::serve(bind, cfg.db_path, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
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

async fn cmd_bench_embed(text: String, concurrency: usize) -> Result<()> {
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..concurrency.max(1) {
        let text = text.clone();
        set.spawn(async move {
            let start = std::time::Instant::now();
            let result = tokio::task::spawn_blocking(move || embed::embed_query(&text))
                .await
                .map_err(|e| anyhow::anyhow!("join: {}", e))
                .and_then(|r| r);
            (start.elapsed(), result)
        });
    }
    while let Some(joined) = set.join_next().await {
        let (elapsed, embed_result) = joined.map_err(|e| anyhow::anyhow!("join: {}", e))?;
        let embedding = embed_result?;
        println!("{:.1} {}", elapsed.as_secs_f64() * 1000.0, embedding.len());
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
