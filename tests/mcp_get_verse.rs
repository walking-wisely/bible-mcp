mod mcp_support;

use anyhow::Result;
use mcp_support::{call_tool_error, call_tool_json, seed_db, spawn_client};
use serde_json::{json, Value};

#[tokio::test]
async fn get_verse_returns_expected_json_shape() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let verse: Value = call_tool_json(
        &client,
        "get_verse",
        json!({
            "book": "John",
            "chapter": 3,
            "verse": 16
        }),
    )
    .await?;

    assert_eq!(verse["reference"], "John 3:16");
    assert_eq!(verse["book"], "John");
    assert_eq!(verse["chapter"], 3);
    assert_eq!(verse["verse"], 16);
    assert_eq!(verse["text"], "For God so loved the world");
    assert!(verse.get("score").is_none());

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn get_verse_returns_error_for_unknown_book() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let error = call_tool_error(
        &client,
        "get_verse",
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
async fn get_verse_returns_error_for_missing_reference() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let error = call_tool_error(
        &client,
        "get_verse",
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
