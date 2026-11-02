use crate::mcp_server::BibleServer;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

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
    let app = axum::Router::new().nest_service(MCP_PATH, service);
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
