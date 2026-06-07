mod mcp_support;

use anyhow::Result;
use mcp_support::{call_tool_error, call_tool_json, seed_db, spawn_client};
use serde_json::{json, Value};

#[tokio::test]
async fn search_verses_returns_ranked_results() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), Some("1,0,0,0")).await?;

    let verses: Vec<Value> = call_tool_json(
        &client,
        "search_verses",
        json!({
            "query": "God",
            "limit": 2
        }),
    )
    .await?;

    assert_eq!(verses.len(), 2);
    assert_eq!(verses[0]["reference"], "John 3:16");
    assert!(verses[0].get("score").is_some());
    assert_eq!(verses[1]["reference"], "John 3:17");

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn search_verses_honors_book_filter() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), Some("1,0,0,0")).await?;

    let verses: Vec<Value> = call_tool_json(
        &client,
        "search_verses",
        json!({
            "query": "God",
            "limit": 5,
            "book": "John"
        }),
    )
    .await?;

    assert_eq!(verses.len(), 2);
    assert!(verses.iter().all(|verse| verse["book"] == "John"));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn search_verses_returns_error_for_unknown_book_filter() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), Some("1,0,0,0")).await?;

    let error = call_tool_error(
        &client,
        "search_verses",
        json!({
            "query": "God",
            "book": "Pizza"
        }),
    )
    .await?;

    assert!(error.contains("Unknown book"));

    client.cancel().await?;
    Ok(())
}
