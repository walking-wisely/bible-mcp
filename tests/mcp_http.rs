use bible_mcp::http;
use serde_json::{json, Value};
use std::net::{SocketAddr, TcpListener};
use std::time::Duration;
use tokio::sync::oneshot;

fn free_addr() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}

async fn post(
    client: &reqwest::Client,
    url: &str,
    session: Option<&str>,
    body: Value,
) -> reqwest::Response {
    let mut last_err = None;
    for _ in 0..50 {
        let mut req = client
            .post(url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body.to_string());
        if let Some(id) = session {
            req = req.header("mcp-session-id", id);
        }
        match req.send().await {
            Ok(resp) => return resp,
            Err(e) => last_err = Some(e),
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("server did not start: {last_err:?}");
}

fn json_from_body(body: &str) -> Value {
    let payload = body
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .find(|d| !d.trim().is_empty())
        .unwrap_or(body);
    serde_json::from_str(payload.trim()).unwrap_or_else(|e| panic!("bad body {body:?}: {e}"))
}

#[tokio::test]
async fn initialize_and_list_tools_over_http() {
    let addr = free_addr();
    let url = format!("http://{addr}{}", http::MCP_PATH);
    let (stop, stopped) = oneshot::channel::<()>();
    let db = std::env::temp_dir().join(format!("bible-mcp-http-test-{}.db", addr.port()));
    std::fs::write(&db, b"").unwrap();
    let server = tokio::spawn(http::serve(addr, db.clone(), async {
        let _ = stopped.await;
    }));
    let client = reqwest::Client::builder().no_proxy().build().unwrap();

    let init = post(
        &client,
        &url,
        None,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}
            }
        }),
    )
    .await;
    assert!(init.status().is_success(), "status {}", init.status());
    let session = init
        .headers()
        .get("mcp-session-id")
        .map(|v| v.to_str().unwrap().to_string());
    let body = json_from_body(&init.text().await.unwrap());
    assert_eq!(body["result"]["serverInfo"]["name"], "bible-mcp");

    if let Some(id) = &session {
        post(
            &client,
            &url,
            Some(id),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
        .await;
    }
    let tools = post(
        &client,
        &url,
        session.as_deref(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    )
    .await;
    assert!(tools.status().is_success(), "status {}", tools.status());
    let body = json_from_body(&tools.text().await.unwrap());
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"search_verses"), "got {names:?}");

    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn serve_refuses_to_start_without_a_database() {
    let missing = std::env::temp_dir().join("bible-mcp-definitely-missing.db");
    let err = http::serve(free_addr(), missing, std::future::pending())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
