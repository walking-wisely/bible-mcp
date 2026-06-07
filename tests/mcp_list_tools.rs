mod mcp_support;

use anyhow::Result;
use mcp_support::{seed_db, spawn_client};

#[tokio::test]
async fn lists_all_four_tools() -> Result<()> {
    let db = seed_db();
    let client = spawn_client(db.path(), None).await?;

    let tools = client.list_all_tools().await?;
    let mut names: Vec<_> = tools
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "get_passage",
            "get_verse",
            "search_verses",
            "similar_verses"
        ]
    );

    client.cancel().await?;
    Ok(())
}
