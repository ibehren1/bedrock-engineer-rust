//! Local knowledgebases for agents: documents and crawled websites, chunked and embedded into
//! one SQLite file per knowledgebase (sqlite-vec for vectors, FTS5 for keywords), searched by
//! the in-process MCP server the agent gets when it has knowledgebases.

pub mod chunk;
pub mod crawl;
pub mod embed;
pub mod extract;
pub mod help;
pub mod html;
pub mod local;
pub mod mcp_server;
pub mod search;
pub mod service;
pub mod store;
pub mod types;
