//! Minimal Docker Engine API client over the local socket. Port of
//! `src/main/api/docker/dockerEngine.ts`.
//!
//! Everything else goes through the `docker` CLI. The interactive terminal cannot: `docker
//! exec -t` refuses a TTY when its own stdio is a pipe, and only the Engine API can resize a
//! running exec. Hand-rolled HTTP/1.1 over a Unix socket or Windows named pipe — five
//! endpoints do not justify a Docker client library.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::runner::{CommandRunner, RunOptions};
use crate::util::now_ms;
use crate::{Error, Result};

const REQUEST_TIMEOUT: Duration = Duration::from_millis(10_000);
const ENDPOINT_TTL: Duration = Duration::from_millis(15_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockerEndpointScheme {
    Unix,
    Npipe,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointSource {
    #[serde(rename = "DOCKER_HOST")]
    DockerHost,
    #[serde(rename = "docker context")]
    DockerContext,
    #[serde(rename = "platform default")]
    PlatformDefault,
}

/// A parsed endpoint string (`parseDockerHost`'s return, without `source`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDockerHost {
    /// The raw endpoint string, e.g. `unix:///var/run/docker.sock`.
    pub host: String,
    pub scheme: DockerEndpointScheme,
    /// Path to connect to, or `None` for a remote daemon we cannot attach a terminal to.
    pub socket_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerEndpoint {
    pub host: String,
    pub scheme: DockerEndpointScheme,
    pub socket_path: Option<String>,
    /// Where the endpoint came from, for diagnostics.
    pub source: EndpointSource,
}

impl DockerEndpoint {
    fn from_parsed(parsed: ParsedDockerHost, source: EndpointSource) -> Self {
        Self {
            host: parsed.host,
            scheme: parsed.scheme,
            socket_path: parsed.socket_path,
            source,
        }
    }
}

/// Parse a Docker endpoint string into something we can connect to.
///
/// `tcp://` and `ssh://` resolve to `remote` with no socket path: a remote daemon cannot
/// bind-mount the user's project at /workspace either, so the sandbox assumes a local one.
pub fn parse_docker_host(host: &str) -> ParsedDockerHost {
    let trimmed = host.trim();

    if let Some(path) = trimmed.strip_prefix("unix://") {
        return ParsedDockerHost {
            host: trimmed.to_string(),
            scheme: DockerEndpointScheme::Unix,
            socket_path: Some(path.to_string()),
        };
    }

    if let Some(path) = trimmed.strip_prefix("npipe://") {
        // `npipe:////./pipe/docker_engine` -> `\\.\pipe\docker_engine`
        return ParsedDockerHost {
            host: trimmed.to_string(),
            scheme: DockerEndpointScheme::Npipe,
            socket_path: Some(path.replace('/', "\\")),
        };
    }

    // A bare path is what some setups put in DOCKER_HOST.
    if trimmed.starts_with('/') || trimmed.starts_with("\\\\") {
        return ParsedDockerHost {
            host: trimmed.to_string(),
            scheme: if cfg!(windows) {
                DockerEndpointScheme::Npipe
            } else {
                DockerEndpointScheme::Unix
            },
            socket_path: Some(trimmed.to_string()),
        };
    }

    ParsedDockerHost {
        host: trimmed.to_string(),
        scheme: DockerEndpointScheme::Remote,
        socket_path: None,
    }
}

fn platform_default_host() -> &'static str {
    if cfg!(windows) {
        "npipe:////./pipe/docker_engine"
    } else {
        "unix:///var/run/docker.sock"
    }
}

/// Where `DOCKER_HOST` comes from: the process environment, or a fixed value (tests).
#[derive(Debug, Clone)]
enum DockerHostSource {
    Environment,
    Fixed(Option<String>),
}

/// Bidirectional byte stream to the daemon.
pub trait EngineIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> EngineIo for T {}

pub type EngineStream = Box<dyn EngineIo>;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineResponse {
    pub status: u16,
    /// Parsed JSON, the raw text when it is not JSON, or `null` for an empty body.
    pub body: Value,
}

/// An upgraded exec stream (`POST /exec/{id}/start` with `Upgrade: tcp`).
pub struct HijackedStream {
    /// The raw connection. Nothing is read from it until the caller does, which is the
    /// Rust equivalent of the TS handing back a paused socket.
    pub stream: EngineStream,
    pub status: u16,
    /// Stream bytes that shared the HTTP header's chunk — usually the first prompt.
    pub initial: Vec<u8>,
}

/// Engine API client. Holds the endpoint cache the TS kept module-global.
pub struct DockerEngine {
    runner: Arc<dyn CommandRunner>,
    docker_host: DockerHostSource,
    cached: Mutex<Option<(DockerEndpoint, Instant)>>,
}

impl DockerEngine {
    /// Reads `DOCKER_HOST` from the environment on each (uncached) resolution.
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            runner,
            docker_host: DockerHostSource::Environment,
            cached: Mutex::new(None),
        }
    }

    /// Uses `docker_host` in place of the `DOCKER_HOST` environment variable.
    pub fn with_docker_host(runner: Arc<dyn CommandRunner>, docker_host: Option<String>) -> Self {
        Self {
            runner,
            docker_host: DockerHostSource::Fixed(docker_host),
            cached: Mutex::new(None),
        }
    }

    fn docker_host_value(&self) -> Option<String> {
        match &self.docker_host {
            DockerHostSource::Environment => std::env::var("DOCKER_HOST")
                .ok()
                .filter(|value| !value.is_empty()),
            DockerHostSource::Fixed(value) => value.clone().filter(|value| !value.is_empty()),
        }
    }

    /// Locate the Docker socket: `DOCKER_HOST`, else the active docker context (which is
    /// what makes OrbStack, Colima, Rancher Desktop and rootless installs work), else the
    /// platform default.
    pub async fn resolve_endpoint(&self, force: bool) -> DockerEndpoint {
        if !force {
            if let Some((endpoint, at)) = self.cached.lock().unwrap().as_ref() {
                if at.elapsed() < ENDPOINT_TTL {
                    return endpoint.clone();
                }
            }
        }

        let endpoint = if let Some(host) = self.docker_host_value() {
            DockerEndpoint::from_parsed(parse_docker_host(&host), EndpointSource::DockerHost)
        } else {
            let args: Vec<String> = [
                "context",
                "inspect",
                "--format",
                "{{.Endpoints.docker.Host}}",
            ]
            .iter()
            .map(|arg| arg.to_string())
            .collect();
            let result = self
                .runner
                .run("docker", &args, RunOptions::timeout_ms(8000))
                .await;
            let host = if result.exit_code == 0 {
                result.stdout.trim().to_string()
            } else {
                String::new()
            };
            if host.is_empty() {
                DockerEndpoint::from_parsed(
                    parse_docker_host(platform_default_host()),
                    EndpointSource::PlatformDefault,
                )
            } else {
                DockerEndpoint::from_parsed(parse_docker_host(&host), EndpointSource::DockerContext)
            }
        };

        *self.cached.lock().unwrap() = Some((endpoint.clone(), Instant::now()));
        log::debug!(
            target: "docker:engine",
            "Resolved Docker endpoint host={} scheme={:?} source={:?}",
            endpoint.host,
            endpoint.scheme,
            endpoint.source
        );
        endpoint
    }

    /// Drop the cached endpoint. Used by tests and after an availability re-check.
    pub fn reset_endpoint_cache(&self) {
        *self.cached.lock().unwrap() = None;
    }

    async fn require_local_socket(&self) -> Result<String> {
        let endpoint = self.resolve_endpoint(false).await;
        endpoint.socket_path.ok_or_else(|| {
            Error::EngineUnavailable(format!(
                "This Docker context points at a remote daemon ({}). The in-app terminal needs a local Docker socket.",
                endpoint.host
            ))
        })
    }

    /// One JSON request against the Engine API. Resolves for any status code.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<EngineResponse> {
        let socket_path = self.require_local_socket().await?;
        let payload = body.map(|b| serde_json::to_vec(b).unwrap_or_default());

        let exchange = async {
            let mut stream = connect(&socket_path).await?;
            let mut head =
                format!("{method} {path} HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n");
            if let Some(payload) = &payload {
                head.push_str(&format!(
                    "content-type: application/json\r\ncontent-length: {}\r\n",
                    payload.len()
                ));
            }
            head.push_str("\r\n");
            stream.write_all(head.as_bytes()).await?;
            if let Some(payload) = &payload {
                stream.write_all(payload).await?;
            }
            stream.flush().await?;
            read_response(&mut stream).await
        };

        let (status, raw) = match tokio::time::timeout(REQUEST_TIMEOUT, exchange).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                return Err(Error::EngineUnavailable(describe_connect_error(
                    &error,
                    &socket_path,
                )))
            }
            Err(_) => {
                return Err(Error::EngineUnavailable(format!(
                    "Could not reach the Docker socket at {socket_path}: Docker API request timed out after {}ms",
                    REQUEST_TIMEOUT.as_millis()
                )))
            }
        };

        let text = String::from_utf8_lossy(&raw).into_owned();
        let body = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Ok(EngineResponse { status, body })
    }

    /// Upgrade a connection to a raw bidirectional stream.
    pub async fn hijack(&self, path: &str, body: &Value) -> Result<HijackedStream> {
        let socket_path = self.require_local_socket().await?;
        let payload = serde_json::to_vec(body).unwrap_or_default();

        let unavailable = |error: std::io::Error| {
            Error::EngineUnavailable(describe_connect_error(&error, &socket_path))
        };

        let mut stream = connect(&socket_path).await.map_err(unavailable)?;
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: docker\r\nContent-Type: application/json\r\nConnection: Upgrade\r\nUpgrade: tcp\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        );
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(unavailable)?;
        stream.write_all(&payload).await.map_err(unavailable)?;
        stream.flush().await.map_err(unavailable)?;

        let mut header = Vec::new();
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let read = stream.read(&mut chunk).await.map_err(unavailable)?;
            if read == 0 {
                return Err(unavailable(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed before the exec stream started",
                )));
            }
            header.extend_from_slice(&chunk[..read]);
            if let Some(split) = split_http_header(&header) {
                if split.status != 101 && split.status != 200 {
                    return Err(Error::EngineUnavailable(format!(
                        "Docker refused the exec stream (HTTP {})",
                        split.status
                    )));
                }
                return Ok(HijackedStream {
                    stream,
                    status: split.status,
                    initial: split.rest,
                });
            }
        }
    }

    pub async fn inspect_container(&self, name: &str) -> Result<ContainerInspect> {
        let response = self
            .request("GET", &format!("/containers/{name}/json"), None)
            .await?;
        if response.status != 200 {
            return Err(Error::EngineUnavailable(format!(
                "Could not inspect container {name} (HTTP {})",
                response.status
            )));
        }
        Ok(serde_json::from_value(response.body).unwrap_or_default())
    }

    /// One-shot resource sample. `one-shot=true` answers immediately but zeroes
    /// `precpu_stats`, so CPU% needs the caller to diff two samples.
    pub async fn container_stats(&self, name: &str) -> Result<ContainerSample> {
        let response = self
            .request(
                "GET",
                &format!("/containers/{name}/stats?stream=false&one-shot=true"),
                None,
            )
            .await?;
        if response.status != 200 {
            return Err(Error::EngineUnavailable(format!(
                "Could not read stats for {name} (HTTP {})",
                response.status
            )));
        }
        Ok(sample_from_stats(&response.body, now_ms()))
    }

    /// Containers of a compose project in `listProjectContainers`' shape, without a spawn.
    pub async fn list_project_containers(
        &self,
        project_name: &str,
    ) -> Result<Vec<crate::types::ContainerRow>> {
        let filters =
            serde_json::json!({ "label": [format!("com.docker.compose.project={project_name}")] });
        let path = format!(
            "/containers/json?all=1&filters={}",
            encode_uri_component(&filters.to_string())
        );
        let response = self.request("GET", &path, None).await?;
        let rows = match (&response.body, response.status) {
            (Value::Array(rows), 200) => rows,
            _ => {
                return Err(Error::EngineUnavailable(format!(
                    "Could not list containers for {project_name} (HTTP {})",
                    response.status
                )))
            }
        };

        Ok(rows
            .iter()
            .map(|container| {
                let service = container
                    .pointer("/Labels/com.docker.compose.service")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = container
                    .pointer("/Names/0")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_start_matches('/')
                    .to_string();
                let state = container
                    .get("State")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let ports = container
                    .get("Ports")
                    .and_then(Value::as_array)
                    .map(|ports| {
                        ports
                            .iter()
                            .filter(|port| {
                                port.get("PublicPort").and_then(Value::as_u64).unwrap_or(0) != 0
                            })
                            .map(|port| {
                                format!(
                                    "0.0.0.0:{}->{}/{}",
                                    port.get("PublicPort").and_then(Value::as_u64).unwrap_or(0),
                                    port.get("PrivatePort").map(js_string).unwrap_or_default(),
                                    port.get("Type").and_then(Value::as_str).unwrap_or("tcp")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                crate::types::ContainerRow {
                    service,
                    name,
                    state,
                    ports,
                }
            })
            .collect())
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "undefined".to_string(),
        other => other.to_string(),
    }
}

/// `encodeURIComponent`.
pub(crate) fn encode_uri_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(unix)]
async fn connect(socket_path: &str) -> std::io::Result<EngineStream> {
    let stream = tokio::net::UnixStream::connect(socket_path).await?;
    Ok(Box::new(stream))
}

#[cfg(windows)]
async fn connect(socket_path: &str) -> std::io::Result<EngineStream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    // ERROR_PIPE_BUSY: every instance is in use; retry briefly like Node does.
    const ERROR_PIPE_BUSY: i32 = 231;
    let mut attempts = 0;
    loop {
        match ClientOptions::new().open(socket_path) {
            Ok(client) => return Ok(Box::new(client)),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn describe_connect_error(error: &std::io::Error, socket_path: &str) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("No Docker socket at {socket_path}. Is Docker running?")
        }
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied opening the Docker socket at {socket_path}.")
        }
        _ => format!("Could not reach the Docker socket at {socket_path}: {error}"),
    }
}

/// The result of [`split_http_header`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitHeader {
    pub status: u16,
    pub headers: String,
    pub rest: Vec<u8>,
}

/// Split a buffer at the end of the HTTP response header. Bytes after `\r\n\r\n` in the
/// same chunk are already stream payload and must not be dropped.
pub fn split_http_header(buffer: &[u8]) -> Option<SplitHeader> {
    let index = buffer.windows(4).position(|window| window == b"\r\n\r\n")?;
    // latin1 decode: every byte maps to the code point of the same value.
    let headers: String = buffer[..index].iter().map(|&b| b as char).collect();
    let status_line = headers.split("\r\n").next().unwrap_or("");
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    Some(SplitHeader {
        status,
        headers,
        rest: buffer[index + 4..].to_vec(),
    })
}

/// Read a whole HTTP response: status and decoded body (Content-Length, chunked, or EOF).
async fn read_response(stream: &mut EngineStream) -> std::io::Result<(u16, Vec<u8>)> {
    let mut buffer = Vec::new();
    let mut chunk = vec![0u8; 16 * 1024];
    loop {
        if let Some(parsed) = try_parse_response(&buffer, false) {
            return Ok(parsed);
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return try_parse_response(&buffer, true).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "socket hang up")
            });
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.split("\r\n").skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// `None` when more bytes are needed (and `eof` is false).
fn try_parse_response(buffer: &[u8], eof: bool) -> Option<(u16, Vec<u8>)> {
    let split = split_http_header(buffer)?;
    let body = split.rest;

    let chunked = header_value(&split.headers, "transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    if chunked {
        return match decode_chunked(&body) {
            Some(decoded) => Some((split.status, decoded)),
            None if eof => Some((split.status, body)),
            None => None,
        };
    }

    if let Some(length) =
        header_value(&split.headers, "content-length").and_then(|v| v.parse::<usize>().ok())
    {
        if body.len() >= length {
            return Some((split.status, body[..length].to_vec()));
        }
        return eof.then_some((split.status, body));
    }

    // 1xx/204/304 have no body; otherwise the body runs to EOF.
    if split.status == 204 || split.status == 304 || (100..200).contains(&split.status) {
        return Some((split.status, Vec::new()));
    }
    eof.then_some((split.status, body))
}

/// Decode a complete chunked body, or `None` if the terminating chunk has not arrived.
fn decode_chunked(mut input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = input.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&input[..line_end]).ok()?;
        let size_hex = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16).ok()?;
        input = &input[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if input.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&input[..size]);
        input = &input[size + 2..];
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ContainerInspectConfig {
    pub image: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ContainerInspectState {
    pub status: String,
    pub running: bool,
    pub started_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ContainerInspect {
    pub id: String,
    pub name: String,
    pub config: ContainerInspectConfig,
    pub image: String,
    pub state: ContainerInspectState,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSample {
    /// When this sample was taken (epoch ms), for turning counters into rates.
    pub at: i64,
    /// Nanoseconds of CPU time used by the container, cumulative.
    pub cpu_total: u64,
    /// Nanoseconds of CPU time across the host, cumulative.
    pub system_total: u64,
    pub online_cpus: u64,
    pub memory_used: u64,
    pub memory_limit: u64,
    /// Cumulative bytes received / sent across every interface.
    pub net_rx: u64,
    pub net_tx: u64,
    /// Cumulative bytes read / written to block devices, or `None` when not reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_write: Option<u64>,
}

fn num(value: Option<&Value>) -> u64 {
    value
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f.max(0.0) as u64)))
        .unwrap_or(0)
}

/// Turn a raw `/stats` body into a sample. Memory subtracts `inactive_file`, which is what
/// `docker stats` reports.
pub fn sample_from_stats(raw: &Value, at: i64) -> ContainerSample {
    let usage = num(raw.pointer("/memory_stats/usage"));
    let inactive_file = num(raw.pointer("/memory_stats/stats/inactive_file"));

    // Summed across interfaces: a sandbox has one, but a compose service can have more.
    let (net_rx, net_tx) = raw
        .get("networks")
        .and_then(Value::as_object)
        .map(|interfaces| {
            interfaces.values().fold((0, 0), |(rx, tx), entry| {
                (
                    rx + num(entry.get("rx_bytes")),
                    tx + num(entry.get("tx_bytes")),
                )
            })
        })
        .unwrap_or((0, 0));

    let blkio = raw
        .pointer("/blkio_stats/io_service_bytes_recursive")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty());
    let sum_ops = |op: &str| -> Option<u64> {
        blkio.map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry
                        .get("op")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .eq_ignore_ascii_case(op)
                })
                .map(|entry| num(entry.get("value")))
                .sum()
        })
    };

    ContainerSample {
        at,
        cpu_total: num(raw.pointer("/cpu_stats/cpu_usage/total_usage")),
        system_total: num(raw.pointer("/cpu_stats/system_cpu_usage")),
        online_cpus: num(raw.pointer("/cpu_stats/online_cpus")),
        memory_used: usage.saturating_sub(inactive_file),
        memory_limit: num(raw.pointer("/memory_stats/limit")),
        net_rx,
        net_tx,
        block_read: sum_ops("read"),
        block_write: sum_ops("write"),
    }
}

/// CPU percentage between two samples, in `docker stats` units: 100% is one full core.
pub fn cpu_percent_between(previous: &ContainerSample, current: &ContainerSample) -> Option<f64> {
    let cpu_delta = current.cpu_total as f64 - previous.cpu_total as f64;
    let system_delta = current.system_total as f64 - previous.system_total as f64;
    if system_delta <= 0.0 || cpu_delta < 0.0 {
        return None;
    }
    let cpus = [current.online_cpus, previous.online_cpus]
        .into_iter()
        .find(|&n| n != 0)
        .unwrap_or(1);
    Some((cpu_delta / system_delta) * cpus as f64 * 100.0)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRates {
    /// Bytes per second since the previous sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_rx_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_tx_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_read_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_write_per_second: Option<f64>,
}

/// Turn two cumulative samples into per-second rates. A counter that went backwards
/// (container recreated) reports `None` rather than a nonsensical rate.
pub fn rates_between(previous: &ContainerSample, current: &ContainerSample) -> ContainerRates {
    let seconds = (current.at - previous.at) as f64 / 1000.0;
    if seconds <= 0.0 {
        return ContainerRates::default();
    }
    let rate = |before: Option<u64>, after: Option<u64>| -> Option<f64> {
        let (before, after) = (before?, after?);
        (after >= before).then(|| (after - before) as f64 / seconds)
    };
    ContainerRates {
        net_rx_per_second: rate(Some(previous.net_rx), Some(current.net_rx)),
        net_tx_per_second: rate(Some(previous.net_tx), Some(current.net_tx)),
        block_read_per_second: rate(previous.block_read, current.block_read),
        block_write_per_second: rate(previous.block_write, current.block_write),
    }
}

#[cfg(test)]
mod tests {
    //! Port of `dockerEngine.test.ts`. The `run` mock becomes a [`FakeRunner`] and
    //! `process.env.DOCKER_HOST` becomes [`DockerEngine::with_docker_host`].
    use super::*;
    use crate::runner::{FakeRunner, RunResult};

    // describe('parseDockerHost')
    #[test]
    fn reads_a_unix_socket_path() {
        assert_eq!(
            parse_docker_host("unix:///var/run/docker.sock"),
            ParsedDockerHost {
                host: "unix:///var/run/docker.sock".into(),
                scheme: DockerEndpointScheme::Unix,
                socket_path: Some("/var/run/docker.sock".into()),
            }
        );
    }

    #[test]
    fn handles_a_non_default_socket_location() {
        assert_eq!(
            parse_docker_host("unix:///Users/me/.orbstack/run/docker.sock").socket_path,
            Some("/Users/me/.orbstack/run/docker.sock".into())
        );
    }

    #[test]
    fn converts_a_windows_named_pipe_to_backslash_form() {
        assert_eq!(
            parse_docker_host("npipe:////./pipe/docker_engine"),
            ParsedDockerHost {
                host: "npipe:////./pipe/docker_engine".into(),
                scheme: DockerEndpointScheme::Npipe,
                socket_path: Some("\\\\.\\pipe\\docker_engine".into()),
            }
        );
    }

    #[test]
    fn accepts_a_bare_path() {
        assert_eq!(
            parse_docker_host("/run/user/1000/docker.sock").socket_path,
            Some("/run/user/1000/docker.sock".into())
        );
    }

    #[test]
    fn reports_tcp_and_ssh_endpoints_as_remote() {
        for host in ["tcp://10.0.4.19:2376", "ssh://user@build-host"] {
            let parsed = parse_docker_host(host);
            assert_eq!(parsed.scheme, DockerEndpointScheme::Remote);
            assert_eq!(parsed.socket_path, None);
        }
    }

    #[test]
    fn trims_surrounding_whitespace_from_cli_output() {
        assert_eq!(
            parse_docker_host("  unix:///var/run/docker.sock\n").socket_path,
            Some("/var/run/docker.sock".into())
        );
    }

    // describe('resolveDockerEndpoint')
    #[tokio::test]
    async fn prefers_docker_host_over_the_docker_context() {
        let runner = FakeRunner::new();
        let engine = DockerEngine::with_docker_host(
            runner.clone(),
            Some("unix:///tmp/from-env.sock".into()),
        );
        let endpoint = engine.resolve_endpoint(true).await;
        assert_eq!(endpoint.socket_path.as_deref(), Some("/tmp/from-env.sock"));
        assert_eq!(endpoint.source, EndpointSource::DockerHost);
        assert!(runner.calls().is_empty());
    }

    #[tokio::test]
    async fn asks_the_docker_context_when_docker_host_is_unset() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| RunResult::ok("unix:///Users/me/.orbstack/run/docker.sock\n"));
        let engine = DockerEngine::with_docker_host(runner.clone(), None);
        let endpoint = engine.resolve_endpoint(true).await;
        assert_eq!(
            runner.calls(),
            vec![(
                "docker".to_string(),
                vec![
                    "context".to_string(),
                    "inspect".into(),
                    "--format".into(),
                    "{{.Endpoints.docker.Host}}".into()
                ]
            )]
        );
        assert_eq!(
            endpoint.socket_path.as_deref(),
            Some("/Users/me/.orbstack/run/docker.sock")
        );
        assert_eq!(endpoint.source, EndpointSource::DockerContext);
    }

    #[tokio::test]
    async fn falls_back_to_the_platform_default_when_the_context_lookup_fails() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| RunResult::failed(1, "boom"));
        let engine = DockerEngine::with_docker_host(runner, None);
        let endpoint = engine.resolve_endpoint(true).await;
        assert_eq!(endpoint.source, EndpointSource::PlatformDefault);
        let expected = if cfg!(windows) {
            "\\\\.\\pipe\\docker_engine"
        } else {
            "/var/run/docker.sock"
        };
        assert_eq!(endpoint.socket_path.as_deref(), Some(expected));
    }

    #[tokio::test]
    async fn caches_the_endpoint_so_every_request_does_not_spawn_the_cli() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| RunResult::ok("unix:///tmp/cached.sock"));
        let engine = DockerEngine::with_docker_host(runner.clone(), None);
        engine.resolve_endpoint(true).await;
        engine.resolve_endpoint(false).await;
        engine.resolve_endpoint(false).await;
        assert_eq!(runner.calls().len(), 1);
    }

    // describe('splitHttpHeader')
    #[test]
    fn returns_none_until_the_whole_header_has_arrived() {
        assert!(split_http_header(b"HTTP/1.1 101 UPGRADED\r\nUpgrade: tcp").is_none());
    }

    #[test]
    fn keeps_payload_that_arrived_in_the_same_chunk_as_the_header() {
        let split = split_http_header(b"HTTP/1.1 101 UPGRADED\r\nUpgrade: tcp\r\n\r\nroot@abc:/# ")
            .unwrap();
        assert_eq!(split.status, 101);
        assert_eq!(split.rest, b"root@abc:/# ");
    }

    #[test]
    fn parses_a_plain_200_response() {
        assert_eq!(
            split_http_header(b"HTTP/1.1 200 OK\r\n\r\n")
                .unwrap()
                .status,
            200
        );
    }

    #[test]
    fn works_when_fed_one_byte_at_a_time() {
        let full = b"HTTP/1.1 101 UPGRADED\r\n\r\nX";
        let mut acc = Vec::new();
        let mut split = None;
        for byte in full {
            acc.push(*byte);
            split = split_http_header(&acc);
            if split.is_some() {
                break;
            }
        }
        let split = split.unwrap();
        assert_eq!(split.status, 101);
        // The trailing X arrives after the split, so it is not in `rest` yet.
        assert!(split.rest.is_empty());
    }

    #[test]
    fn does_not_mistake_a_body_only_crlf_pair_for_the_header_terminator() {
        let split =
            split_http_header(b"HTTP/1.1 101 UPGRADED\r\nA: b\r\n\r\nline1\r\nline2").unwrap();
        assert_eq!(split.rest, b"line1\r\nline2");
    }

    // describe('cpuPercentBetween')
    fn cpu_sample(cpu_total: u64, system_total: u64) -> ContainerSample {
        ContainerSample {
            cpu_total,
            system_total,
            online_cpus: 4,
            ..Default::default()
        }
    }

    #[test]
    fn reports_100_percent_for_one_fully_used_core() {
        let pct = cpu_percent_between(&cpu_sample(0, 0), &cpu_sample(25, 100)).unwrap();
        assert!((pct - 100.0).abs() < 1e-5);
    }

    #[test]
    fn goes_above_100_percent_for_more_than_one_core() {
        let pct = cpu_percent_between(&cpu_sample(0, 0), &cpu_sample(50, 100)).unwrap();
        assert!((pct - 200.0).abs() < 1e-5);
    }

    #[test]
    fn returns_none_when_the_system_delta_is_zero() {
        assert_eq!(
            cpu_percent_between(&cpu_sample(10, 100), &cpu_sample(10, 100)),
            None
        );
    }

    #[test]
    fn returns_none_when_counters_went_backwards() {
        assert_eq!(
            cpu_percent_between(&cpu_sample(90, 100), &cpu_sample(10, 200)),
            None
        );
    }

    // describe('ratesBetween')
    fn rate_sample(at: i64) -> ContainerSample {
        ContainerSample {
            at,
            online_cpus: 1,
            ..Default::default()
        }
    }

    #[test]
    fn turns_cumulative_byte_counters_into_per_second_rates() {
        let previous = ContainerSample {
            net_rx: 1000,
            net_tx: 500,
            block_read: Some(2000),
            block_write: Some(100),
            ..rate_sample(1000)
        };
        let current = ContainerSample {
            net_rx: 3000,
            net_tx: 1500,
            block_read: Some(6000),
            block_write: Some(300),
            ..rate_sample(3000)
        };
        let rates = rates_between(&previous, &current);
        assert_eq!(rates.net_rx_per_second, Some(1000.0));
        assert_eq!(rates.net_tx_per_second, Some(500.0));
        assert_eq!(rates.block_read_per_second, Some(2000.0));
        assert_eq!(rates.block_write_per_second, Some(100.0));
    }

    #[test]
    fn reports_no_disk_rate_on_hosts_that_do_not_report_block_io() {
        let rates = rates_between(
            &rate_sample(0),
            &ContainerSample {
                net_rx: 100,
                ..rate_sample(1000)
            },
        );
        assert_eq!(rates.block_read_per_second, None);
        assert_eq!(rates.block_write_per_second, None);
        assert_eq!(rates.net_rx_per_second, Some(100.0));
    }

    #[test]
    fn ignores_a_counter_that_went_backwards() {
        let rates = rates_between(
            &ContainerSample {
                net_rx: 5000,
                ..rate_sample(0)
            },
            &ContainerSample {
                net_rx: 10,
                ..rate_sample(1000)
            },
        );
        assert_eq!(rates.net_rx_per_second, None);
    }

    #[test]
    fn returns_nothing_when_two_samples_share_a_timestamp() {
        let rates = rates_between(
            &rate_sample(500),
            &ContainerSample {
                net_rx: 900,
                ..rate_sample(500)
            },
        );
        assert_eq!(rates, ContainerRates::default());
    }

    #[test]
    fn encodes_filters_like_encode_uri_component() {
        assert_eq!(
            encode_uri_component(r#"{"label":["a=b"]}"#),
            "%7B%22label%22%3A%5B%22a%3Db%22%5D%7D"
        );
    }

    #[test]
    fn decodes_chunked_bodies() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n[1,2\r\n2\r\n,3\r\n1\r\n]\r\n0\r\n\r\n";
        assert_eq!(
            try_parse_response(raw, false),
            Some((200, b"[1,2,3]".to_vec()))
        );
        assert_eq!(try_parse_response(&raw[..raw.len() - 5], false), None);
    }

    // describe('engine transport against a stub socket'). Unix sockets only; the Windows
    // named-pipe stub is not ported (see crate report).
    #[cfg(unix)]
    mod stub_socket {
        use super::*;
        use tokio::net::UnixListener;

        struct Stub {
            _dir: tempfile::TempDir,
            engine: DockerEngine,
            socket_path: std::path::PathBuf,
        }

        fn stub() -> Stub {
            let dir = tempfile::tempdir().unwrap();
            let socket_path = dir.path().join("docker.sock");
            let engine = DockerEngine::with_docker_host(
                FakeRunner::new(),
                Some(format!("unix://{}", socket_path.display())),
            );
            Stub {
                _dir: dir,
                engine,
                socket_path,
            }
        }

        /// Accept connections; once a request header has arrived, hand the socket to
        /// `handler`.
        fn listen<F, Fut>(path: &std::path::Path, handler: F)
        where
            F: Fn(tokio::net::UnixStream) -> Fut + Send + Sync + 'static,
            Fut: std::future::Future<Output = ()> + Send + 'static,
        {
            let listener = UnixListener::bind(path).unwrap();
            let handler = Arc::new(handler);
            tokio::spawn(async move {
                while let Ok((mut socket, _)) = listener.accept().await {
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        let mut request = Vec::new();
                        let mut buf = [0u8; 4096];
                        loop {
                            let n = socket.read(&mut buf).await.unwrap_or(0);
                            if n == 0 {
                                return;
                            }
                            request.extend_from_slice(&buf[..n]);
                            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        handler(socket).await;
                    });
                }
            });
        }

        fn ok_json(body: &str) -> String {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        }

        #[tokio::test]
        async fn parses_a_json_response_body() {
            let stub = stub();
            let body = serde_json::json!({
                "memory_stats": { "usage": 500, "limit": 2000, "stats": { "inactive_file": 100 } },
                "networks": { "eth0": { "rx_bytes": 700, "tx_bytes": 200 }, "eth1": { "rx_bytes": 300 } },
                "blkio_stats": {
                    "io_service_bytes_recursive": [
                        { "op": "Read", "value": 4096 },
                        { "op": "Write", "value": 8192 },
                        { "op": "Sync", "value": 999 }
                    ]
                }
            })
            .to_string();
            listen(&stub.socket_path, move |mut socket| {
                let response = ok_json(&body);
                async move {
                    socket.write_all(response.as_bytes()).await.unwrap();
                    socket.shutdown().await.ok();
                }
            });

            let stats = stub.engine.container_stats("whatever").await.unwrap();
            // usage - inactive_file, which is what `docker stats` reports.
            assert_eq!(stats.memory_used, 400);
            assert_eq!(stats.memory_limit, 2000);
            // Summed across interfaces.
            assert_eq!(stats.net_rx, 1000);
            assert_eq!(stats.net_tx, 200);
            // Only the read/write ops; Sync and friends would double-count.
            assert_eq!(stats.block_read, Some(4096));
            assert_eq!(stats.block_write, Some(8192));
        }

        #[tokio::test]
        async fn leaves_block_io_undefined_when_the_host_reports_an_empty_list() {
            let stub = stub();
            let body = serde_json::json!({
                "memory_stats": { "usage": 10, "limit": 100 },
                "blkio_stats": { "io_service_bytes_recursive": [] }
            })
            .to_string();
            listen(&stub.socket_path, move |mut socket| {
                let response = ok_json(&body);
                async move {
                    socket.write_all(response.as_bytes()).await.unwrap();
                    socket.shutdown().await.ok();
                }
            });

            let stats = stub.engine.container_stats("whatever").await.unwrap();
            assert_eq!(stats.block_read, None);
            // No networks key at all is normal for network_mode: none.
            assert_eq!(stats.net_rx, 0);
        }

        #[tokio::test]
        async fn surfaces_a_non_200_as_engine_unavailable_with_the_status() {
            let stub = stub();
            listen(&stub.socket_path, |mut socket| async move {
                socket
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
                socket.shutdown().await.ok();
            });

            let error = stub.engine.container_stats("missing").await.unwrap_err();
            assert!(error.is_engine_unavailable());
            assert!(error.to_string().contains("HTTP 404"), "{error}");
        }

        #[tokio::test]
        async fn hands_back_bytes_that_shared_the_header_chunk_and_an_unread_stream() {
            let stub = stub();
            listen(&stub.socket_path, |mut socket| async move {
                socket
                    .write_all(b"HTTP/1.1 101 UPGRADED\r\nUpgrade: tcp\r\n\r\nfirst-")
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
                socket.write_all(b"second").await.unwrap();
                // Keep the connection open until the client goes away.
                let mut sink = [0u8; 16];
                let _ = socket.read(&mut sink).await;
            });

            let mut hijacked = stub
                .engine
                .hijack(
                    "/exec/abc/start",
                    &serde_json::json!({ "Detach": false, "Tty": true }),
                )
                .await
                .unwrap();
            assert_eq!(hijacked.status, 101);
            // The shell's first prompt arrives with the header and must not be lost.
            assert_eq!(hijacked.initial, b"first-");

            let mut later = vec![0u8; 64];
            let n = hijacked.stream.read(&mut later).await.unwrap();
            assert_eq!(&later[..n], b"second");
        }

        #[tokio::test]
        async fn rejects_an_upgrade_the_daemon_refused() {
            let stub = stub();
            listen(&stub.socket_path, |mut socket| async move {
                socket
                    .write_all(b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
                socket.shutdown().await.ok();
            });

            let error = stub
                .engine
                .hijack("/exec/abc/start", &serde_json::json!({}))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().contains("HTTP 409"), "{error}");
        }

        #[tokio::test]
        async fn explains_a_missing_socket_instead_of_leaking_enoent() {
            let dir = tempfile::tempdir().unwrap();
            let engine = DockerEngine::with_docker_host(
                FakeRunner::new(),
                Some(format!(
                    "unix://{}",
                    dir.path().join("not-there.sock").display()
                )),
            );
            let error = engine.request("GET", "/_ping", None).await.unwrap_err();
            assert!(error.to_string().contains("Is Docker running?"), "{error}");
        }

        #[tokio::test]
        async fn lists_project_containers_via_the_engine() {
            let stub = stub();
            let body = serde_json::json!([{
                "Names": ["/bedrock-sandbox-s-main-1"],
                "State": "running",
                "Labels": { "com.docker.compose.service": "main" },
                "Ports": [
                    { "PrivatePort": 3000, "PublicPort": 3000, "Type": "tcp" },
                    { "PrivatePort": 9229, "Type": "tcp" }
                ]
            }])
            .to_string();
            listen(&stub.socket_path, move |mut socket| {
                let response = ok_json(&body);
                async move {
                    socket.write_all(response.as_bytes()).await.unwrap();
                    socket.shutdown().await.ok();
                }
            });
            let rows = stub
                .engine
                .list_project_containers("bedrock-sandbox-s")
                .await
                .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].name, "bedrock-sandbox-s-main-1");
            assert_eq!(rows[0].service, "main");
            assert_eq!(rows[0].ports, "0.0.0.0:3000->3000/tcp");
        }
    }

    // describe('remote endpoints')
    #[tokio::test]
    async fn remote_endpoints_refuse_to_open_a_stream_naming_the_reason() {
        let engine =
            DockerEngine::with_docker_host(FakeRunner::new(), Some("tcp://10.0.4.19:2376".into()));
        let error = engine
            .hijack("/exec/abc/start", &serde_json::json!({}))
            .await
            .err()
            .unwrap();
        assert!(error.is_engine_unavailable());
        assert!(error.to_string().contains("remote daemon"));
    }
}
