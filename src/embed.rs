use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static MODEL: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

// First call may need to download the model (~500 MB) over a network with no
// guarantee of a working timeout of its own, so it gets a generous ceiling.
// Once loaded, embedding a single query is a fixed-size ONNX forward pass and
// should never take anywhere near this long.
const MODEL_INIT_TIMEOUT: Duration = Duration::from_secs(300);
const EMBED_TIMEOUT: Duration = Duration::from_secs(30);

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
    if let Some(override_embedding) = std::env::var_os("BIBLE_MCP_TEST_EMBEDDING") {
        return parse_override_embedding(&override_embedding.to_string_lossy());
    }
    let mutex = model()?;
    let mut m = mutex
        .lock()
        .map_err(|_| anyhow::anyhow!("embed model mutex poisoned"))?;
    let mut results = m.embed(vec![text.to_string()], None)?;
    Ok(results.remove(0))
}

/// Embed a query on a blocking thread, bounded by a timeout so a stuck model
/// load or a hung download can't wedge the caller forever. On timeout the
/// blocking thread itself keeps running to completion in the background
/// (Tokio has no way to cancel it) — this only stops the caller from waiting
/// on it, it doesn't free the thread.
pub async fn embed_query_async(text: &str) -> Result<Vec<f32>> {
    let timeout = if MODEL.get().is_some() {
        EMBED_TIMEOUT
    } else {
        MODEL_INIT_TIMEOUT
    };
    let text = text.to_string();
    tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(move || embed_query(&text)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("embedding timed out after {timeout:?}"))?
    .context("embed task panicked")?
}

fn parse_override_embedding(value: &str) -> Result<Vec<f32>> {
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .with_context(|| format!("invalid test embedding value: {part}"))
        })
        .collect()
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
