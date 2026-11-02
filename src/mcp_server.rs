use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

use crate::{books::resolve_book, db, embed};

#[derive(Debug, Clone)]
pub struct BibleServer {
    pub db_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl BibleServer {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchVersesArgs {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub book: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SimilarVersesArgs {
    pub book: String,
    pub chapter: u32,
    pub verse: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetVerseArgs {
    pub book: String,
    pub chapter: u32,
    pub verse: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPassageArgs {
    pub book: String,
    pub chapter: u32,
    #[serde(rename = "fromVerse")]
    pub from_verse: u32,
    #[serde(rename = "toVerse")]
    pub to_verse: u32,
}

fn default_limit() -> u32 {
    5
}

fn clamp_limit(n: u32) -> usize {
    n.clamp(1, 50) as usize
}

#[derive(Serialize)]
struct VerseResult {
    reference: String,
    book: String,
    chapter: u32,
    verse: u32,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
}

impl VerseResult {
    fn from_verse(v: &db::Verse, score: Option<f64>) -> Self {
        Self {
            reference: format!("{} {}:{}", v.book, v.chapter, v.verse),
            book: v.book.clone(),
            chapter: v.chapter,
            verse: v.verse,
            text: v.text.clone(),
            score: score.map(|s| (s * 10000.0).round() / 10000.0),
        }
    }
}

fn ok_json(value: impl Serialize) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&value)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn err_json(msg: impl std::fmt::Display) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&json!({ "error": msg.to_string() }))
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn anyhow_to_mcp(e: anyhow::Error) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

pub const MAX_QUERY_CHARS: usize = 200;

#[tool_router]
impl BibleServer {
    #[tool(
        description = "Hybrid semantic + keyword search over all ~31K Bible verses (World English Bible). Returns the most relevant verses for a topic, theme, concept, or phrase. Optionally filter by book."
    )]
    async fn search_verses(
        &self,
        Parameters(args): Parameters<SearchVersesArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args.query.chars().count() > MAX_QUERY_CHARS {
            return err_json(anyhow::anyhow!(
                "Query is too long: the limit is {MAX_QUERY_CHARS} characters."
            ));
        }
        let limit = clamp_limit(args.limit);

        let book_num = match &args.book {
            Some(b) => match resolve_book(b) {
                Ok((num, _)) => Some(num),
                Err(e) => return err_json(e),
            },
            None => None,
        };

        let embedding = embed::embed_query_async(&args.query)
            .await
            .map_err(anyhow_to_mcp)?;

        let hits = db::hybrid_search(self.db_path.clone(), embedding, args.query, limit, book_num)
            .await
            .map_err(anyhow_to_mcp)?;

        let ids: Vec<i64> = hits.iter().map(|&(id, _)| id).collect();
        let score_map: std::collections::HashMap<i64, f64> = hits.into_iter().collect();
        let verses = db::get_verses_by_ids(self.db_path.clone(), ids)
            .await
            .map_err(anyhow_to_mcp)?;
        let results: Vec<VerseResult> = verses
            .iter()
            .map(|v| VerseResult::from_verse(v, score_map.get(&v.id).copied()))
            .collect();

        ok_json(results)
    }

    #[tool(
        description = "Find verses similar to a given verse. Combines human-curated cross-references with vector similarity, so verses found both ways rank highest. Surfaces theologically intentional connections, not just word overlap."
    )]
    async fn similar_verses(
        &self,
        Parameters(args): Parameters<SimilarVersesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let limit = clamp_limit(args.limit);

        let (book_num, canonical_book) = match resolve_book(&args.book) {
            Ok(r) => r,
            Err(e) => return err_json(e),
        };

        let src = db::get_verse_by_ref(self.db_path.clone(), book_num, args.chapter, args.verse)
            .await
            .map_err(anyhow_to_mcp)?;
        let src = match src {
            Some(v) => v,
            None => {
                return err_json(format!(
                    "{} {}:{} not found.",
                    canonical_book, args.chapter, args.verse
                ))
            }
        };

        let xrefs = db::get_cross_refs(self.db_path.clone(), src.id, limit)
            .await
            .map_err(anyhow_to_mcp)?;

        let mut vec_hits = vec![];
        if let Some(emb) = db::get_verse_embedding(self.db_path.clone(), src.id)
            .await
            .map_err(anyhow_to_mcp)?
        {
            vec_hits = db::vector_search(self.db_path.clone(), emb, limit + 1, None)
                .await
                .map_err(anyhow_to_mcp)?;
            vec_hits.retain(|&(id, _)| id != src.id);
        }

        let merged: Vec<(i64, f64)> = db::rrf_merge(&xrefs, &vec_hits)
            .into_iter()
            .take(limit)
            .collect();
        let ids: Vec<i64> = merged.iter().map(|&(id, _)| id).collect();
        let rows_by_id: std::collections::HashMap<i64, db::Verse> =
            db::get_verses_by_ids(self.db_path.clone(), ids)
                .await
                .map_err(anyhow_to_mcp)?
                .into_iter()
                .map(|v| (v.id, v))
                .collect();

        let results: Vec<VerseResult> = merged
            .iter()
            .filter_map(|(id, score)| {
                rows_by_id
                    .get(id)
                    .map(|v| VerseResult::from_verse(v, Some(*score)))
            })
            .collect();

        ok_json(results)
    }

    #[tool(description = "Retrieve a single Bible verse by exact reference.")]
    async fn get_verse(
        &self,
        Parameters(args): Parameters<GetVerseArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (book_num, canonical_book) = match resolve_book(&args.book) {
            Ok(r) => r,
            Err(e) => return err_json(e),
        };

        match db::get_verse_by_ref(self.db_path.clone(), book_num, args.chapter, args.verse)
            .await
            .map_err(anyhow_to_mcp)?
        {
            Some(v) => ok_json(VerseResult::from_verse(&v, None)),
            None => err_json(format!(
                "{} {}:{} not found.",
                canonical_book, args.chapter, args.verse
            )),
        }
    }

    #[tool(description = "Retrieve a contiguous range of verses from one chapter.")]
    async fn get_passage(
        &self,
        Parameters(args): Parameters<GetPassageArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (book_num, _) = match resolve_book(&args.book) {
            Ok(r) => r,
            Err(e) => return err_json(e),
        };

        let verses = db::get_passage_by_ref(
            self.db_path.clone(),
            book_num,
            args.chapter,
            args.from_verse,
            args.to_verse,
        )
        .await
        .map_err(anyhow_to_mcp)?;

        ok_json(
            verses
                .iter()
                .map(|v| VerseResult::from_verse(v, None))
                .collect::<Vec<_>>(),
        )
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BibleServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("bible-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Search and retrieve Bible verses (World English Bible). \
                 Use search_verses for topics/themes, similar_verses for related passages, \
                 get_verse for a specific reference, get_passage for a range.",
            )
    }
}
