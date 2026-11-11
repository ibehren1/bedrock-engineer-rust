//! Local knowledgebases: documents and crawled websites, chunked and embedded into one SQLite
//! file per knowledgebase (sqlite-vec for vectors, FTS5 for keywords). Agents attach them by
//! id and search them through the in-process MCP server they get when they attach any.

pub mod chunk;
pub mod crawl;
pub mod embed;
pub mod extract;
pub mod help;
pub mod html;
pub mod layout;
pub mod local;
pub mod mcp_server;
pub mod search;
pub mod service;
pub mod store;
pub mod types;
