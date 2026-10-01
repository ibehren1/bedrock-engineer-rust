//! MCP (Model Context Protocol) client support — port of `src/main/mcp/**` and
//! `src/common/mcp/**`.
//!
//! - [`client::McpClient`]: one server over stdio (child process), Streamable HTTP, or legacy
//!   SSE (tried when Streamable HTTP fails), built on `rmcp`.
//! - [`manager::McpManager`]: the per-agent client pool (`initMcpFromAgentConfig`,
//!   `getMcpToolSpecs`, `tryExecuteMcpTool`, connection tests, cleanup).
//! - [`adapter`]: the `mcp` tool adapter (routing a tool use to the agent's servers).
//! - [`registry`]: the official MCP Registry search and config builders.
//! - [`schemas`], [`utils`], [`command_resolver`]: config validation and helpers.
//!
//! MCP tool names are passed through unchanged (no prefix); the legacy `mcp_` prefix is stripped
//! (see `common::tool_names`).

pub mod adapter;
pub mod client;
pub mod command_resolver;
pub mod manager;
pub mod registry;
pub mod schemas;
pub mod sse;
pub mod utils;

pub use client::McpClient;
pub use manager::McpManager;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Mcp(String),
    #[error("{0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
