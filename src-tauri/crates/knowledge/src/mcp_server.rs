//! The built-in MCP server that gives an agent its local knowledgebases.
//!
//! It runs inside the app: [`KnowledgeMcp`] is the `mcp` crate's [`BuiltinServers`] provider,
//! and each connection is an in-memory duplex stream with a [`KbMcpServer`] for one agent on
//! the other end. Tools:
//!
//! * `searchKnowledgeBase` — hybrid search over the agent's knowledgebases, results with their
//!   source (file path or page URL) so the model can cite them.
//! * `listKnowledgeBases` — names, descriptions and sizes.
//! * `listKnowledgeBaseSources` — the files, folders and websites in a knowledgebase.

use crate::service::KnowledgeService;
use crate::types::{SearchHit, SourceKind};
use common::agent::McpServerConfig;
use mcp::BuiltinServers;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, InitializeResult, JsonObject, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use serde_json::Value;
use std::sync::Arc;

pub const SEARCH_TOOL: &str = "searchKnowledgeBase";
pub const LIST_TOOL: &str = "listKnowledgeBases";
pub const SOURCES_TOOL: &str = "listKnowledgeBaseSources";

/// Longest text returned per search result.
const MAX_RESULT_CHARS: usize = 4000;

/// Tool definitions (`name`, `description`, `inputSchema`). The renderer has the same list in
/// `src/common/agents/knowledgeTools.ts` (checked by its test).
const TOOLS_JSON: &str = include_str!("../data/tools.json");

pub fn tools() -> Vec<Tool> {
    let list: Vec<Value> = serde_json::from_str(TOOLS_JSON).expect("data/tools.json is valid");
    list.into_iter()
        .map(|t| {
            Tool::new(
                t["name"].as_str().unwrap_or_default().to_string(),
                t["description"].as_str().unwrap_or_default().to_string(),
                Arc::new(t["inputSchema"].as_object().cloned().unwrap_or_default()),
            )
        })
        .collect()
}

/// One agent's view of the knowledgebases.
#[derive(Clone)]
pub struct KbMcpServer {
    svc: Arc<KnowledgeService>,
    agent_id: String,
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{cut}…")
}

/// Search results as text for the model.
pub fn format_hits(query: &str, hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return format!("No passages in the knowledge base match \"{query}\".");
    }
    let mut out = format!("Found {} passages for \"{query}\":\n", hits.len());
    for (i, h) in hits.iter().enumerate() {
        let mut head = h.title.clone();
        if !h.heading_path.is_empty() {
            head.push_str(" › ");
            head.push_str(&h.heading_path);
        }
        out.push_str(&format!(
            "\n[{}] {head}\nSource: {}\nKnowledge base: {}\n\n{}\n",
            i + 1,
            h.uri,
            h.kb_name,
            truncate(&h.text, MAX_RESULT_CHARS)
        ));
    }
    out
}

impl KbMcpServer {
    pub fn new(svc: Arc<KnowledgeService>, agent_id: impl Into<String>) -> Self {
        Self {
            svc,
            agent_id: agent_id.into(),
        }
    }

    fn select_kb(&self, sel: Option<&str>) -> Result<Vec<crate::types::KbInfo>, String> {
        let mut kbs = self.svc.list(&self.agent_id)?;
        if let Some(sel) = sel.map(str::trim).filter(|s| !s.is_empty()) {
            kbs.retain(|k| k.id == sel || k.name.eq_ignore_ascii_case(sel));
            if kbs.is_empty() {
                return Err(format!("No knowledge base named \"{sel}\""));
            }
        }
        Ok(kbs)
    }

    /// Run a tool; `Err` is a tool-level error shown to the model.
    pub async fn run(&self, name: &str, args: &JsonObject) -> Result<String, String> {
        let str_arg = |k: &str| args.get(k).and_then(Value::as_str);
        match name {
            SEARCH_TOOL => {
                let query = str_arg("query").unwrap_or_default();
                let top_k = args
                    .get("topK")
                    .and_then(Value::as_u64)
                    .map(|k| k.clamp(1, 20) as usize);
                let hits = self
                    .svc
                    .search(
                        &self.agent_id,
                        str_arg("knowledgeBase"),
                        query,
                        top_k,
                        crate::types::SearchOrigin::Agent,
                    )
                    .await?;
                Ok(format_hits(query.trim(), &hits))
            }
            LIST_TOOL => {
                let kbs = self.select_kb(None)?;
                if kbs.is_empty() {
                    return Ok("This agent has no knowledge bases.".into());
                }
                let mut out = String::new();
                for k in kbs {
                    out.push_str(&format!(
                        "- {} (id: {}): {} sources, {} passages{}\n",
                        k.name,
                        k.id,
                        k.source_count,
                        k.chunk_count,
                        if k.description.is_empty() {
                            String::new()
                        } else {
                            format!(". {}", k.description)
                        }
                    ));
                }
                Ok(out)
            }
            SOURCES_TOOL => {
                let mut out = String::new();
                for k in self.select_kb(str_arg("knowledgeBase"))? {
                    out.push_str(&format!("{}:\n", k.name));
                    let sources = self.svc.list_sources(&self.agent_id, &k.id)?;
                    if sources.is_empty() {
                        out.push_str("  (no sources)\n");
                    }
                    for s in sources {
                        let kind = match s.kind {
                            SourceKind::File => "file",
                            SourceKind::Folder => "folder",
                            SourceKind::Url => "website",
                        };
                        out.push_str(&format!(
                            "  - {} [{kind}] {} ({} documents, status {})\n",
                            s.title,
                            s.uri,
                            s.document_count,
                            s.status.as_str()
                        ));
                    }
                }
                Ok(out)
            }
            other => Err(format!("Unknown tool: {other}")),
        }
    }
}

impl ServerHandler for KbMcpServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                common::agent::LOCAL_KNOWLEDGE_BASE_SERVER,
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Search the agent's local knowledge bases.")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request.arguments.unwrap_or_default();
        let result = match self.run(&request.name, &args).await {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        };
        Ok(result.into())
    }
}

/// [`BuiltinServers`] for the local knowledgebase server.
pub struct KnowledgeMcp {
    pub svc: Arc<KnowledgeService>,
    pub runtime: tokio::runtime::Handle,
}

/// Buffer of the in-memory connection.
const DUPLEX_BUFFER: usize = 256 * 1024;

impl BuiltinServers for KnowledgeMcp {
    fn connect(&self, server: &McpServerConfig) -> mcp::Result<tokio::io::DuplexStream> {
        if server.name != common::agent::LOCAL_KNOWLEDGE_BASE_SERVER {
            return Err(mcp::Error::Mcp(format!(
                "Unknown built-in MCP server: {}",
                server.name
            )));
        }
        let agent_id = server
            .agent_id
            .clone()
            .filter(|a| common::json_file::is_safe_file_id(a))
            .ok_or_else(|| mcp::Error::Mcp("Built-in server is missing a valid agentId".into()))?;
        let (client_end, server_end) = tokio::io::duplex(DUPLEX_BUFFER);
        let handler = KbMcpServer::new(self.svc.clone(), agent_id);
        self.runtime.spawn(async move {
            match handler.serve(server_end).await {
                Ok(running) => {
                    let _ = running.waiting().await;
                }
                Err(e) => tracing::warn!(error = %e, "Knowledge base MCP server failed to start"),
            }
        });
        Ok(client_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::fake::FakeProvider;
    use crate::service::KnowledgeHost;
    use crate::types::KnowledgeSettings;
    use mcp::McpManager;
    use serde_json::json;

    struct Host;
    impl KnowledgeHost for Host {
        fn settings(&self) -> KnowledgeSettings {
            let mut s = KnowledgeSettings::default();
            s.embedding.dimensions = 256;
            s
        }
        fn http_client(&self) -> reqwest::Client {
            reqwest::Client::new()
        }
        fn emit(&self, _: &str, _: Value) {}
    }

    #[test]
    fn tool_names_match_constants() {
        let names: Vec<String> = tools().iter().map(|t| t.name.to_string()).collect();
        assert_eq!(names, vec![SEARCH_TOOL, LIST_TOOL, SOURCES_TOOL]);
        assert!(tools()[0].input_schema.contains_key("properties"));
    }

    #[test]
    fn formats_hits() {
        assert!(format_hits("x", &[]).starts_with("No passages"));
        let hit = SearchHit {
            kb_id: "k".into(),
            kb_name: "Docs".into(),
            source_id: "s".into(),
            title: "Guide".into(),
            uri: "https://x/guide".into(),
            heading_path: "Install".into(),
            text: "Run it.".into(),
            score: 1.0,
        };
        let s = format_hits("install", &[hit]);
        assert!(
            s.contains(
                "[1] Guide › Install\nSource: https://x/guide\nKnowledge base: Docs\n\nRun it."
            ),
            "{s}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn manager_reaches_builtin_server() {
        let dir = tempfile::tempdir().unwrap();
        let svc = KnowledgeService::new(
            dir.path().to_path_buf(),
            Arc::new(Host),
            Arc::new(FakeProvider),
            tokio::runtime::Handle::current(),
        );
        let kb = svc.create("agent-1", "Notes", "my notes").unwrap();
        let f = dir.path().join("n.md");
        std::fs::write(&f, "The launch code is in the blue folder.").unwrap();
        svc.add_files("agent-1", &kb.id, &[f.to_string_lossy().into_owned()])
            .unwrap();
        for _ in 0..500 {
            if svc.jobs(None).is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let manager = McpManager::default();
        manager.set_builtin(Arc::new(KnowledgeMcp {
            svc: svc.clone(),
            runtime: tokio::runtime::Handle::current(),
        }));
        let servers = common::agent::effective_mcp_servers(&json!({
            "id": "agent-1", "localKnowledgeBases": [{ "id": kb.id, "name": "Notes" }]
        }));
        let specs = manager.get_tool_specs(Some(&servers)).await.unwrap();
        let names: Vec<&str> = specs
            .iter()
            .map(|s| s["toolSpec"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec![SEARCH_TOOL, LIST_TOOL, SOURCES_TOOL]);

        let r = manager
            .execute_tool(
                SEARCH_TOOL,
                &json!({ "query": "launch code" }),
                Some(&servers),
            )
            .await;
        assert!(r.success, "{r:?}");
        let text = r.result[0]["text"].as_str().unwrap();
        assert!(text.contains("blue folder"), "{text}");
        assert!(text.contains("Source: "), "{text}");

        let r = manager
            .execute_tool(LIST_TOOL, &json!({}), Some(&servers))
            .await;
        assert!(r.result[0]["text"]
            .as_str()
            .unwrap()
            .contains("Notes (id: "));
        let r = manager
            .execute_tool(
                SOURCES_TOOL,
                &json!({ "knowledgeBase": "notes" }),
                Some(&servers),
            )
            .await;
        assert!(
            r.result[0]["text"].as_str().unwrap().contains("n.md"),
            "{r:?}"
        );

        // Tool-level errors reach the model as text (the MCP client keeps only the content).
        let r = manager
            .execute_tool(
                SEARCH_TOOL,
                &json!({ "query": "x", "knowledgeBase": "nope" }),
                Some(&servers),
            )
            .await;
        assert_eq!(r.result[0]["text"], "No knowledge base named \"nope\"");

        // Bad configs are rejected.
        let bad = KnowledgeMcp {
            svc,
            runtime: tokio::runtime::Handle::current(),
        };
        let mut cfg = common::agent::local_knowledge_base_server("../x");
        assert!(bad.connect(&cfg).is_err());
        cfg.agent_id = Some("ok".into());
        cfg.name = "other".into();
        assert!(bad.connect(&cfg).is_err());
    }
}
