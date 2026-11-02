mod mcp_support;

use anyhow::Result;
use mcp_support::{call_tool_error, call_tool_json, seed_db, spawn_client};
use rmcp::model::CallToolRequestParams;
use serde_json::{json, Value};

#[tokio::test]
async fn get_passage_returns_contiguous_verses() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let verses: Vec<Value> = call_tool_json(
        &client,
        "get_passage",
        json!({
            "book": "John",
            "chapter": 3,
            "fromVerse": 16,
            "toVerse": 17
        }),
    )
    .await?;

    assert_eq!(verses.len(), 2);
    assert_eq!(verses[0]["reference"], "John 3:16");
    assert_eq!(verses[1]["reference"], "John 3:17");
    assert!(verses.iter().all(|verse| verse.get("score").is_none()));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn get_passage_returns_error_for_unknown_book() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let error = call_tool_error(
        &client,
        "get_passage",
        json!({
            "book": "Pizza",
            "chapter": 3,
            "fromVerse": 16,
            "toVerse": 17
        }),
    )
    .await?;

    assert!(error.contains("Unknown book"));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn get_passage_requires_camel_case_range_fields() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let result = client
        .call_tool(
            CallToolRequestParams::new("get_passage").with_arguments(mcp_support::args(json!({
                "book": "John",
                "chapter": 3,
                "from_verse": 16,
                "to_verse": 17
            }))),
        )
        .await?;

    assert_eq!(result.is_error, Some(true));
    let message = result.content[0].as_text().unwrap().text.clone();
    assert!(message.contains("missing field"), "got: {message}");

    client.cancel().await?;
    Ok(())
}
