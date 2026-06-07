mod mcp_support;

use anyhow::Result;
use mcp_support::{call_tool_error, call_tool_json, seed_db, spawn_client};
use serde_json::{json, Value};

#[tokio::test]
async fn similar_verses_prefers_cross_reference_hits() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let verses: Vec<Value> = call_tool_json(
        &client,
        "similar_verses",
        json!({
            "book": "John",
            "chapter": 3,
            "verse": 16,
            "limit": 2
        }),
    )
    .await?;

    assert_eq!(verses.len(), 2);
    assert_eq!(verses[0]["reference"], "Romans 8:28");
    assert_eq!(verses[0]["score"], 0.95);
    assert_eq!(verses[1]["reference"], "John 3:17");
    assert!(verses[1].get("score").is_none());

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn similar_verses_returns_error_for_unknown_book() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let error = call_tool_error(
        &client,
        "similar_verses",
        json!({
            "book": "Pizza",
            "chapter": 3,
            "verse": 16
        }),
    )
    .await?;

    assert!(error.contains("Unknown book"));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn similar_verses_returns_error_for_missing_source_verse() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let error = call_tool_error(
        &client,
        "similar_verses",
        json!({
            "book": "John",
            "chapter": 9,
            "verse": 99
        }),
    )
    .await?;

    assert_eq!(error, "John 9:99 not found.");

    client.cancel().await?;
    Ok(())
}
