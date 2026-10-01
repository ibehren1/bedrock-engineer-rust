//! Agent lookup by id.
//!
//! The TS app has two lookups, and they differ in where shared agents come from:
//!
//! * `BackgroundAgentService.getAllAgents()` (agent runs and delegation): store `customAgents`
//!   followed by the shared agents read from `<projectPath>/.bedrock-engineer/agents` on disk
//!   (`read-shared-agents`) — [`AgentCatalog::all_agents`].
//! * `src/preload/helpers/agent-helpers.ts` `findAgentById()` (tools such as `executeCommand`):
//!   store `customAgents` followed by the store `sharedAgents` cache the renderer writes —
//!   [`AgentCatalog::tool_agents`], and the [`tools::AgentResolver`] impl.
//!
//! There is no separate built-in list: the renderer seeds `DEFAULT_AGENTS` into `customAgents`
//! on startup (skipping the ones the user hid), so the defaults are found through the store just
//! as in the Electron app. Directory (marketplace) agents are not included, as in TS.

use crate::store::{project_path, StoreReader};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tools::{AgentResolver, AgentToolConfig};

fn array_at(store: &Value, key: &str) -> Vec<Value> {
    store
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn has_id(agent: &Value, id: &str) -> bool {
    agent.get("id").and_then(Value::as_str) == Some(id)
}

/// Agent definitions from the store and the project's shared agents directory.
#[derive(Clone)]
pub struct AgentCatalog {
    store: Arc<dyn StoreReader>,
}

impl AgentCatalog {
    pub fn new(store: Arc<dyn StoreReader>) -> Self {
        AgentCatalog { store }
    }

    /// `getAllAgents()` of `BackgroundAgentService`: `[...customAgents, ...sharedAgents]` with
    /// the shared agents freshly loaded from disk.
    pub fn all_agents(&self) -> Vec<Value> {
        let store = self.store.snapshot();
        let mut agents = array_at(&store, "customAgents");
        let shared = common::agent_files::load_shared_agents(project_path(&store).map(Path::new));
        agents.extend(shared.agents);
        agents
    }

    /// `getAgentById(agentId)`: the first agent in [`Self::all_agents`] with that id.
    pub fn find_agent_by_id(&self, agent_id: &str) -> Option<Value> {
        let agents = self.all_agents();
        let found = agents.into_iter().find(|a| has_id(a, agent_id));
        tracing::debug!(agent_id, found = found.is_some(), "Agent search completed");
        found
    }

    /// `getAllAgents()` of the preload helpers: `[...customAgents, ...sharedAgents]` from the
    /// store.
    pub fn tool_agents(&self) -> Vec<Value> {
        let store = self.store.snapshot();
        let mut agents = array_at(&store, "customAgents");
        agents.extend(array_at(&store, "sharedAgents"));
        agents
    }
}

impl AgentResolver for AgentCatalog {
    /// `findAgentById(agentId)` of the preload helpers.
    fn find_agent(&self, agent_id: &str) -> Option<AgentToolConfig> {
        self.tool_agents()
            .iter()
            .find(|a| has_id(a, agent_id))
            .and_then(AgentToolConfig::from_agent_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(store: Value) -> AgentCatalog {
        AgentCatalog::new(Arc::new(move || store.clone()))
    }

    #[test]
    fn run_lookup_reads_custom_then_shared_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let agents_dir = dir.path().join(".bedrock-engineer/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("reviewer.yaml"),
            "id: shared-reviewer-abc\nname: Reviewer\ndescription: Reviews\nsystem: Review code\nscenarios: []\n",
        )
        .unwrap();
        let c = catalog(json!({
            "projectPath": dir.path().to_string_lossy(),
            "customAgents": [{"id": "softwareAgent", "name": "Software Developer"}],
            "sharedAgents": [{"id": "cached-only", "name": "Cached"}]
        }));

        let ids: Vec<_> = c
            .all_agents()
            .iter()
            .map(|a| a["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, vec!["softwareAgent", "shared-reviewer-abc"]);
        let shared = c.find_agent_by_id("shared-reviewer-abc").unwrap();
        assert_eq!(shared["isShared"], true);
        assert_eq!(shared["name"], "Reviewer");
        // The store's sharedAgents cache is not consulted for runs.
        assert!(c.find_agent_by_id("cached-only").is_none());
    }

    #[test]
    fn no_project_means_custom_agents_only() {
        let c = catalog(json!({"customAgents": [{"id": "a"}]}));
        assert_eq!(c.all_agents().len(), 1);
        assert!(c.find_agent_by_id("b").is_none());
    }

    #[test]
    fn tool_resolver_reads_the_store_shared_agents_cache() {
        let c = catalog(json!({
            "customAgents": [{"id": "a", "allowedCommands": [{"pattern": "ls *", "description": "list"}]}],
            "sharedAgents": [{"id": "shared-b", "tools": ["dockerSandbox"]}]
        }));
        assert_eq!(
            c.find_agent("a").unwrap().allowed_commands[0].pattern,
            "ls *"
        );
        assert_eq!(
            c.find_agent("shared-b").unwrap().tools,
            vec!["dockerSandbox"]
        );
        assert!(c.find_agent("missing").is_none());
    }
}
