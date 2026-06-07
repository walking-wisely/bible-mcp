#![allow(dead_code)]

use anyhow::Result;
use bible_mcp::db;
use rmcp::{
    model::{CallToolRequestParam, CallToolResult},
    service::RunningService,
    transport::TokioChildProcess,
    RoleClient, ServiceExt,
};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tempfile::NamedTempFile;

pub fn seed_db() -> NamedTempFile {
    db::init_sqlite_vec();
    let file = NamedTempFile::new().unwrap();
    let conn = Connection::open(file.path()).unwrap();

    conn.execute_batch(
        "
        CREATE TABLE verses (
          id INTEGER PRIMARY KEY,
          book TEXT NOT NULL,
          book_num INTEGER NOT NULL,
          chapter INTEGER NOT NULL,
          verse INTEGER NOT NULL,
          text TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE verse_embeddings USING vec0(embedding float[4]);
        CREATE VIRTUAL TABLE verses_fts USING fts5(text, content=verses, content_rowid=id);
        CREATE TABLE cross_references (
          from_verse_id INTEGER NOT NULL,
          to_verse_id INTEGER NOT NULL,
          weight REAL NOT NULL,
          PRIMARY KEY (from_verse_id, to_verse_id)
        );
        ",
    )
    .unwrap();

    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (1, 'John', 43, 3, 16, 'For God so loved the world')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (2, 'Romans', 45, 8, 28, 'All things work together for good')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses (id, book, book_num, chapter, verse, text) VALUES (3, 'John', 43, 3, 17, 'For God sent his Son into the world')",
        [],
    )
    .unwrap();

    let e1: Vec<u8> = [1.0f32, 0.0, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    let e2: Vec<u8> = [0.0f32, 1.0, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    let e3: Vec<u8> = [0.9f32, 0.1, 0.0, 0.0]
        .iter()
        .flat_map(|f: &f32| f.to_le_bytes())
        .collect();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (1, ?1)",
        rusqlite::params![e1],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (2, ?1)",
        rusqlite::params![e2],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verse_embeddings (rowid, embedding) VALUES (3, ?1)",
        rusqlite::params![e3],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (1, 'For God so loved the world')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (2, 'All things work together for good')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO verses_fts(rowid, text) VALUES (3, 'For God sent his Son into the world')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO cross_references (from_verse_id, to_verse_id, weight) VALUES (1, 2, 0.95)",
        [],
    )
    .unwrap();

    file
}

pub async fn spawn_client(
    db_path: &std::path::Path,
    embedding_override: Option<&str>,
) -> Result<RunningService<RoleClient, ()>> {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_bible-mcp"));
    command.arg("serve").env("BIBLE_MCP_DB_PATH", db_path);
    if let Some(embedding) = embedding_override {
        command.env("BIBLE_MCP_TEST_EMBEDDING", embedding);
    }

    let transport = TokioChildProcess::new(&mut command)?;
    let client = ().serve(transport).await?;
    Ok(client)
}

pub fn args(value: Value) -> Map<String, Value> {
    serde_json::from_value(value).unwrap()
}

pub fn parse_tool_result<T: DeserializeOwned>(result: CallToolResult) -> T {
    let text = result
        .content
        .first()
        .and_then(|content| content.raw.as_text())
        .map(|text| text.text.clone())
        .expect("tool result should contain text JSON");
    serde_json::from_str(&text).expect("tool result text should be valid JSON")
}

pub fn parse_error_message(result: CallToolResult) -> String {
    let value: Value = parse_tool_result(result);
    value["error"]
        .as_str()
        .expect("tool error payload should contain a string error field")
        .to_string()
}

pub async fn call_tool_json<T: DeserializeOwned>(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> Result<T> {
    let result = client
        .call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments: Some(args(arguments)),
        })
        .await?;
    Ok(parse_tool_result(result))
}

pub async fn call_tool_error(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> Result<String> {
    let result = client
        .call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments: Some(args(arguments)),
        })
        .await?;
    Ok(parse_error_message(result))
}
