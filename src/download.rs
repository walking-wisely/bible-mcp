use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MANIFEST_URL: &str = "https://pub-placeholder.r2.dev/manifest.json";

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub sha256: String,
    pub url: String,
}

pub async fn fetch_manifest() -> Result<Manifest> {
    let client = Client::new();
    let manifest: Manifest = client
        .get(MANIFEST_URL)
        .send()
        .await
        .context("fetching manifest")?
        .json()
        .await
        .context("parsing manifest JSON")?;
    Ok(manifest)
}

pub async fn download_db(dest: &Path, manifest: &Manifest) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let client = Client::new();
    let response = client
        .get(&manifest.url)
        .send()
        .await
        .context("starting database download")?;

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

    std::fs::write(dest, &bytes).context("writing database file")?;
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
