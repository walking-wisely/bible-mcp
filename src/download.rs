use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MANIFEST_URL_VAR: &str = "BIBLE_MCP_MANIFEST_URL";

fn manifest_url() -> Result<String> {
    std::env::var(MANIFEST_URL_VAR)
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| option_env!("BIBLE_MCP_MANIFEST_URL").map(str::to_owned))
        .filter(|v| !v.is_empty())
        .with_context(|| {
            format!("no database manifest URL configured; set {MANIFEST_URL_VAR} to the manifest.json URL")
        })
}

fn http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900))
        .build()
        .context("building HTTP client")
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub sha256: String,
    pub url: String,
}

pub async fn fetch_manifest() -> Result<Manifest> {
    let manifest: Manifest = http_client()?
        .get(manifest_url()?)
        .send()
        .await
        .context("fetching manifest")?
        .error_for_status()
        .context("manifest request failed")?
        .json()
        .await
        .context("parsing manifest JSON")?;
    Ok(manifest)
}

pub async fn download_db(dest: &Path, manifest: &Manifest) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let response = http_client()?
        .get(&manifest.url)
        .send()
        .await
        .context("starting database download")?
        .error_for_status()
        .context("database download failed")?;

    let total = response.content_length();
    let pb = ProgressBar::new(total.unwrap_or(0));
    pb.set_style(
        ProgressStyle::with_template("{msg} [{bar:40}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message("Downloading bible.db");

    let bytes = response.bytes().await.context("reading download body")?;
    pb.set_position(bytes.len() as u64);
    pb.finish_with_message("Download complete");

    let digest = format!("{:x}", Sha256::digest(&bytes));
    if digest != manifest.sha256 {
        bail!(
            "SHA-256 mismatch: expected {}, got {}",
            manifest.sha256,
            digest
        );
    }

    let tmp = dest.with_extension("download");
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, dest)
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).context("writing database file");
    }
    Ok(())
}

pub fn db_needs_update(db_path: &Path, manifest: &Manifest) -> bool {
    if !db_path.exists() {
        return true;
    }
    match std::fs::read(db_path) {
        Ok(bytes) => format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256,
        Err(_) => true,
    }
}

pub async fn ensure_db(db_path: &Path) -> Result<PathBuf> {
    let manifest = fetch_manifest().await?;
    if db_needs_update(db_path, &manifest) {
        println!("Downloading database (version {})...", manifest.version);
        download_db(db_path, &manifest).await?;
    }
    Ok(db_path.to_path_buf())
}
