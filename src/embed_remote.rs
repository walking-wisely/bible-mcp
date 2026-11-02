use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::Duration;

const ENDPOINT: &str = "https://api.fireworks.ai/inference/v1/embeddings";
const MODEL: &str = "nomic-ai/nomic-embed-text-v1.5";
pub(crate) const QUERY_PREFIX: &str = "search_query: ";
const EXPECTED_DIM: usize = 768;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const ERROR_BODY_LIMIT: usize = 200;

#[derive(Deserialize)]
struct Response {
    data: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    embedding: Vec<f32>,
}

pub fn enabled() -> bool {
    api_key().is_some()
}

fn api_key() -> Option<String> {
    std::env::var("FIREWORKS_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

fn client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let built = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("building Fireworks HTTP client")?;
    Ok(CLIENT.get_or_init(|| built))
}

pub async fn embed_query(text: &str) -> Result<Vec<f32>> {
    let key = api_key().context("FIREWORKS_API_KEY is not set")?;
    embed_with(client()?, ENDPOINT, &key, text).await
}

async fn embed_with(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    text: &str,
) -> Result<Vec<f32>> {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": MODEL,
        "input": format!("{QUERY_PREFIX}{text}"),
    }))?;
    let response = client
        .post(url)
        .bearer_auth(key)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .context("sending Fireworks embedding request")?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .context("reading Fireworks embedding response")?;
    if !status.is_success() {
        let snippet: String = String::from_utf8_lossy(&bytes)
            .chars()
            .take(ERROR_BODY_LIMIT)
            .collect();
        bail!("Fireworks returned {status}: {snippet}");
    }
    let parsed: Response =
        serde_json::from_slice(&bytes).context("parsing Fireworks embedding response")?;
    let [item] = <[Item; 1]>::try_from(parsed.data)
        .map_err(|d| anyhow::anyhow!("expected 1 embedding, got {}", d.len()))?;
    if item.embedding.len() != EXPECTED_DIM {
        bail!(
            "expected {EXPECTED_DIM}-dim embedding, got {}",
            item.embedding.len()
        );
    }
    l2_normalized(item.embedding)
}

fn l2_normalized(mut v: Vec<f32>) -> Result<Vec<f32>> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm == 0.0 {
        bail!("embedding has an invalid norm");
    }
    v.iter_mut().for_each(|x| *x /= norm);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn request_complete(buf: &[u8]) -> bool {
        let text = String::from_utf8_lossy(buf);
        let Some(header_end) = text.find("\r\n\r\n") else {
            return false;
        };
        let content_length = text[..header_end]
            .lines()
            .find_map(|line| {
                let line = line.to_ascii_lowercase();
                line.strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        buf.len() >= header_end + 4 + content_length
    }

    async fn serve_once(
        status: &'static str,
        body: String,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/embeddings", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).await.unwrap();
                received.extend_from_slice(&chunk[..n]);
                if n == 0 || request_complete(&received) {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        (url, handle)
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }

    fn body_with(embedding: Vec<f32>) -> String {
        serde_json::json!({ "data": [{ "embedding": embedding }] }).to_string()
    }

    #[tokio::test]
    async fn returns_unit_vector_and_sends_model_prefix_and_key() {
        let mut raw = vec![0.0f32; EXPECTED_DIM];
        raw[0] = 3.0;
        raw[1] = 4.0;
        let (url, request) = serve_once("200 OK", body_with(raw)).await;

        let v = embed_with(&client(), &url, "secret-key", "love your enemies")
            .await
            .unwrap();

        assert_eq!(v.len(), EXPECTED_DIM);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
        let sent = request.await.unwrap();
        let sent_lower = sent.to_ascii_lowercase();
        assert!(sent_lower.contains("authorization: bearer secret-key"));
        assert!(sent.contains("nomic-ai/nomic-embed-text-v1.5"));
        assert!(sent.contains("search_query: love your enemies"));
    }

    #[tokio::test]
    async fn rejects_wrong_dimension() {
        let (url, _) = serve_once("200 OK", body_with(vec![1.0; 512])).await;

        let err = embed_with(&client(), &url, "k", "q").await.unwrap_err();

        assert!(format!("{err:#}").contains("768"));
    }

    #[tokio::test]
    async fn surfaces_http_error_status_without_leaking_the_key() {
        let (url, _) = serve_once("401 Unauthorized", r#"{"error":"bad key"}"#.to_string()).await;

        let err = embed_with(&client(), &url, "secret-key", "q")
            .await
            .unwrap_err();

        let msg = format!("{err:#}");
        assert!(msg.contains("401"));
        assert!(!msg.contains("secret-key"));
    }

    #[tokio::test]
    async fn rejects_zero_vector() {
        let (url, _) = serve_once("200 OK", body_with(vec![0.0; EXPECTED_DIM])).await;

        assert!(embed_with(&client(), &url, "k", "q").await.is_err());
    }

    #[tokio::test]
    async fn rejects_empty_data() {
        let (url, _) = serve_once("200 OK", r#"{"data":[]}"#.to_string()).await;

        assert!(embed_with(&client(), &url, "k", "q").await.is_err());
    }
}
