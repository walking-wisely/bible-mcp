use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Mutex, OnceLock};

static MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

fn model() -> Result<&'static Mutex<TextEmbedding>> {
    if let Some(m) = MODEL.get() {
        return Ok(m);
    }
    let m = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
    )
    .context("failed to load fastembed nomic-embed-text model")?;
    Ok(MODEL.get_or_init(|| Mutex::new(m)))
}

/// Embed a single query string and return a `Vec<f32>` of length 768.
pub fn embed_query(text: &str) -> Result<Vec<f32>> {
    let mutex = model()?;
    let mut m = mutex.lock().map_err(|_| anyhow::anyhow!("embed model mutex poisoned"))?;
    let mut results = m.embed(vec![text.to_string()], None)?;
    Ok(results.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads model (~130 MB) on first run"]
    fn embed_returns_768_dims() {
        let v = embed_query("For God so loved the world").unwrap();
        assert_eq!(v.len(), 768);
    }
}
