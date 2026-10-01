//! The official MCP Registry (<https://registry.modelcontextprotocol.io>) — port of
//! `src/common/mcp/registry.ts` and `src/main/mcp/registry-client.ts`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::time::Duration;

pub const MCP_REGISTRY_BASE_URL: &str = "https://registry.modelcontextprotocol.io";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryPackage {
    /// npm, pypi, oci, nuget, …
    pub registry_type: String,
    pub identifier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<String>,
    /// Names only; the registry never carries values.
    pub env_vars: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryRemote {
    /// streamable-http or sse
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryServer {
    /// Fully qualified registry name, e.g. `io.github.containers/kubernetes-mcp-server`.
    pub name: String,
    /// Last path segment, used as the config key.
    pub short_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
    pub packages: Vec<McpRegistryPackage>,
    pub remotes: Vec<McpRegistryRemote>,
}

fn as_array(value: Option<&Value>) -> &[Value] {
    match value {
        Some(Value::Array(a)) => a,
        _ => &[],
    }
}

/// JS truthiness for the string fields (`x || undefined`).
fn truthy_str(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) if n.as_f64() != Some(0.0) => Some(n.to_string()),
        _ => None,
    }
}

fn first_truthy(obj: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| truthy_str(obj.get(*k)))
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(_) => true,
    }
}

/// `registryShortName`: last path segment of a registry name, sanitised for use as a config key.
pub fn registry_short_name(name: &str) -> String {
    let tail = name
        .rsplit('/')
        .next()
        .filter(|t| !t.is_empty())
        .unwrap_or(name);
    tail.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// `normalizeRegistryServers`: map a `/v0/servers` response into the fields the UI needs.
pub fn normalize_registry_servers(payload: &Value) -> Vec<McpRegistryServer> {
    as_array(payload.get("servers"))
        .iter()
        .filter_map(|entry| entry.get("server"))
        .filter_map(|server| {
            let name = server.get("name")?.as_str().filter(|n| !n.is_empty())?;
            Some(McpRegistryServer {
                name: name.to_string(),
                short_name: registry_short_name(name),
                title: truthy_str(server.get("title")),
                description: truthy_str(server.get("description")).unwrap_or_default(),
                version: truthy_str(server.get("version")),
                repository_url: server
                    .get("repository")
                    .and_then(|r| truthy_str(r.get("url"))),
                packages: as_array(server.get("packages"))
                    .iter()
                    .map(|pkg| McpRegistryPackage {
                        registry_type: first_truthy(pkg, &["registryType", "registry_type"])
                            .unwrap_or_else(|| "unknown".into()),
                        identifier: truthy_str(pkg.get("identifier")).unwrap_or_default(),
                        version: truthy_str(pkg.get("version")),
                        runtime_hint: first_truthy(pkg, &["runtimeHint", "runtime_hint"]),
                        env_vars: {
                            let vars = if truthy(pkg.get("environmentVariables")) {
                                pkg.get("environmentVariables")
                            } else {
                                pkg.get("environment_variables")
                            };
                            as_array(vars)
                                .iter()
                                .filter_map(|v| truthy_str(v.get("name")))
                                .collect()
                        },
                    })
                    .collect(),
                remotes: as_array(server.get("remotes"))
                    .iter()
                    .filter(|r| truthy(r.get("url")))
                    .filter_map(|r| {
                        Some(McpRegistryRemote {
                            kind: truthy_str(r.get("type"))
                                .unwrap_or_else(|| "streamable-http".into()),
                            url: truthy_str(r.get("url"))?,
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

/// How a registry entry would be launched (`McpRegistryLaunch`; `None` = no way to run).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum McpRegistryLaunch {
    #[serde(rename_all = "camelCase")]
    Command {
        command: String,
        args: Vec<String>,
        env_vars: Vec<String>,
    },
    Url {
        url: String,
    },
}

/// `registryServerLaunch`: npm (pinned) first, then pypi via `uvx`, then the first remote.
pub fn registry_server_launch(server: &McpRegistryServer) -> Option<McpRegistryLaunch> {
    if let Some(npm) = server
        .packages
        .iter()
        .find(|p| p.registry_type == "npm" && !p.identifier.is_empty())
    {
        let spec = match &npm.version {
            Some(v) => format!("{}@{v}", npm.identifier),
            None => npm.identifier.clone(),
        };
        return Some(McpRegistryLaunch::Command {
            command: "npx".into(),
            args: vec!["-y".into(), spec],
            env_vars: npm.env_vars.clone(),
        });
    }
    if let Some(pypi) = server
        .packages
        .iter()
        .find(|p| p.registry_type == "pypi" && !p.identifier.is_empty())
    {
        return Some(McpRegistryLaunch::Command {
            command: "uvx".into(),
            args: vec![pypi.identifier.clone()],
            env_vars: pypi.env_vars.clone(),
        });
    }
    server
        .remotes
        .first()
        .map(|r| McpRegistryLaunch::Url { url: r.url.clone() })
}

/// `registryLaunchLabel`: one-line summary of how the server runs.
pub fn registry_launch_label(server: &McpRegistryServer) -> Option<String> {
    Some(match registry_server_launch(server)? {
        McpRegistryLaunch::Url { url } => url,
        McpRegistryLaunch::Command { command, args, .. } => std::iter::once(command)
            .chain(args)
            .collect::<Vec<_>>()
            .join(" "),
    })
}

/// `registryServerToConfigJson`: the `mcpServers` JSON for an entry, pretty-printed.
pub fn registry_server_to_config_json(server: &McpRegistryServer) -> Option<String> {
    let entry = match registry_server_launch(server)? {
        McpRegistryLaunch::Url { url } => json!({ "url": url }),
        McpRegistryLaunch::Command {
            command,
            args,
            env_vars,
        } => {
            let mut entry = json!({ "command": command, "args": args });
            if !env_vars.is_empty() {
                let env: Map<String, Value> = env_vars
                    .into_iter()
                    .map(|k| (k, Value::String(String::new())))
                    .collect();
                entry["env"] = Value::Object(env);
            }
            entry
        }
    };
    let mut servers = Map::new();
    servers.insert(server.short_name.clone(), entry);
    serde_json::to_string_pretty(&json!({ "mcpServers": servers })).ok()
}

/// JS `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The search URL (latest versions only; `limit` clamped to 1..=30).
pub fn registry_search_url(query: &str, limit: u32) -> String {
    format!(
        "{MCP_REGISTRY_BASE_URL}/v0/servers?search={}&version=latest&limit={}",
        encode_uri_component(query.trim()),
        limit.clamp(1, 30)
    )
}

/// `searchMcpRegistry(query, limit = 10)`. Pass a client configured with the app's proxy.
pub async fn search_mcp_registry(
    client: &reqwest::Client,
    query: &str,
    limit: u32,
) -> crate::Result<Vec<McpRegistryServer>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let payload: Value = client
        .get(registry_search_url(trimmed, limit))
        .timeout(Duration::from_secs(15))
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let servers = normalize_registry_servers(&payload);
    tracing::debug!(
        category = "mcp:registry",
        query = trimmed,
        result_count = servers.len(),
        "MCP registry search"
    );
    Ok(servers)
}

#[cfg(test)]
mod tests {
    //! Port of `src/common/mcp/registry.test.ts`.
    use super::*;

    // Trimmed shape of a real /v0/servers?search=… response.
    fn payload() -> Value {
        json!({
            "servers": [
                {
                    "server": {
                        "name": "io.github.containers/kubernetes-mcp-server",
                        "description": "A Model Context Protocol (MCP) server for Kubernetes and OpenShift",
                        "version": "0.0.66",
                        "repository": {
                            "url": "https://github.com/containers/kubernetes-mcp-server",
                            "source": "github"
                        },
                        "packages": [
                            { "registryType": "npm", "identifier": "kubernetes-mcp-server", "version": "0.0.66" },
                            { "registryType": "pypi", "identifier": "kubernetes-mcp-server", "version": "0.0.66" },
                            { "registryType": "oci", "identifier": "ghcr.io/containers/kubernetes-mcp-server:v0.0.66" }
                        ]
                    }
                },
                {
                    "server": {
                        "name": "ai.waystation/postgres",
                        "title": "Postgres",
                        "description": "Connect to your PostgreSQL database to query data and schemas.",
                        "remotes": [
                            { "type": "streamable-http", "url": "https://waystation.ai/postgres/mcp" },
                            { "type": "sse", "url": "https://waystation.ai/postgres/mcp/sse" }
                        ]
                    }
                },
                {
                    "server": {
                        "name": "io.github.someone/pypi-only",
                        "description": "Python only server",
                        "packages": [
                            {
                                "registryType": "pypi",
                                "identifier": "some-mcp",
                                "environmentVariables": [{ "name": "API_KEY" }, { "name": "REGION" }]
                            }
                        ]
                    }
                },
                {
                    "server": {
                        "name": "io.github.someone/repo-only",
                        "description": "Published without package metadata"
                    }
                },
                { "server": { "description": "nameless entries are dropped" } }
            ]
        })
    }

    fn servers() -> Vec<McpRegistryServer> {
        normalize_registry_servers(&payload())
    }

    // describe('normalizeRegistryServers')
    #[test]
    fn maps_names_descriptions_packages_and_remotes_dropping_nameless_entries() {
        let servers = servers();
        assert_eq!(servers.len(), 4);
        assert_eq!(
            servers[0].name,
            "io.github.containers/kubernetes-mcp-server"
        );
        assert_eq!(servers[0].short_name, "kubernetes-mcp-server");
        assert_eq!(servers[0].version.as_deref(), Some("0.0.66"));
        assert_eq!(
            servers[0].repository_url.as_deref(),
            Some("https://github.com/containers/kubernetes-mcp-server")
        );
        assert_eq!(
            servers[0]
                .packages
                .iter()
                .map(|p| p.registry_type.as_str())
                .collect::<Vec<_>>(),
            vec!["npm", "pypi", "oci"]
        );
        assert_eq!(
            serde_json::to_value(&servers[1].remotes[0]).unwrap(),
            json!({ "type": "streamable-http", "url": "https://waystation.ai/postgres/mcp" })
        );
    }

    #[test]
    fn collects_environment_variable_names_only() {
        assert_eq!(servers()[2].packages[0].env_vars, vec!["API_KEY", "REGION"]);
    }

    #[test]
    fn tolerates_junk_input() {
        assert!(normalize_registry_servers(&Value::Null).is_empty());
        assert!(normalize_registry_servers(&json!({ "servers": "nope" })).is_empty());
    }

    // describe('registryServerLaunch')
    #[test]
    fn prefers_the_npm_package_and_pins_the_version() {
        assert_eq!(
            serde_json::to_value(registry_server_launch(&servers()[0])).unwrap(),
            json!({
                "kind": "command",
                "command": "npx",
                "args": ["-y", "kubernetes-mcp-server@0.0.66"],
                "envVars": []
            })
        );
    }

    #[test]
    fn falls_back_to_a_remote_endpoint_when_there_is_no_package() {
        assert_eq!(
            serde_json::to_value(registry_server_launch(&servers()[1])).unwrap(),
            json!({ "kind": "url", "url": "https://waystation.ai/postgres/mcp" })
        );
    }

    #[test]
    fn uses_uvx_for_pypi_only_servers() {
        match registry_server_launch(&servers()[2]).unwrap() {
            McpRegistryLaunch::Command { command, args, .. } => {
                assert_eq!(command, "uvx");
                assert_eq!(args, vec!["some-mcp"]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn returns_null_when_the_entry_has_no_way_to_run() {
        assert!(registry_server_launch(&servers()[3]).is_none());
        assert!(registry_launch_label(&servers()[3]).is_none());
    }

    // describe('registryServerToConfigJson')
    #[test]
    fn builds_a_command_config_keyed_by_the_short_name() {
        let v: Value =
            serde_json::from_str(&registry_server_to_config_json(&servers()[0]).unwrap()).unwrap();
        assert_eq!(
            v,
            json!({
                "mcpServers": {
                    "kubernetes-mcp-server": {
                        "command": "npx",
                        "args": ["-y", "kubernetes-mcp-server@0.0.66"]
                    }
                }
            })
        );
    }

    #[test]
    fn includes_empty_placeholders_for_required_environment_variables() {
        let v: Value =
            serde_json::from_str(&registry_server_to_config_json(&servers()[2]).unwrap()).unwrap();
        assert_eq!(
            v,
            json!({
                "mcpServers": {
                    "pypi-only": {
                        "command": "uvx",
                        "args": ["some-mcp"],
                        "env": { "API_KEY": "", "REGION": "" }
                    }
                }
            })
        );
    }

    #[test]
    fn builds_a_url_config_for_remote_servers() {
        let v: Value =
            serde_json::from_str(&registry_server_to_config_json(&servers()[1]).unwrap()).unwrap();
        assert_eq!(
            v,
            json!({ "mcpServers": { "postgres": { "url": "https://waystation.ai/postgres/mcp" } } })
        );
    }

    #[test]
    fn returns_null_when_there_is_nothing_to_launch() {
        assert!(registry_server_to_config_json(&servers()[3]).is_none());
    }

    // describe('registryShortName')
    #[test]
    fn takes_the_last_segment_and_sanitises_it() {
        assert_eq!(registry_short_name("io.github.foo/bar-baz"), "bar-baz");
        assert_eq!(registry_short_name("weird name/with spaces"), "with-spaces");
    }

    // registry-client.ts
    #[test]
    fn search_url_encodes_and_clamps() {
        assert_eq!(
            registry_search_url(" kube rnetes ", 100),
            "https://registry.modelcontextprotocol.io/v0/servers?search=kube%20rnetes&version=latest&limit=30"
        );
        assert!(registry_search_url("x", 0).ends_with("limit=1"));
    }
}
