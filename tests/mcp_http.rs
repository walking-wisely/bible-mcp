use bible_mcp::http;
use std::net::{SocketAddr, TcpListener};
use std::time::Duration;
use tokio::sync::oneshot;

fn free_addr() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

async fn connect(url: &str) -> reqwest::Response {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for _ in 0..50 {
        if let Ok(resp) = client.get(url).send().await {
            return resp;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("server did not start");
}

#[tokio::test]
async fn sse_endpoint_announces_message_path() {
    let addr = free_addr();
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(http::serve(addr, "unused.db".into(), async {
        let _ = stopped.await;
    }));

    let mut resp = connect(&format!("http://{addr}/sse")).await;
    assert!(resp.status().is_success());
    let chunk = tokio::time::timeout(Duration::from_secs(5), resp.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.contains("event: endpoint"), "got: {text}");
    assert!(text.contains("/message?sessionId="), "got: {text}");

    drop(resp);
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}
