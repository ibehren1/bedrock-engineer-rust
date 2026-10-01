//! Build and write a sandbox's compose file. Port of `src/main/api/docker/composeWriter.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map as JsonMap, Value as JsonValue};
use yaml_serde::{Mapping, Value};

use crate::types::{
    CreateSandboxOptions, DockerSandboxConfig, SandboxPortMapping, SandboxServiceSpec,
    SandboxServiceSummary, DEFAULT_SANDBOX_IMAGE, DEFAULT_SERVICE_NAME, WORKSPACE_MOUNT,
};
use crate::util::{is_inside, relative, resolve, resolve_one};
use crate::{Error, Result};

/// Top-level keys agent-authored compose YAML may use. `volumes` declarations are converted
/// to mapped folders and `x-*` extension fields (anchor holders) are dropped; anything else
/// (`networks`, `secrets`, `configs`, `include`, `name`, ...) could reach outside the sandbox
/// or rename the project, so it is rejected.
const ALLOWED_TOP_LEVEL_KEYS: [&str; 3] = ["services", "volumes", "version"];

/// Per-service keys agent-authored compose YAML may use. This is an allowlist: keys such as
/// `privileged`, `cap_add`, `devices`, `pid`, `ipc`, `security_opt`, `env_file`,
/// `volumes_from`, `secrets`, `configs`, `extends`, `networks`, `deploy`, `sysctls` and
/// `ulimits` weaken isolation, read host files, or override the resource limits.
/// `volumes` are rewritten, `mem_limit`/`cpus` overwritten with the configured limits,
/// `network_mode` must be `bridge`, and `build` is rejected with its own message.
const ALLOWED_SERVICE_KEYS: [&str; 25] = [
    "image",
    "build",
    "command",
    "entrypoint",
    "environment",
    "ports",
    "expose",
    "working_dir",
    "depends_on",
    "healthcheck",
    "restart",
    "user",
    "tty",
    "stdin_open",
    "init",
    "hostname",
    "stop_signal",
    "stop_grace_period",
    "platform",
    "pull_policy",
    "read_only",
    "volumes",
    "network_mode",
    "mem_limit",
    "cpus",
];

/// Named volumes (and `dataVolumes` names) become `data/<name>` on the host.
fn is_valid_volume_name(name: &str) -> bool {
    static NAME: OnceLock<Regex> = OnceLock::new();
    NAME.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_.-]*$").unwrap())
        .is_match(name)
        && !name.contains("..")
}

/// Environment variable names accepted from the agent's `env`.
fn is_valid_env_name(name: &str) -> bool {
    static NAME: OnceLock<Regex> = OnceLock::new();
    NAME.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").unwrap())
        .is_match(name)
}

/// Compose interpolates `$VAR` / `${VAR}` in every value from the host environment and the
/// `.env` file, which would let agent-authored text read host secrets or rewrite the
/// writer's `${PROJECT_PATH}`. `$$` is Compose's literal `$`.
fn escape_interpolation(text: &str) -> String {
    text.replace('$', "$$")
}

/// [`escape_interpolation`] over every string scalar in a value (mapping keys untouched).
fn escape_value(value: &mut Value) {
    match value {
        Value::String(text) => *text = escape_interpolation(text),
        Value::Sequence(items) => items.iter_mut().for_each(escape_value),
        Value::Mapping(map) => map.values_mut().for_each(escape_value),
        Value::Tagged(tagged) => escape_value(&mut tagged.value),
        _ => {}
    }
}

/// Reject `environment` entries without a value: Compose copies those from the host's
/// environment (`FOO:` / `- FOO`), which would hand host secrets to the container.
fn check_environment(service_name: &str, environment: &Value) -> Result<()> {
    let passthrough = |name: &str| {
        Error::msg(format!(
            "Service \"{service_name}\" sets environment variable \"{name}\" without a value, which would copy it from the host. Give it a value."
        ))
    };
    match environment {
        Value::Null => Ok(()),
        Value::Mapping(map) => match map.iter().find(|(_, v)| v.is_null()) {
            Some((name, _)) => Err(passthrough(&key_name(name))),
            None => Ok(()),
        },
        Value::Sequence(items) => {
            for item in items {
                let text = js_display(item);
                if !text.contains('=') {
                    return Err(passthrough(&text));
                }
            }
            Ok(())
        }
        _ => Err(Error::msg(format!(
            "Service \"{service_name}\" has an unsupported environment entry. Use a mapping or a list of \"NAME=value\"."
        ))),
    }
}

/// `path` with symlinks resolved as far as it exists; the missing tail is appended as is.
/// Lexical checks alone are not enough: the container can write symlinks into the project
/// mount (`ln -s / /workspace/escape`), which Docker would follow on the host.
fn canonicalize_lenient(path: &Path) -> PathBuf {
    let absolute = resolve_one(path);
    let mut existing = absolute.as_path();
    let mut tail = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            let mut out = canonical;
            out.extend(tail.iter().rev());
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

/// `target` stays inside one of `roots` both lexically and after resolving symlinks.
fn is_contained(target: &Path, roots: &[&Path]) -> bool {
    let lexical = resolve_one(target);
    let canonical = canonicalize_lenient(target);
    roots
        .iter()
        .any(|root| is_inside(&lexical, &resolve_one(root)))
        && roots
            .iter()
            .any(|root| is_inside(&canonical, &canonicalize_lenient(root)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltCompose {
    /// The compose document, ready to serialize.
    pub document: Value,
    /// Normalized service summary for sandbox.json and the UI.
    pub services: Vec<SandboxServiceSummary>,
    /// Warnings worth surfacing to the agent (e.g. a rewritten volume).
    pub warnings: Vec<String>,
}

impl BuiltCompose {
    /// `document.services` as a mapping.
    pub fn services_mapping(&self) -> Option<&Mapping> {
        self.document.get("services").and_then(Value::as_mapping)
    }
}

/// JS `Number(x)` over a YAML scalar.
fn js_number(value: &Value) -> f64 {
    match value {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => js_string_to_number(s),
        _ => f64::NAN,
    }
}

fn js_string_to_number(text: &str) -> f64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("inf") || lower.contains("nan") {
        return f64::NAN;
    }
    trimmed.parse().unwrap_or(f64::NAN)
}

/// A finite JS number that is a valid port.
fn to_port(n: f64) -> Option<u16> {
    (n.is_finite() && n.fract() == 0.0 && (0.0..=65535.0).contains(&n)).then_some(n as u16)
}

fn parse_port_mapping(raw: &Value) -> Option<SandboxPortMapping> {
    match raw {
        Value::Number(_) => {
            let port = to_port(js_number(raw))?;
            Some(SandboxPortMapping {
                host: port,
                container: port,
            })
        }
        Value::String(text) => {
            // Accept "3000", "3000:3000", "127.0.0.1:3000:3000". Reject ranges.
            let parts: Vec<&str> = text.split(':').collect();
            if parts.len() == 1 {
                let port = to_port(js_string_to_number(parts[0]))?;
                return Some(SandboxPortMapping {
                    host: port,
                    container: port,
                });
            }
            let tail = &parts[parts.len() - 2..];
            static PROTOCOL: OnceLock<Regex> = OnceLock::new();
            let container_text = PROTOCOL
                .get_or_init(|| Regex::new(r"/(tcp|udp)$").unwrap())
                .replace(tail[1], "");
            let host = to_port(js_string_to_number(tail[0]))?;
            let container = to_port(js_string_to_number(&container_text))?;
            Some(SandboxPortMapping { host, container })
        }
        Value::Mapping(map) => {
            let host = map.get("published").map(js_number).unwrap_or(f64::NAN);
            let container = map.get("target").map(js_number).unwrap_or(f64::NAN);
            Some(SandboxPortMapping {
                host: to_port(host)?,
                container: to_port(container)?,
            })
        }
        _ => None,
    }
}

/// Check whether a TCP port can be bound on the host, so creation fails with a clear
/// message instead of an opaque `compose up` bind error.
pub fn is_host_port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn assert_ports_available(services: &[SandboxServiceSummary]) -> Result<()> {
    let mut seen: HashMap<u16, &str> = HashMap::new();
    for service in services {
        for mapping in &service.ports {
            if let Some(previous) = seen.get(&mapping.host) {
                return Err(Error::msg(format!(
                    "Host port {} is requested by both \"{previous}\" and \"{}\". Give each service a distinct host port.",
                    mapping.host, service.name
                )));
            }
            seen.insert(mapping.host, &service.name);

            if !is_host_port_free(mapping.host) {
                return Err(Error::msg(format!(
                    "Host port {} (requested by service \"{}\") is already in use. Pick a different host port, or stop whatever is listening on {}.",
                    mapping.host, service.name, mapping.host
                )));
            }
        }
    }
    Ok(())
}

fn serialize_volume(source: &str, container_path: &str) -> String {
    format!("{source}:{container_path}")
}

/// Compose volume sources are forward-slash separated, even on Windows.
fn to_compose_path(path: &Path) -> String {
    path.to_string_lossy()
        .split(std::path::MAIN_SEPARATOR)
        .collect::<Vec<_>>()
        .join("/")
}

fn key(name: &str) -> Value {
    Value::String(name.to_string())
}

fn str_value(text: impl Into<String>) -> Value {
    Value::String(text.into())
}

fn string_seq(items: impl IntoIterator<Item = String>) -> Value {
    Value::Sequence(items.into_iter().map(Value::String).collect())
}

fn json_to_yaml(value: &JsonValue) -> Value {
    yaml_serde::to_value(value).unwrap_or(Value::Null)
}

/// JS `String(x)` / template interpolation of a YAML value.
fn js_display(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Sequence(items) => items.iter().map(js_display).collect::<Vec<_>>().join(","),
        Value::Mapping(_) => "[object Object]".to_string(),
        Value::Tagged(tagged) => js_display(&tagged.value),
    }
}

/// JS truthiness of a YAML value.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn key_name(value: &Value) -> String {
    js_display(value)
}

/// Build the compose document for a default, generated sandbox.
fn build_generated_compose(
    services: &[SandboxServiceSpec],
    config: &DockerSandboxConfig,
) -> Result<BuiltCompose> {
    let mut service_map = Mapping::new();
    let mut summary = Vec::new();

    for spec in services {
        let image = spec
            .image
            .clone()
            .unwrap_or_else(|| DEFAULT_SANDBOX_IMAGE.to_string());
        let ports = spec.ports.clone().unwrap_or_default();

        let mut volumes = vec![
            // projectPath is shared read-write so files move in and out of the sandbox.
            serialize_volume("${PROJECT_PATH}", WORKSPACE_MOUNT),
            // Mapped folder instead of a named volume, so data is inspectable on the host.
            serialize_volume(&format!("./data/{}", spec.name), "/data"),
        ];
        for extra in spec.data_volumes.iter().flatten() {
            volumes.push(serialize_volume(
                &format!("./data/{}", extra.name),
                &escape_interpolation(&extra.container_path),
            ));
        }

        let mut service = Mapping::new();
        service.insert(key("image"), str_value(escape_interpolation(&image)));
        // Keeps a bare ubuntu container alive so we can exec into it repeatedly.
        service.insert(
            key("command"),
            str_value(escape_interpolation(
                spec.command.as_deref().unwrap_or("sleep infinity"),
            )),
        );
        service.insert(key("working_dir"), str_value(WORKSPACE_MOUNT));
        service.insert(key("volumes"), string_seq(volumes));
        if !ports.is_empty() {
            service.insert(
                key("ports"),
                string_seq(ports.iter().map(|p| format!("{}:{}", p.host, p.container))),
            );
        }
        if let Some(environment) = spec.environment.as_ref().filter(|env| !env.is_empty()) {
            let mut environment = json_to_yaml(&JsonValue::Object(environment.clone()));
            check_environment(&spec.name, &environment)?;
            escape_value(&mut environment);
            service.insert(key("environment"), environment);
        }
        service.insert(key("mem_limit"), str_value(&config.memory_limit));
        service.insert(key("cpus"), Value::from(config.cpu_limit));

        service_map.insert(key(&spec.name), Value::Mapping(service));
        summary.push(SandboxServiceSummary {
            name: spec.name.clone(),
            image,
            ports,
        });
    }

    let mut document = Mapping::new();
    document.insert(key("services"), Value::Mapping(service_map));
    Ok(BuiltCompose {
        document: Value::Mapping(document),
        services: summary,
        warnings: Vec::new(),
    })
}

/// Validate and normalize agent-authored compose YAML: named volumes become mapped folders
/// under `data/`, bind mounts are confined to projectPath and the sandbox folder, and keys
/// that would break out of the container are rejected.
fn build_from_agent_yaml(
    compose_yaml: &str,
    sandbox_dir: &Path,
    project_path: &Path,
    config: &DockerSandboxConfig,
) -> Result<BuiltCompose> {
    let parsed: Value = yaml_serde::from_str(compose_yaml)
        .map_err(|error| Error::msg(format!("composeYaml is not valid YAML: {error}")))?;

    let mut parsed = parsed;
    // Resolve `<<: *anchor` merges first, so merged-in keys go through the allowlist too.
    parsed
        .apply_merge()
        .map_err(|error| Error::msg(format!("composeYaml is not valid YAML: {error}")))?;

    let mut document = match parsed {
        Value::Mapping(map) => map,
        // JS treats arrays as objects, which then have no `services` key.
        Value::Sequence(_) => Mapping::new(),
        _ => {
            return Err(Error::msg(
                "composeYaml must be a YAML mapping with a top-level \"services\" key.",
            ))
        }
    };

    let has_services = document
        .get("services")
        .and_then(Value::as_mapping)
        .map(|services| !services.is_empty())
        .unwrap_or(false);
    if !has_services {
        return Err(Error::msg(
            "composeYaml must define at least one service under \"services\".",
        ));
    }

    let mut warnings = Vec::new();
    let mut summary = Vec::new();

    let top_level_keys: Vec<String> = document.keys().map(key_name).collect();
    for top in top_level_keys {
        if top.starts_with("x-") {
            // Extension fields only hold anchors, already expanded by the parser.
            document.remove(top.as_str());
        } else if !ALLOWED_TOP_LEVEL_KEYS.contains(&top.as_str()) {
            return Err(Error::msg(format!(
                "composeYaml sets top-level \"{top}\", which is not permitted in a sandbox. Only \"services\" (and \"volumes\", which become mapped folders) are allowed."
            )));
        }
    }

    // Named volumes become mapped folders, so drop the top-level declarations.
    let declared_named_volumes: Vec<String> = document
        .get("volumes")
        .and_then(Value::as_mapping)
        .map(|volumes| volumes.keys().map(key_name).collect())
        .unwrap_or_default();
    document.remove("volumes");
    if !declared_named_volumes.is_empty() {
        warnings.push(format!(
            "Named volumes ({}) were converted to mapped folders under data/.",
            declared_named_volumes.join(", ")
        ));
    }

    let resolved_project = resolve_one(project_path);
    let resolved_sandbox = resolve_one(sandbox_dir);
    static EXPLICIT_PATH: OnceLock<Regex> = OnceLock::new();
    let explicit_path = EXPLICIT_PATH.get_or_init(|| Regex::new(r"^[./~]|^[A-Za-z]:").unwrap());

    let services = document
        .get_mut("services")
        .and_then(Value::as_mapping_mut)
        .expect("checked above");

    for (service_key, raw_service) in services.iter_mut() {
        let service_name = key_name(service_key);
        let Value::Mapping(service) = raw_service else {
            return Err(Error::msg(format!(
                "Service \"{service_name}\" must be a mapping."
            )));
        };

        // The name becomes the default `data/<name>` folder, so it must be a plain name.
        if !is_valid_volume_name(&service_name) {
            return Err(Error::msg(format!(
                "Invalid service name \"{service_name}\". Use letters, digits, dots, dashes, and underscores."
            )));
        }

        for service_key in service.keys().map(key_name) {
            if !ALLOWED_SERVICE_KEYS.contains(&service_key.as_str()) {
                return Err(Error::msg(format!(
                    "Service \"{service_name}\" sets \"{service_key}\", which is not permitted in a sandbox because it can weaken container isolation or reach the host. Remove it. Allowed keys: {}.",
                    ALLOWED_SERVICE_KEYS.join(", ")
                )));
            }
        }

        match service.get("network_mode") {
            None | Some(Value::Null) => {}
            Some(Value::String(mode)) if mode == "bridge" => {}
            Some(mode) => {
                return Err(Error::msg(format!(
                    "Service \"{service_name}\" sets network_mode \"{}\". Only the default bridge network is permitted in a sandbox.",
                    js_display(mode)
                )));
            }
        }

        if !truthy(service.get("image")) && !truthy(service.get("build")) {
            service.insert(key("image"), str_value(DEFAULT_SANDBOX_IMAGE));
        }
        if truthy(service.get("build")) {
            return Err(Error::msg(format!(
                "Service \"{service_name}\" uses \"build\". Sandboxes run prebuilt images only — use \"image\" and install packages with a command instead."
            )));
        }

        // Every service keeps the workspace mount and a data folder; rewrite the rest.
        let mut rewritten = vec![serialize_volume("${PROJECT_PATH}", WORKSPACE_MOUNT)];
        let mut saw_data_mount = false;

        let volume_entries: Vec<Value> = service
            .remove("volumes")
            .and_then(|v| v.as_sequence().cloned())
            .unwrap_or_default();
        if let Some(environment) = service.get("environment") {
            check_environment(&service_name, environment)?;
        }
        // Agent-authored values must not interpolate host variables; the writer's own values
        // (volumes, limits, defaults) are inserted after this.
        for value in service.values_mut() {
            escape_value(value);
        }
        for entry in &volume_entries {
            let spec = match entry {
                Value::String(text) => text.clone(),
                Value::Mapping(map) => format!(
                    "{}:{}",
                    map.get("source")
                        .filter(|v| !v.is_null())
                        .map(js_display)
                        .unwrap_or_default(),
                    map.get("target")
                        .filter(|v| !v.is_null())
                        .map(js_display)
                        .unwrap_or_default()
                ),
                _ => String::new(),
            };

            let mut parts = spec.split(':');
            let source = parts.next().unwrap_or("").to_string();
            let target = parts.collect::<Vec<_>>().join(":");

            if source.is_empty() || target.is_empty() {
                warnings.push(format!(
                    "Dropped unrecognized volume entry on \"{service_name}\": {spec}"
                ));
                continue;
            }
            if target == WORKSPACE_MOUNT {
                // Already added above.
                continue;
            }
            let target = escape_interpolation(&target);

            let is_named =
                declared_named_volumes.contains(&source) || !explicit_path.is_match(&source);
            if is_named {
                if !is_valid_volume_name(&source) {
                    return Err(Error::msg(format!(
                        "Service \"{service_name}\" mounts volume \"{source}\", which is not a valid volume name. Use letters, digits, dots, dashes, and underscores, or a path inside the project directory."
                    )));
                }
                rewritten.push(serialize_volume(&format!("./data/{source}"), &target));
                saw_data_mount = true;
                continue;
            }

            let outside = || {
                Error::msg(format!(
                    "Service \"{service_name}\" bind-mounts \"{source}\", which is outside the project directory. Sandbox mounts are limited to the project directory and the sandbox's own folder."
                ))
            };
            if source.contains('$') {
                // Compose would substitute variables into the host path.
                return Err(outside());
            }
            let absolute: PathBuf = resolve(sandbox_dir, Path::new(&source));
            if !is_contained(&absolute, &[&resolved_project, &resolved_sandbox]) {
                return Err(outside());
            }
            let rel = to_compose_path(&relative(&resolved_sandbox, &absolute));
            let host = if rel.starts_with('.') {
                rel
            } else {
                format!("./{rel}")
            };
            rewritten.push(serialize_volume(&host, &target));
            saw_data_mount = true;
        }

        if !saw_data_mount {
            rewritten.push(serialize_volume(&format!("./data/{service_name}"), "/data"));
        }

        service.insert(key("volumes"), string_seq(rewritten));
        if !service.contains_key("working_dir")
            || service.get("working_dir").is_some_and(Value::is_null)
        {
            service.insert(key("working_dir"), str_value(WORKSPACE_MOUNT));
        }
        if !service.contains_key("command") || service.get("command").is_some_and(Value::is_null) {
            service.insert(key("command"), str_value("sleep infinity"));
        }
        service.insert(key("mem_limit"), str_value(&config.memory_limit));
        service.insert(key("cpus"), Value::from(config.cpu_limit));

        let mut ports = Vec::new();
        let raw_ports: Vec<Value> = service
            .get("ports")
            .and_then(Value::as_sequence)
            .cloned()
            .unwrap_or_default();
        for raw in &raw_ports {
            let Some(mapping) = parse_port_mapping(raw) else {
                let shown = serde_json::to_string(raw).unwrap_or_else(|_| js_display(raw));
                return Err(Error::msg(format!(
                    "Service \"{service_name}\" has an unsupported port entry: {shown}. Use \"hostPort:containerPort\"."
                )));
            };
            ports.push(mapping);
        }
        if !ports.is_empty() {
            service.insert(
                key("ports"),
                string_seq(ports.iter().map(|p| format!("{}:{}", p.host, p.container))),
            );
        }

        let image = service.get("image").map(js_display).unwrap_or_default();
        summary.push(SandboxServiceSummary {
            name: service_name,
            image,
            ports,
        });
    }

    Ok(BuiltCompose {
        document: Value::Mapping(document),
        services: summary,
        warnings,
    })
}

/// Produce the compose document for a sandbox, from agent-supplied YAML or the structured
/// service list, falling back to a single bare ubuntu service.
pub fn build_compose(
    options: &CreateSandboxOptions,
    sandbox_dir: &Path,
    project_path: &Path,
    config: &DockerSandboxConfig,
) -> Result<BuiltCompose> {
    let mut built = build_services(options, sandbox_dir, project_path, config)?;
    if let Some(env) = options.env.as_ref().filter(|env| !env.is_empty()) {
        apply_agent_env(&mut built, env)?;
    }
    let defaults: Vec<(String, String)> = DEFAULT_SERVICE_ENV
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    merge_environment(&mut built, &defaults)?;
    Ok(built)
}

fn valid_service_name(name: &str) -> bool {
    static NAME: OnceLock<Regex> = OnceLock::new();
    NAME.get_or_init(|| Regex::new(r"^[a-z0-9][a-z0-9_-]*$").unwrap())
        .is_match(name)
}

fn build_services(
    options: &CreateSandboxOptions,
    sandbox_dir: &Path,
    project_path: &Path,
    config: &DockerSandboxConfig,
) -> Result<BuiltCompose> {
    if let Some(yaml) = options.compose_yaml.as_deref().filter(|y| !y.is_empty()) {
        return build_from_agent_yaml(yaml, sandbox_dir, project_path, config);
    }

    let services: Vec<SandboxServiceSpec> = match &options.services {
        Some(services) if !services.is_empty() => services.clone(),
        _ => vec![SandboxServiceSpec::named(DEFAULT_SERVICE_NAME)],
    };

    let mut names = std::collections::HashSet::new();
    for service in &services {
        if !valid_service_name(&service.name) {
            return Err(Error::msg(format!(
                "Invalid service name \"{}\". Use lowercase letters, digits, dashes, and underscores.",
                service.name
            )));
        }
        if !names.insert(service.name.clone()) {
            return Err(Error::msg(format!(
                "Duplicate service name \"{}\".",
                service.name
            )));
        }
        for extra in service.data_volumes.iter().flatten() {
            if !is_valid_volume_name(&extra.name) {
                return Err(Error::msg(format!(
                    "Invalid data volume name \"{}\" on service \"{}\". Use letters, digits, dots, dashes, and underscores.",
                    extra.name, service.name
                )));
            }
            if !extra.container_path.starts_with('/') {
                return Err(Error::msg(format!(
                    "Data volume \"{}\" on service \"{}\" needs an absolute container path, e.g. \"/cache\".",
                    extra.name, service.name
                )));
            }
        }
    }

    build_generated_compose(&services, config)
}

/// Environment every service gets unless it (or the agent's `env`) sets the name itself.
/// A bare ubuntu image prompts on apt without this, which blocks non-interactive runs.
pub const DEFAULT_SERVICE_ENV: [(&str, &str); 1] = [("DEBIAN_FRONTEND", "noninteractive")];

/// The name and value rules for variables passed to a container (agent `env`, and the
/// composeless `docker run -e` arguments read back from the compose file).
fn check_env_entry(name: &str, text: &str) -> Result<()> {
    if !is_valid_env_name(name) {
        return Err(Error::msg(format!(
            "Invalid env variable name \"{name}\". Use letters, digits, and underscores, not starting with a digit."
        )));
    }
    if text.contains('\0') {
        return Err(Error::msg(format!(
            "env variable \"{name}\" contains a NUL character."
        )));
    }
    Ok(())
}

/// Add the agent's `env` to every service's `environment`. The TS appended it to the `.env`
/// file after the writer's own `PROJECT_PATH=` line, so `PROJECT_PATH=/` (or a value with a
/// newline) re-pointed the workspace bind mount at the host root. Service `environment`
/// entries only ever reach the container. A service's own value for a name wins.
fn apply_agent_env(built: &mut BuiltCompose, env: &JsonMap<String, JsonValue>) -> Result<()> {
    let mut entries = Vec::new();
    for (name, value) in env {
        // Name first, so a bad name is reported before a bad value.
        check_env_entry(name, "")?;
        let text = match value {
            JsonValue::String(s) => s.clone(),
            JsonValue::Null => String::new(),
            JsonValue::Bool(_) | JsonValue::Number(_) => value.to_string(),
            _ => {
                return Err(Error::msg(format!(
                    "env variable \"{name}\" must be a string, number, or boolean."
                )))
            }
        };
        check_env_entry(name, &text)?;
        entries.push((name.clone(), escape_interpolation(&text)));
    }
    merge_environment(built, &entries)
}

/// Add `entries` (already escaped) to every service's `environment`, keeping any name the
/// service already sets.
fn merge_environment(built: &mut BuiltCompose, entries: &[(String, String)]) -> Result<()> {
    let Some(services) = built
        .document
        .get_mut("services")
        .and_then(Value::as_mapping_mut)
    else {
        return Ok(());
    };
    for (service_key, service) in services.iter_mut() {
        let Value::Mapping(service) = service else {
            continue;
        };
        let environment = service
            .entry(key("environment"))
            .or_insert_with(|| Value::Mapping(Mapping::new()));
        if environment.is_null() {
            *environment = Value::Mapping(Mapping::new());
        }
        match environment {
            Value::Mapping(map) => {
                for (name, text) in entries {
                    if !map.contains_key(name.as_str()) {
                        map.insert(key(name), str_value(text));
                    }
                }
            }
            Value::Sequence(items) => {
                let present: Vec<String> = items
                    .iter()
                    .map(|item| js_display(item).split('=').next().unwrap_or("").to_string())
                    .collect();
                for (name, text) in entries {
                    if !present.contains(name) {
                        items.push(str_value(format!("{name}={text}")));
                    }
                }
            }
            _ => {
                return Err(Error::msg(format!(
                    "Service \"{}\" has an unsupported environment entry. Use a mapping or a list of \"NAME=value\".",
                    key_name(service_key)
                )))
            }
        }
    }
    Ok(())
}

/// A service's `environment` read back from a written compose file, as the `NAME`/value
/// pairs the container sees (Compose's `$$` escapes undone). Used by the composeless
/// `docker run` path. Entries follow the same rules as when the file was written.
pub fn service_environment(
    compose_yaml: &str,
    service_name: &str,
) -> Result<Vec<(String, String)>> {
    let document: Value = yaml_serde::from_str(compose_yaml)
        .map_err(|error| Error::msg(format!("Could not parse the compose file: {error}")))?;
    let Some(environment) = document
        .get("services")
        .and_then(|services| services.get(service_name))
        .and_then(|service| service.get("environment"))
    else {
        return Ok(Vec::new());
    };
    check_environment(service_name, environment)?;
    let unsupported = || {
        Error::msg(format!(
            "Service \"{service_name}\" has an unsupported environment entry. Use a mapping or a list of \"NAME=value\"."
        ))
    };
    let mut entries = Vec::new();
    match environment {
        Value::Mapping(map) => {
            for (name, value) in map {
                if matches!(value, Value::Mapping(_) | Value::Sequence(_)) {
                    return Err(unsupported());
                }
                entries.push((key_name(name), js_display(value)));
            }
        }
        Value::Sequence(items) => {
            for item in items {
                let text = js_display(item);
                let (name, value) = text.split_once('=').ok_or_else(unsupported)?;
                entries.push((name.to_string(), value.to_string()));
            }
        }
        _ => {}
    }
    entries
        .into_iter()
        .map(|(name, value)| {
            let value = value.replace("$$", "$");
            check_env_entry(&name, &value)?;
            Ok((name, value))
        })
        .collect()
}

/// Paths written by [`write_sandbox_files`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenFiles {
    pub compose_file: PathBuf,
    pub env_file: PathBuf,
}

/// Write the compose file, the .env file, and the data directories for a sandbox.
///
/// The `.env` file holds only the writer's own variables; agent-supplied variables are
/// service `environment` entries (see [`build_compose`]).
pub fn write_sandbox_files(
    sandbox_dir: &Path,
    project_name: &str,
    project_path: &str,
    built: &BuiltCompose,
) -> Result<WrittenFiles> {
    let project_root = Path::new(project_path);
    if !is_contained(sandbox_dir, &[project_root]) {
        return Err(Error::msg(format!(
            "The sandbox folder {} resolves outside the project directory. Remove the sandbox and try again.",
            sandbox_dir.display()
        )));
    }
    std::fs::create_dir_all(sandbox_dir)?;

    // Every host source must stay inside the project (the sandbox folder lives there too),
    // even after following symlinks the container may have planted in the project mount.
    // Checked before anything is created, since `create_dir_all` follows symlinks too.
    let mut to_create = Vec::new();
    if let Some(services) = built.services_mapping() {
        for (service_name, service) in services {
            let volumes = service
                .get("volumes")
                .and_then(Value::as_sequence)
                .cloned()
                .unwrap_or_default();
            for volume in volumes.iter().filter_map(Value::as_str) {
                let source = volume.split(':').next().unwrap_or("");
                if source == "${PROJECT_PATH}" {
                    continue;
                }
                let resolved = resolve(sandbox_dir, Path::new(source));
                let relative_source = source.starts_with("./") || source.starts_with("../");
                if !relative_source
                    || source.contains('$')
                    || !is_contained(&resolved, &[project_root])
                {
                    return Err(Error::msg(format!(
                        "Service \"{}\" mounts \"{source}\", which resolves outside the project directory.",
                        key_name(service_name)
                    )));
                }
                // Create every mapped data directory up front. Docker would create them as
                // root otherwise, which leaves folders the user can't delete.
                if source.starts_with("./") {
                    to_create.push(resolved);
                }
            }
            log::debug!(
                target: "docker:compose-writer",
                "Prepared service directories service={}",
                key_name(service_name)
            );
        }
    }
    for dir in to_create {
        std::fs::create_dir_all(dir)?;
    }

    let compose_file = sandbox_dir.join("docker-compose.yml");
    let dumped = yaml_serde::to_string(&built.document)
        .map_err(|error| Error::msg(format!("Could not serialize the compose file: {error}")))?;
    std::fs::write(
        &compose_file,
        [
            "# Generated by Bedrock Engineer for a chat-scoped Docker sandbox.",
            "# Edits are overwritten when the sandbox is recreated.",
            dumped.as_str(),
        ]
        .join("\n"),
    )?;

    let env_file = sandbox_dir.join(".env");
    let lines = [
        format!("COMPOSE_PROJECT_NAME={project_name}"),
        format!("PROJECT_PATH={project_path}"),
        // Kept for older tooling; containers get it from each service's `environment`
        // (see [`DEFAULT_SERVICE_ENV`]), since Compose never passes `.env` to them.
        "DEBIAN_FRONTEND=noninteractive".to_string(),
    ];
    std::fs::write(&env_file, format!("{}\n", lines.join("\n")))?;

    Ok(WrittenFiles {
        compose_file,
        env_file,
    })
}

#[cfg(test)]
mod tests {
    //! Port of `composeWriter.test.ts`.
    use super::*;
    use crate::types::DataVolumeSpec;
    use serde_json::json;

    const PROJECT_PATH: &str = "/tmp/example-project";

    fn sandbox_dir() -> PathBuf {
        Path::new(PROJECT_PATH)
            .join("docker-sandboxes")
            .join("session_1")
    }

    fn config() -> DockerSandboxConfig {
        DockerSandboxConfig::default()
    }

    fn build(options: CreateSandboxOptions) -> Result<BuiltCompose> {
        build_compose(&options, &sandbox_dir(), Path::new(PROJECT_PATH), &config())
    }

    fn yaml(compose_yaml: &str) -> Result<BuiltCompose> {
        build(CreateSandboxOptions {
            compose_yaml: Some(compose_yaml.to_string()),
            ..Default::default()
        })
    }

    fn service<'a>(built: &'a BuiltCompose, name: &str) -> &'a Mapping {
        built
            .services_mapping()
            .unwrap()
            .get(name)
            .and_then(Value::as_mapping)
            .unwrap()
    }

    fn strings(value: &Value) -> Vec<String> {
        value
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    fn err(result: Result<BuiltCompose>) -> String {
        result.expect_err("expected an error").to_string()
    }

    // describe('buildCompose - generated default')
    #[test]
    fn produces_a_single_bare_ubuntu_service_that_stays_alive() {
        let built = build(CreateSandboxOptions::default()).unwrap();
        let main = service(&built, "main");
        assert_eq!(
            main.get("image").unwrap().as_str(),
            Some(DEFAULT_SANDBOX_IMAGE)
        );
        assert_eq!(
            main.get("command").unwrap().as_str(),
            Some("sleep infinity")
        );
        assert_eq!(
            main.get("working_dir").unwrap().as_str(),
            Some(WORKSPACE_MOUNT)
        );
        assert_eq!(
            built.services,
            vec![SandboxServiceSummary {
                name: "main".into(),
                image: DEFAULT_SANDBOX_IMAGE.into(),
                ports: vec![]
            }]
        );
    }

    #[test]
    fn mounts_the_project_directory_read_write_and_uses_a_mapped_data_folder() {
        let built = build(CreateSandboxOptions::default()).unwrap();
        assert_eq!(
            strings(service(&built, "main").get("volumes").unwrap()),
            [
                format!("${{PROJECT_PATH}}:{WORKSPACE_MOUNT}"),
                "./data/main:/data".into()
            ]
        );
    }

    #[test]
    fn applies_the_configured_resource_limits() {
        let built = build_compose(
            &CreateSandboxOptions::default(),
            &sandbox_dir(),
            Path::new(PROJECT_PATH),
            &DockerSandboxConfig {
                memory_limit: "512m".into(),
                cpu_limit: 1.5,
                timeout: 60.0,
                terminal_acknowledged: None,
            },
        )
        .unwrap();
        let main = service(&built, "main");
        assert_eq!(main.get("mem_limit").unwrap().as_str(), Some("512m"));
        assert_eq!(main.get("cpus").unwrap().as_f64(), Some(1.5));
    }

    #[test]
    fn publishes_agent_declared_ports() {
        let built = build(CreateSandboxOptions {
            services: Some(vec![SandboxServiceSpec {
                ports: Some(vec![SandboxPortMapping {
                    host: 3000,
                    container: 3000,
                }]),
                ..SandboxServiceSpec::named("web")
            }]),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            strings(service(&built, "web").get("ports").unwrap()),
            ["3000:3000"]
        );
        assert_eq!(
            built.services[0].ports,
            [SandboxPortMapping {
                host: 3000,
                container: 3000
            }]
        );
    }

    #[test]
    fn rejects_invalid_and_duplicated_service_names() {
        let invalid = err(build(CreateSandboxOptions {
            services: Some(vec![SandboxServiceSpec::named("Bad Name")]),
            ..Default::default()
        }));
        assert!(invalid.contains("Invalid service name"), "{invalid}");

        let duplicate = err(build(CreateSandboxOptions {
            services: Some(vec![
                SandboxServiceSpec::named("api"),
                SandboxServiceSpec::named("api"),
            ]),
            ..Default::default()
        }));
        assert!(duplicate.contains("Duplicate service name"), "{duplicate}");
    }

    // describe('buildCompose - agent-authored YAML')
    #[test]
    fn rewrites_named_volumes_to_mapped_folders_under_data() {
        let built = yaml(
            "
volumes:
  pgdata: {}
services:
  db:
    image: postgres:17
    volumes:
      - pgdata:/var/lib/postgresql/data
",
        )
        .unwrap();
        assert!(built.document.get("volumes").is_none());
        assert!(strings(service(&built, "db").get("volumes").unwrap())
            .contains(&"./data/pgdata:/var/lib/postgresql/data".to_string()));
        assert!(built.warnings.join(" ").contains("pgdata"));
    }

    #[test]
    fn always_adds_the_workspace_mount() {
        let built = yaml("services:\n  app:\n    image: ubuntu:26.04\n").unwrap();
        assert_eq!(
            strings(service(&built, "app").get("volumes").unwrap())[0],
            format!("${{PROJECT_PATH}}:{WORKSPACE_MOUNT}")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_bind_mount_outside_the_project_directory() {
        let message = err(yaml(
            "services:\n  app:\n    image: ubuntu:26.04\n    volumes:\n      - /:/host\n",
        ));
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn accepts_a_bind_mount_inside_the_project_directory() {
        let built = yaml(&format!(
            "services:\n  app:\n    image: ubuntu:26.04\n    volumes:\n      - {PROJECT_PATH}/assets:/assets\n"
        ))
        .unwrap();
        assert!(strings(service(&built, "app").get("volumes").unwrap())
            .contains(&"../../assets:/assets".to_string()));
    }

    #[test]
    fn rejects_keys_that_weaken_isolation() {
        for (key, compose_yaml) in [
            ("privileged", "services:\n  app:\n    image: ubuntu:26.04\n    privileged: true\n"),
            ("cap_add", "services:\n  app:\n    image: ubuntu:26.04\n    cap_add:\n      - SYS_ADMIN\n"),
            ("pid", "services:\n  app:\n    image: ubuntu:26.04\n    pid: host\n"),
            (
                "security_opt",
                "services:\n  app:\n    image: ubuntu:26.04\n    security_opt:\n      - seccomp=unconfined\n",
            ),
        ] {
            let message = err(yaml(compose_yaml));
            assert!(message.contains(key), "rejects {key}: {message}");
        }
    }

    #[test]
    fn rejects_host_networking() {
        let message = err(yaml(
            "services:\n  app:\n    image: ubuntu:26.04\n    network_mode: host\n",
        ));
        assert!(
            message.contains("Only the default bridge network"),
            "{message}"
        );
    }

    #[test]
    fn rejects_build_directives() {
        let message = err(yaml("services:\n  app:\n    build: .\n"));
        assert!(message.contains("prebuilt images only"), "{message}");
    }

    #[test]
    fn rejects_yaml_with_no_services() {
        let message = err(yaml("version: \"3\"\n"));
        assert!(message.contains("at least one service"), "{message}");
    }

    #[test]
    fn rejects_malformed_yaml_with_a_useful_message() {
        let message = err(yaml("services: [unclosed"));
        assert!(message.contains("not valid YAML"), "{message}");
    }

    #[test]
    fn normalizes_port_strings_including_an_ip_prefixed_form() {
        let built = yaml(
            "services:\n  app:\n    image: ubuntu:26.04\n    ports:\n      - \"127.0.0.1:8080:80\"\n",
        )
        .unwrap();
        assert_eq!(
            built.services[0].ports,
            [SandboxPortMapping {
                host: 8080,
                container: 80
            }]
        );
        assert_eq!(
            strings(service(&built, "app").get("ports").unwrap()),
            ["8080:80"]
        );
    }

    #[test]
    fn overrides_resource_limits_the_agent_tried_to_set() {
        let built =
            yaml("services:\n  app:\n    image: ubuntu:26.04\n    mem_limit: 64g\n    cpus: 32\n")
                .unwrap();
        let app = service(&built, "app");
        assert_eq!(
            app.get("mem_limit").unwrap().as_str(),
            Some(config().memory_limit.as_str())
        );
        assert_eq!(app.get("cpus").unwrap().as_f64(), Some(config().cpu_limit));
    }

    // describe('writeSandboxFiles')
    #[test]
    fn writes_the_compose_file_env_file_and_data_directories() {
        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_sandbox = tmp_project
            .path()
            .join("docker-sandboxes")
            .join("session_1");
        let mut env = JsonMap::new();
        env.insert("EXTRA".into(), JsonValue::String("value".into()));
        let built = build_compose(
            &CreateSandboxOptions {
                services: Some(vec![SandboxServiceSpec {
                    data_volumes: Some(vec![DataVolumeSpec {
                        name: "cache".into(),
                        container_path: "/cache".into(),
                    }]),
                    ..SandboxServiceSpec::named("main")
                }]),
                env: Some(env),
                ..Default::default()
            },
            &tmp_sandbox,
            tmp_project.path(),
            &config(),
        )
        .unwrap();

        let project = tmp_project.path().to_string_lossy().to_string();
        let written =
            write_sandbox_files(&tmp_sandbox, "bedrock-sandbox-session-1", &project, &built)
                .unwrap();

        let parsed: Value =
            yaml_serde::from_str(&std::fs::read_to_string(&written.compose_file).unwrap()).unwrap();
        assert_eq!(
            parsed["services"]["main"]["image"].as_str(),
            Some(DEFAULT_SANDBOX_IMAGE)
        );
        // Agent variables reach the container through the service environment.
        assert_eq!(
            parsed["services"]["main"]["environment"]["EXTRA"].as_str(),
            Some("value")
        );

        let env_text = std::fs::read_to_string(&written.env_file).unwrap();
        assert!(env_text.contains("COMPOSE_PROJECT_NAME=bedrock-sandbox-session-1"));
        assert!(env_text.contains(&format!("PROJECT_PATH={project}")));
        // A bare ubuntu image prompts on apt without this.
        assert!(env_text.contains("DEBIAN_FRONTEND=noninteractive"));
        // The .env file only carries the writer's own variables now.
        assert!(!env_text.contains("EXTRA"));

        // Created up front so Docker does not make them root-owned.
        assert!(tmp_sandbox.join("data").join("main").exists());
        assert!(tmp_sandbox.join("data").join("cache").exists());
    }

    fn with_env(
        options: CreateSandboxOptions,
        pairs: &[(&str, JsonValue)],
    ) -> CreateSandboxOptions {
        CreateSandboxOptions {
            env: Some(
                pairs
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), v.clone()))
                    .collect(),
            ),
            ..options
        }
    }

    // Security: agent-supplied `env` used to be appended to .env after PROJECT_PATH.
    #[test]
    fn agent_env_cannot_repoint_the_workspace_mount() {
        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_sandbox = tmp_project.path().join("docker-sandboxes").join("s");
        let project = tmp_project.path().to_string_lossy().to_string();
        let options = with_env(
            CreateSandboxOptions::default(),
            &[
                ("PROJECT_PATH", json!("/")),
                (
                    "SNEAKY",
                    json!("x\nPROJECT_PATH=/\nCOMPOSE_FILE=/etc/x.yml"),
                ),
                ("HOMEDIR", json!("${HOME}")),
                ("PORT", json!(3000)),
            ],
        );
        let built = build_compose(&options, &tmp_sandbox, tmp_project.path(), &config()).unwrap();
        let written = write_sandbox_files(&tmp_sandbox, "p", &project, &built).unwrap();

        let env_text = std::fs::read_to_string(&written.env_file).unwrap();
        let project_lines: Vec<&str> = env_text
            .lines()
            .filter(|l| l.starts_with("PROJECT_PATH="))
            .collect();
        assert_eq!(project_lines, [format!("PROJECT_PATH={project}")]);
        assert!(!env_text.contains("COMPOSE_FILE"));
        assert!(!env_text.contains("SNEAKY"));

        let parsed: Value =
            yaml_serde::from_str(&std::fs::read_to_string(&written.compose_file).unwrap()).unwrap();
        let main = &parsed["services"]["main"];
        // The workspace mount still comes from the writer's own variable.
        assert_eq!(
            main["volumes"][0].as_str(),
            Some(format!("${{PROJECT_PATH}}:{WORKSPACE_MOUNT}").as_str())
        );
        // Values are container-only, and never interpolate host variables.
        assert_eq!(main["environment"]["PROJECT_PATH"].as_str(), Some("/"));
        assert_eq!(
            main["environment"]["SNEAKY"].as_str(),
            Some("x\nPROJECT_PATH=/\nCOMPOSE_FILE=/etc/x.yml")
        );
        assert_eq!(main["environment"]["HOMEDIR"].as_str(), Some("$${HOME}"));
        assert_eq!(main["environment"]["PORT"].as_str(), Some("3000"));
    }

    #[test]
    fn rejects_invalid_agent_env_names_and_values() {
        for name in ["BAD NAME", "1ABC", "A=B", "X\nPROJECT_PATH", ""] {
            let message = err(build(with_env(
                CreateSandboxOptions::default(),
                &[(name, json!("v"))],
            )));
            assert!(
                message.contains("Invalid env variable name"),
                "{name}: {message}"
            );
        }
        let message = err(build(with_env(
            CreateSandboxOptions::default(),
            &[("OK", json!("a\u{0}b"))],
        )));
        assert!(message.contains("NUL"), "{message}");
        let message = err(build(with_env(
            CreateSandboxOptions::default(),
            &[("OK", json!({"nested": true}))],
        )));
        assert!(message.contains("must be a string"), "{message}");
    }

    #[test]
    fn agent_env_merges_into_agent_yaml_services_without_overriding_them() {
        let built = build(with_env(
            CreateSandboxOptions {
                compose_yaml: Some(
                    "services:\n  a:\n    image: x\n    environment:\n      KEEP: mine\n  b:\n    image: y\n    environment:\n      - LIST=1\n"
                        .into(),
                ),
                ..Default::default()
            },
            &[("KEEP", json!("theirs")), ("NEW", json!("n"))],
        ))
        .unwrap();
        let a = service(&built, "a").get("environment").unwrap();
        assert_eq!(a["KEEP"].as_str(), Some("mine"));
        assert_eq!(a["NEW"].as_str(), Some("n"));
        assert_eq!(
            strings(service(&built, "b").get("environment").unwrap()),
            [
                "LIST=1",
                "KEEP=theirs",
                "NEW=n",
                "DEBIAN_FRONTEND=noninteractive"
            ]
        );
    }

    // DEBIAN_FRONTEND used to be written only to `.env`, which Compose never passes on.
    #[test]
    fn sets_debian_frontend_in_every_service_without_overriding_it() {
        let built = build(CreateSandboxOptions::default()).unwrap();
        assert_eq!(
            service(&built, DEFAULT_SERVICE_NAME)
                .get("environment")
                .unwrap()["DEBIAN_FRONTEND"]
                .as_str(),
            Some("noninteractive")
        );

        let built = yaml(
            "services:\n  a:\n    image: x\n  b:\n    image: y\n    environment:\n      DEBIAN_FRONTEND: readline\n  c:\n    image: z\n    environment:\n      - DEBIAN_FRONTEND=teletype\n",
        )
        .unwrap();
        let env = |name| service(&built, name).get("environment").unwrap().clone();
        assert_eq!(env("a")["DEBIAN_FRONTEND"].as_str(), Some("noninteractive"));
        assert_eq!(env("b")["DEBIAN_FRONTEND"].as_str(), Some("readline"));
        assert_eq!(strings(&env("c")), ["DEBIAN_FRONTEND=teletype"]);

        // The agent's `env` wins over the default too.
        let built = build(with_env(
            CreateSandboxOptions::default(),
            &[("DEBIAN_FRONTEND", json!("dialog"))],
        ))
        .unwrap();
        assert_eq!(
            service(&built, DEFAULT_SERVICE_NAME)
                .get("environment")
                .unwrap()["DEBIAN_FRONTEND"]
                .as_str(),
            Some("dialog")
        );
    }

    #[test]
    fn reads_service_environment_back_from_the_written_compose_file() {
        let tmp_project = tempfile::tempdir().unwrap();
        let tmp_sandbox = tmp_project.path().join("docker-sandboxes").join("s");
        let project = tmp_project.path().to_string_lossy().to_string();
        let mut environment = JsonMap::new();
        environment.insert("OWN".into(), json!("a $b"));
        environment.insert("NUM".into(), json!(3));
        let options = with_env(
            CreateSandboxOptions {
                services: Some(vec![SandboxServiceSpec {
                    environment: Some(environment),
                    ..SandboxServiceSpec::named("main")
                }]),
                ..Default::default()
            },
            &[("EXTRA", json!("${HOME}")), ("OWN", json!("ignored"))],
        );
        let built = build_compose(&options, &tmp_sandbox, tmp_project.path(), &config()).unwrap();
        let written = write_sandbox_files(&tmp_sandbox, "p", &project, &built).unwrap();
        let text = std::fs::read_to_string(&written.compose_file).unwrap();
        let mut env = service_environment(&text, "main").unwrap();
        env.sort();
        let pair = |n: &str, v: &str| (n.to_string(), v.to_string());
        assert_eq!(
            env,
            [
                pair("DEBIAN_FRONTEND", "noninteractive"),
                pair("EXTRA", "${HOME}"),
                pair("NUM", "3"),
                pair("OWN", "a $b"),
            ]
        );
        assert!(service_environment(&text, "missing").unwrap().is_empty());

        // The writer's rules apply to what is read back.
        let bad = "services:\n  main:\n    environment:\n      - SECRET\n";
        assert!(service_environment(bad, "main").is_err());
        let bad = "services:\n  main:\n    environment:\n      \"BAD NAME\": x\n";
        let message = service_environment(bad, "main").unwrap_err().to_string();
        assert!(message.contains("Invalid env variable name"), "{message}");
    }

    // Security: named-volume sources were joined onto ./data/ without containment.
    #[test]
    fn rejects_named_volume_traversal() {
        for source in ["x/../../../..", "a/b", "vol..x", "-flag"] {
            let message = err(yaml(&format!(
                "services:\n  app:\n    image: ubuntu:26.04\n    volumes:\n      - \"{source}:/host\"\n"
            )));
            assert!(
                message.contains("not a valid volume name"),
                "{source}: {message}"
            );
        }
        let built = yaml(
            "services:\n  app:\n    image: x\n    volumes:\n      - pg_data.v1-2:/var/lib/data\n",
        )
        .unwrap();
        assert!(strings(service(&built, "app").get("volumes").unwrap())
            .contains(&"./data/pg_data.v1-2:/var/lib/data".to_string()));
    }

    #[test]
    fn rejects_data_volume_traversal_in_structured_services() {
        let volume = |name: &str, path: &str| {
            build(CreateSandboxOptions {
                services: Some(vec![SandboxServiceSpec {
                    data_volumes: Some(vec![DataVolumeSpec {
                        name: name.into(),
                        container_path: path.into(),
                    }]),
                    ..SandboxServiceSpec::named("main")
                }]),
                ..Default::default()
            })
        };
        let message = err(volume("../../../../etc", "/x"));
        assert!(message.contains("Invalid data volume name"), "{message}");
        let message = err(volume("cache", "relative"));
        assert!(message.contains("absolute container path"), "{message}");
        assert!(volume("cache", "/cache").is_ok());
    }

    #[test]
    fn rejects_agent_yaml_service_names_that_escape_the_data_folder() {
        let message = err(yaml("services:\n  \"../../x\":\n    image: ubuntu:26.04\n"));
        assert!(message.contains("Invalid service name"), "{message}");
    }

    #[test]
    fn rejects_top_level_keys_outside_the_allowlist() {
        for (key, compose_yaml) in [
            (
                "secrets",
                "secrets:\n  s:\n    file: /etc/shadow\nservices:\n  app:\n    image: x\n",
            ),
            (
                "configs",
                "configs:\n  c:\n    file: ~/.aws/credentials\nservices:\n  app:\n    image: x\n",
            ),
            (
                "include",
                "include:\n  - /etc/other.yml\nservices:\n  app:\n    image: x\n",
            ),
            (
                "networks",
                "networks:\n  n:\n    external: true\nservices:\n  app:\n    image: x\n",
            ),
            (
                "name",
                "name: other-project\nservices:\n  app:\n    image: x\n",
            ),
        ] {
            let message = err(yaml(compose_yaml));
            assert!(
                message.contains(&format!("top-level \"{key}\"")),
                "rejects {key}: {message}"
            );
        }
    }

    #[test]
    fn rejects_service_keys_outside_the_allowlist() {
        for (key, body) in [
            ("env_file", "    env_file: /Users/me/.env\n"),
            ("volumes_from", "    volumes_from:\n      - other\n"),
            (
                "extends",
                "    extends:\n      file: /etc/x.yml\n      service: y\n",
            ),
            ("secrets", "    secrets:\n      - s\n"),
            (
                "deploy",
                "    deploy:\n      resources:\n        limits:\n          memory: 64g\n",
            ),
            ("devices", "    devices:\n      - /dev/sda\n"),
            ("userns_mode", "    userns_mode: host\n"),
            ("networks", "    networks:\n      - host\n"),
            (
                "labels",
                "    labels:\n      com.docker.compose.project: other\n",
            ),
            ("sysctls", "    sysctls:\n      net.ipv4.ip_forward: 1\n"),
        ] {
            let message = err(yaml(&format!(
                "services:\n  app:\n    image: ubuntu:26.04\n{body}"
            )));
            assert!(
                message.contains(&format!("sets \"{key}\"")),
                "rejects {key}: {message}"
            );
        }
    }

    #[test]
    fn merge_keys_are_expanded_before_the_allowlist() {
        let message = err(yaml(
            "x-evil: &evil\n  privileged: true\nservices:\n  app:\n    <<: *evil\n    image: x\n",
        ));
        assert!(message.contains("sets \"privileged\""), "{message}");

        let built = yaml(
            "x-base: &base\n  image: node:22\n  restart: unless-stopped\nservices:\n  app:\n    <<: *base\n    command: npm start\n",
        )
        .unwrap();
        assert!(built.document.get("x-base").is_none());
        let app = service(&built, "app");
        assert_eq!(app.get("image").unwrap().as_str(), Some("node:22"));
        assert_eq!(app.get("restart").unwrap().as_str(), Some("unless-stopped"));
    }

    #[test]
    fn rejects_non_string_network_modes() {
        let message = err(yaml(
            "services:\n  app:\n    image: x\n    network_mode:\n      - host\n",
        ));
        assert!(
            message.contains("Only the default bridge network"),
            "{message}"
        );
        assert!(yaml("services:\n  app:\n    image: x\n    network_mode: bridge\n").is_ok());
    }

    #[test]
    fn rejects_environment_passthrough_from_the_host() {
        let message = err(yaml(
            "services:\n  app:\n    image: x\n    environment:\n      - AWS_SECRET_ACCESS_KEY\n",
        ));
        assert!(message.contains("AWS_SECRET_ACCESS_KEY"), "{message}");
        let message = err(yaml(
            "services:\n  app:\n    image: x\n    environment:\n      AWS_SECRET_ACCESS_KEY:\n",
        ));
        assert!(message.contains("without a value"), "{message}");

        let mut environment = JsonMap::new();
        environment.insert("TOKEN".into(), JsonValue::Null);
        let message = err(build(CreateSandboxOptions {
            services: Some(vec![SandboxServiceSpec {
                environment: Some(environment),
                ..SandboxServiceSpec::named("main")
            }]),
            ..Default::default()
        }));
        assert!(message.contains("without a value"), "{message}");
    }

    #[test]
    fn neutralizes_host_variable_interpolation_in_agent_yaml() {
        let built = yaml(
            "services:\n  app:\n    image: ubuntu:26.04\n    command: sh -c 'echo ${AWS_SECRET_ACCESS_KEY} $HOME'\n    environment:\n      KEY: ${AWS_SECRET_ACCESS_KEY}\n    healthcheck:\n      test: [\"CMD\", \"echo\", \"$PATH\"]\n    volumes:\n      - data:/srv/${USER}\n",
        )
        .unwrap();
        let app = service(&built, "app");
        assert_eq!(
            app.get("command").unwrap().as_str(),
            Some("sh -c 'echo $${AWS_SECRET_ACCESS_KEY} $$HOME'")
        );
        assert_eq!(
            app.get("environment").unwrap()["KEY"].as_str(),
            Some("$${AWS_SECRET_ACCESS_KEY}")
        );
        assert_eq!(
            app.get("healthcheck").unwrap()["test"][2].as_str(),
            Some("$$PATH")
        );
        let volumes = strings(app.get("volumes").unwrap());
        // The writer's own variable is left for Compose to substitute.
        assert_eq!(volumes[0], format!("${{PROJECT_PATH}}:{WORKSPACE_MOUNT}"));
        assert!(volumes.contains(&"./data/data:/srv/$${USER}".to_string()));

        let built = build(CreateSandboxOptions {
            services: Some(vec![SandboxServiceSpec {
                command: Some("echo $HOME".into()),
                ..SandboxServiceSpec::named("main")
            }]),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            service(&built, "main").get("command").unwrap().as_str(),
            Some("echo $$HOME")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_bind_sources_with_variables() {
        let message = err(yaml(
            "services:\n  app:\n    image: x\n    volumes:\n      - ./${HOME}:/h\n",
        ));
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
    }

    /// The container can plant `ln -s / /workspace/escape` in the project mount; a lexically
    /// contained bind source through it must still be rejected.
    #[cfg(unix)]
    #[test]
    fn rejects_bind_mounts_through_symlinks_that_leave_the_project() {
        let outside = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let sandbox = project.path().join("docker-sandboxes").join("s");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::os::unix::fs::symlink(outside.path(), project.path().join("escape")).unwrap();
        std::fs::create_dir(project.path().join("assets")).unwrap();

        let build_with = |compose_yaml: &str| {
            build_compose(
                &CreateSandboxOptions {
                    compose_yaml: Some(compose_yaml.into()),
                    ..Default::default()
                },
                &sandbox,
                project.path(),
                &config(),
            )
        };
        let message = err(build_with(
            "services:\n  app:\n    image: x\n    volumes:\n      - ../../escape:/host\n",
        ));
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
        let message = err(build_with(
            "services:\n  app:\n    image: x\n    volumes:\n      - ../../escape/sub/dir:/host\n",
        ));
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
        // A real directory inside the project is still fine.
        let built = build_with(
            "services:\n  app:\n    image: x\n    volumes:\n      - ../../assets:/assets\n",
        )
        .unwrap();
        assert!(strings(service(&built, "app").get("volumes").unwrap())
            .contains(&"../../assets:/assets".to_string()));
    }

    /// `data/` lives inside the project mount too, so the container can swap it for a symlink.
    /// Writing must refuse instead of creating directories wherever it points.
    #[cfg(unix)]
    #[test]
    fn write_refuses_data_folders_redirected_by_symlinks() {
        let outside = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let sandbox = project.path().join("docker-sandboxes").join("s");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::os::unix::fs::symlink(outside.path(), sandbox.join("data")).unwrap();

        let built = build_compose(
            &CreateSandboxOptions::default(),
            &sandbox,
            project.path(),
            &config(),
        )
        .unwrap();
        let message = write_sandbox_files(&sandbox, "p", &project.path().to_string_lossy(), &built)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
        assert!(!outside.path().join("main").exists());
    }

    #[test]
    fn write_rejects_sources_the_builder_never_emits() {
        let project = tempfile::tempdir().unwrap();
        let sandbox = project.path().join("docker-sandboxes").join("s");
        let mut built = build(CreateSandboxOptions::default()).unwrap();
        let services = built
            .document
            .get_mut("services")
            .and_then(Value::as_mapping_mut)
            .unwrap();
        services.get_mut("main").unwrap()["volumes"] =
            string_seq(["./data/../../../../../../x:/x".to_string()]);
        let message = write_sandbox_files(&sandbox, "p", &project.path().to_string_lossy(), &built)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("outside the project directory"),
            "{message}"
        );
    }

    #[test]
    fn assert_ports_available_rejects_duplicates_and_bound_ports() {
        let summary = |name: &str, host: u16| SandboxServiceSummary {
            name: name.into(),
            image: DEFAULT_SANDBOX_IMAGE.into(),
            ports: vec![SandboxPortMapping {
                host,
                container: 80,
            }],
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bound = listener.local_addr().unwrap().port();
        let message = assert_ports_available(&[summary("a", bound)])
            .unwrap_err()
            .to_string();
        assert!(message.contains("already in use"), "{message}");

        drop(listener);
        let message = assert_ports_available(&[summary("a", bound), summary("b", bound)])
            .unwrap_err()
            .to_string();
        assert!(message.contains("requested by both"), "{message}");
    }
}
