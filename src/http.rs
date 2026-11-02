use crate::mcp_server::BibleServer;
use rmcp::transport::sse_server::SseServer;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;

pub async fn serve(
    bind: SocketAddr,
    db_path: PathBuf,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    let ct = SseServer::serve(bind)
        .await?
        .with_service(move || BibleServer {
            db_path: db_path.clone(),
        });
    shutdown.await;
    ct.cancel();
    Ok(())
}
