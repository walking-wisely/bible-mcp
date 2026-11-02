use anyhow::{Context, Result};
use fastembed::{
    EmbeddingModel, InitOptions, InitOptionsUserDefined, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use std::path::Path;
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
    // fastembed's default path (hf-hub download, then ort dlopen-searching for a
    // system ONNX Runtime library) has been observed to hang indefinitely on more
    // than one host, with no timeout anywhere in that call chain. BIBLE_MCP_MODEL_DIR
    // lets ops point at a pre-fetched model directory (model.onnx, tokenizer.json,
    // config.json, special_tokens_map.json, tokenizer_config.json) to skip it
    // entirely; pair with ORT_DYLIB_PATH pointing at an installed onnxruntime lib.
    let m = match std::env::var_os("BIBLE_MCP_MODEL_DIR") {
        Some(dir) => load_model_from_dir(Path::new(&dir))?,
        None => TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15).with_show_download_progress(true),
        )
        .context("failed to load fastembed nomic-embed-text model")?,
    };
    Ok(MODEL.get_or_init(|| Mutex::new(m)))
}

fn load_model_from_dir(dir: &Path) -> Result<TextEmbedding> {
    let read = |name: &str| -> Result<Vec<u8>> {
        std::fs::read(dir.join(name)).with_context(|| format!("reading {name} from {dir:?}"))
    };
    let user_model = UserDefinedEmbeddingModel::new(
        read("model.onnx")?,
        TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        },
    );
    TextEmbedding::try_new_from_user_defined(user_model, InitOptionsUserDefined::default())
        .with_context(|| format!("loading embedding model from {dir:?}"))
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
    if crate::embed_remote::enabled() {
        match crate::embed_remote::embed_query(text).await {
            Ok(v) => return Ok(v),
            Err(e) => eprintln!(
                "bible-mcp: Fireworks embedding failed ({e:#}); falling back to local model"
            ),
        }
    }
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
