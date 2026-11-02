use crate::mcp_server::BibleServer;
use axum::response::IntoResponse;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);
pub const MAX_IN_FLIGHT: usize = 100;

fn with_in_flight_limit(router: axum::Router, max: usize) -> axum::Router {
    let permits = Arc::new(tokio::sync::Semaphore::new(max));
    router.layer(axum::middleware::from_fn_with_state(
        permits,
        limit_in_flight,
    ))
}

async fn limit_in_flight(
    axum::extract::State(permits): axum::extract::State<Arc<tokio::sync::Semaphore>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let Ok(_permit) = permits.try_acquire() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            [(axum::http::header::RETRY_AFTER, "1")],
            "server is busy, try again shortly",
        )
            .into_response();
    };
    next.run(request).await
}

pub const MCP_PATH: &str = "/mcp";

pub async fn serve(
    bind: SocketAddr,
    db_path: PathBuf,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    if !db_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "database not found at {}; run `bible-mcp setup` or set BIBLE_MCP_DB_PATH",
                db_path.display()
            ),
        ));
    }
    let service = StreamableHttpService::new(
        move || Ok(BibleServer::new(db_path.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let app = with_in_flight_limit(
        axum::Router::new().nest_service(MCP_PATH, service),
        MAX_IN_FLIGHT,
    );
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let (signalled_tx, signalled_rx) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown.await;
        let _ = signalled_tx.send(());
    });
    // Open SSE streams never finish on their own, so graceful shutdown alone can hang forever.
    tokio::select! {
        result = server => result,
        _ = async {
            let _ = signalled_rx.await;
            tokio::time::sleep(SHUTDOWN_GRACE).await;
        } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn requests_over_the_limit_get_503_with_retry_after() {
        let release = Arc::new(Notify::new());
        let held = release.clone();
        let router = axum::Router::new().route(
            "/slow",
            axum::routing::get(move || {
                let held = held.clone();
                async move {
                    held.notified().await;
                    "done"
                }
            }),
        );
        let app = with_in_flight_limit(router, 1);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("http://{addr}/slow");
        let first = tokio::spawn({
            let (client, url) = (client.clone(), url.clone());
            async move { client.get(url).send().await.unwrap().status() }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;

        let second = client.get(&url).send().await.unwrap();
        assert_eq!(second.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(second.headers()["retry-after"], "1");

        release.notify_one();
        assert!(first.await.unwrap().is_success());
        release.notify_one();
        let third = tokio::spawn(async move { client.get(url).send().await.unwrap().status() });
        tokio::time::sleep(Duration::from_millis(100)).await;
        release.notify_one();
        assert!(third.await.unwrap().is_success());
    }
}
