//! Port of `src/common/agents/delegation.ts` — shared policy for agent-to-agent delegation
//! (the `invokeAgent` tool).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// Maximum delegation depth. The top-level chat runs at depth 0, so 2 permits
/// chat -> agent A -> agent B and strips `invokeAgent` from B's tools.
pub const MAX_DELEGATION_DEPTH: u32 = 2;

/// Cap on the sub-agent text handed back to the caller, in characters.
pub const MAX_DELEGATION_RESULT_CHARS: usize = 60_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationTarget {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationContext {
    /// Depth the *calling* agent runs at. 0 = top-level chat.
    pub depth: u32,
    /// Agent ids from the root down to and including the calling agent.
    pub lineage: Vec<String>,
    /// Ids the user permitted for this turn via @mention.
    pub allowed_agent_ids: Vec<String>,
}

/// Narrow the turn allowlist to the agents a caller may delegate to: nothing already in its own
/// ancestry (which would loop), not itself, no duplicates or empty ids.
pub fn filter_delegation_targets(
    allowed_agent_ids: &[String],
    lineage: &[String],
    self_agent_id: Option<&str>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    allowed_agent_ids
        .iter()
        .filter(|id| {
            if id.is_empty() || seen.contains(id.as_str()) {
                return false;
            }
            if lineage.iter().any(|l| l == *id) {
                return false;
            }
            if let Some(me) = self_agent_id.filter(|s| !s.is_empty()) {
                if id.as_str() == me {
                    return false;
                }
            }
            seen.insert(id.as_str());
            true
        })
        .cloned()
        .collect()
}

/// Whether an agent at `depth` is allowed to delegate at all.
pub fn can_delegate(depth: u32, remaining_target_count: usize) -> bool {
    depth < MAX_DELEGATION_DEPTH && remaining_target_count > 0
}

/// JS `text.length <= max ? text : text.slice(0, max - 1).trimEnd() + '…'` (UTF-16 lengths).
fn truncate(text: &str, max: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= max {
        return text.to_string();
    }
    let head = String::from_utf16_lossy(&units[..max.saturating_sub(1)]);
    format!("{}…", head.trim_end())
}

/// Clone the static `invokeAgent` tool spec (Bedrock `toolSpec` JSON), constraining `agentId` to
/// the permitted ids and listing their names in the description.
///
/// Returns `None` when nothing may be delegated to — callers must then drop the tool entirely.
pub fn build_invoke_agent_tool_spec(
    base_spec: Option<&Value>,
    allowed: &[DelegationTarget],
) -> Option<Value> {
    let base_spec = base_spec.filter(|v| !v.is_null())?;
    if allowed.is_empty() {
        return None;
    }

    let mut spec = base_spec.clone();

    if let Some(agent_id) = spec
        .pointer_mut("/inputSchema/json/properties/agentId")
        .filter(|v| v.is_object())
    {
        agent_id["enum"] = Value::Array(
            allowed
                .iter()
                .map(|a| Value::String(a.id.clone()))
                .collect(),
        );
    }

    let table = allowed
        .iter()
        .map(|a| {
            let desc = match a.description.as_deref() {
                Some(d) if !d.is_empty() => format!(" — {}", truncate(d, 300)),
                _ => String::new(),
            };
            format!("- `{}` — **{}**{}", a.id, a.name, desc)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let original = spec
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let description = format!(
        "{original}\n\n## Agents you may delegate to in this request\n\n{table}\n\nThe user permitted these agents by @mentioning them. You are not required to delegate — do the work yourself when that is simpler."
    );

    if let Some(obj) = spec.as_object_mut() {
        obj.insert("description".into(), Value::String(description));
    }
    Some(spec)
}

#[cfg(test)]
mod tests {
    //! Port of `src/common/agents/delegation.test.ts`.
    use super::*;
    use serde_json::json;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn base_spec() -> Value {
        json!({
            "name": "invokeAgent",
            "description": "Delegate a task.",
            "inputSchema": {
                "json": {
                    "type": "object",
                    "properties": {
                        "agentId": { "type": "string", "description": "target" },
                        "task": { "type": "string" }
                    },
                    "required": ["agentId", "task"]
                }
            }
        })
    }

    fn target(id: &str, name: &str, description: Option<&str>) -> DelegationTarget {
        DelegationTarget {
            id: id.into(),
            name: name.into(),
            description: description.map(Into::into),
        }
    }

    // describe('filterDelegationTargets')
    #[test]
    fn removes_agents_already_in_the_caller_chain() {
        assert_eq!(
            filter_delegation_targets(&ids(&["a", "b", "c"]), &ids(&["a"]), Some("b")),
            ids(&["c"])
        );
    }

    #[test]
    fn removes_the_caller_itself() {
        assert_eq!(
            filter_delegation_targets(&ids(&["a", "b"]), &[], Some("a")),
            ids(&["b"])
        );
    }

    #[test]
    fn dedupes_and_drops_empty_ids() {
        assert_eq!(
            filter_delegation_targets(&ids(&["a", "a", "", "b"]), &[], None),
            ids(&["a", "b"])
        );
    }

    #[test]
    fn returns_an_empty_list_when_everything_is_excluded() {
        assert!(filter_delegation_targets(&ids(&["a"]), &ids(&["a"]), Some("a")).is_empty());
    }

    // describe('canDelegate')
    #[test]
    fn permits_delegation_below_the_depth_limit_with_targets_available() {
        assert!(can_delegate(0, 1));
        assert!(can_delegate(MAX_DELEGATION_DEPTH - 1, 1));
    }

    #[test]
    fn refuses_at_or_beyond_the_depth_limit() {
        assert!(!can_delegate(MAX_DELEGATION_DEPTH, 5));
        assert!(!can_delegate(MAX_DELEGATION_DEPTH + 1, 5));
    }

    #[test]
    fn refuses_when_no_targets_remain() {
        assert!(!can_delegate(0, 0));
    }

    // describe('buildInvokeAgentToolSpec')
    #[test]
    fn returns_null_when_nothing_may_be_delegated_to() {
        assert!(build_invoke_agent_tool_spec(Some(&base_spec()), &[]).is_none());
    }

    #[test]
    fn returns_null_for_a_missing_base_spec() {
        assert!(build_invoke_agent_tool_spec(None, &[target("a", "A", None)]).is_none());
    }

    #[test]
    fn constrains_agent_id_to_the_permitted_ids() {
        let spec = build_invoke_agent_tool_spec(
            Some(&base_spec()),
            &[
                target("reviewer", "Reviewer", None),
                target("writer", "Writer", None),
            ],
        )
        .unwrap();
        assert_eq!(
            spec["inputSchema"]["json"]["properties"]["agentId"]["enum"],
            json!(["reviewer", "writer"])
        );
    }

    #[test]
    fn lists_every_id_and_name_in_the_description() {
        let spec = build_invoke_agent_tool_spec(
            Some(&base_spec()),
            &[target("reviewer", "Reviewer", Some("Reviews code"))],
        )
        .unwrap();
        let d = spec["description"].as_str().unwrap();
        assert!(d.contains("reviewer"));
        assert!(d.contains("Reviewer"));
        assert!(d.contains("Reviews code"));
        // The original description must survive
        assert!(d.contains("Delegate a task."));
    }

    #[test]
    fn truncates_a_very_long_target_description() {
        let long = "x".repeat(500);
        let spec =
            build_invoke_agent_tool_spec(Some(&base_spec()), &[target("a", "A", Some(&long))])
                .unwrap();
        let d = spec["description"].as_str().unwrap();
        assert!(d.contains('…'));
        assert!(!d.contains(&"x".repeat(400)));
    }

    #[test]
    fn does_not_mutate_the_base_spec() {
        let base = base_spec();
        let before = serde_json::to_string(&base).unwrap();
        build_invoke_agent_tool_spec(Some(&base), &[target("a", "A", None)]);
        assert_eq!(serde_json::to_string(&base).unwrap(), before);
    }
}
