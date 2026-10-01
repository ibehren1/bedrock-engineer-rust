//! `window.api.dockerSandbox.*` (port of `src/main/handlers/docker-sandbox-handlers.ts`) and
//! `api.codeInterpreter.checkDockerAvailability` (`check-docker-availability` in
//! `util-handlers.ts`), over the shared `docker::SandboxManager`.
//!
//! Sandbox events (`docker-sandbox:state:*`, `docker-sandbox:activity:*`,
//! `docker-sandbox:terminal:*`) go out through pub/sub, see `backend::DockerToPubSub`. Terminal
//! bytes (live frames and backlogs) are `number[]` on the wire; the bridge turns backlogs into
//! `Uint8Array`s.

use super::ipc_params;
use crate::backend::Backend;
use crate::errors;
use docker::terminal::{OpenTerminalResult, TerminalTarget};
use docker::{CreateSandboxOptions, SandboxExecOptions, SandboxRemoveOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use std::future::Future;
use std::time::Duration;
use tauri::ipc::Request;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

// ---- handler logic (tested without Docker or a window) ----------------------------------------

/// `docker-sandbox-open-folder`: the directory from the sandbox status, passed to the opener
/// verbatim (no normalization, so Windows paths survive).
pub(crate) fn open_folder_result(
    directory: Option<&str>,
    exists: impl FnOnce(&str) -> bool,
    open: impl FnOnce(&str) -> Result<(), String>,
) -> Value {
    let Some(directory) = directory.filter(|d| !d.is_empty()) else {
        return json!({ "success": false, "error": "This chat has no sandbox folder yet." });
    };
    if !exists(directory) {
        return json!({
            "success": false,
            "error": format!("The sandbox folder no longer exists on disk: {directory}")
        });
    }
    match open(directory) {
        Ok(()) => json!({ "success": true, "path": directory }),
        Err(message) => json!({
            "success": false,
            "error": format!("Could not open {directory}: {message}")
        }),
    }
}

/// `Number.isInteger(port) && 1 <= port <= 65535`.
pub(crate) fn valid_port(port: &Value) -> Option<u16> {
    let n = port.as_f64()?;
    if n.fract() != 0.0 || !(1.0..=65535.0).contains(&n) {
        return None;
    }
    Some(n as u16)
}

/// `docker-sandbox-open-port`: only a port this sandbox publishes, only on localhost.
pub(crate) fn open_port_result(
    port: &Value,
    is_published: impl FnOnce(u16) -> bool,
    open_external: impl FnOnce(&str) -> Result<(), String>,
) -> Value {
    let Some(port) = valid_port(port) else {
        return json!({ "success": false, "error": "Not a valid port number." });
    };
    if !is_published(port) {
        return json!({
            "success": false,
            "error": format!("Port {port} is not published by this sandbox.")
        });
    }
    let url = format!("http://localhost:{port}");
    if let Err(e) = open_external(&url) {
        tracing::warn!(url = %url, error = %e, "Failed to open sandbox port");
    }
    json!({ "success": true, "url": url })
}

/// `docker-sandbox-terminal-open` arguments. There is deliberately no container name: the
/// container is resolved from the session id (and service) on this side, so the renderer can
/// never ask for a shell in an arbitrary container.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalOpenArgs {
    pub session_id: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub cols: Option<u16>,
    #[serde(default)]
    pub rows: Option<u16>,
}

/// Reuse the chat's shell for the service, else resolve the target and open one (80×24 by
/// default).
pub(crate) async fn terminal_open_with<R, RF, O, OF>(
    args: &TerminalOpenArgs,
    existing: Option<OpenTerminalResult>,
    resolve: R,
    open: O,
) -> Result<OpenTerminalResult, String>
where
    R: FnOnce(String, Option<String>) -> RF,
    RF: Future<Output = docker::Result<TerminalTarget>>,
    O: FnOnce(TerminalTarget, u16, u16) -> OF,
    OF: Future<Output = docker::Result<OpenTerminalResult>>,
{
    if let Some(existing) = existing {
        return Ok(existing);
    }
    let target = resolve(args.session_id.clone(), args.service.clone())
        .await
        .map_err(errors::plain)?;
    open(target, args.cols.unwrap_or(80), args.rows.unwrap_or(24))
        .await
        .map_err(errors::plain)
}

/// `check-docker-availability`: `docker --version` with a 5 s timeout.
async fn docker_version_probe() -> Value {
    let now = chrono_now_iso();
    let run = tokio::process::Command::new("docker")
        .arg("--version")
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(5), run).await {
        Err(_) => {
            json!({ "available": false, "error": "Docker check timed out", "lastChecked": now })
        }
        Ok(Err(e)) => json!({ "available": false, "error": e.to_string(), "lastChecked": now }),
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if out.status.success() && stdout.contains("Docker version") {
                json!({ "available": true, "version": docker_version(&stdout), "lastChecked": now })
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let error = if stderr.is_empty() {
                    "Docker not found or not running".to_string()
                } else {
                    stderr.into_owned()
                };
                json!({ "available": false, "error": error, "lastChecked": now })
            }
        }
    }
}

/// `/Docker version (\d+\.\d+\.\d+)/`, else `Unknown`.
fn docker_version(output: &str) -> String {
    output
        .split("Docker version ")
        .nth(1)
        .and_then(|rest| {
            let v: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let parts: Vec<&str> = v.split('.').collect();
            (parts.len() >= 3 && parts[..3].iter().all(|p| !p.is_empty()))
                .then(|| parts[..3].join("."))
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

/// `new Date()` as the ISO string it serializes to; the bridge turns it back into a `Date`.
fn chrono_now_iso() -> String {
    bedrock::sdk::now_iso()
}

// ---- commands -------------------------------------------------------------------------------

#[tauri::command]
pub async fn check_docker_availability() -> Result<Value, String> {
    Ok(docker_version_probe().await)
}

#[tauri::command]
pub async fn docker_sandbox_availability(
    backend: State<'_, Backend>,
    force: Option<bool>,
) -> Result<docker::DockerAvailability, String> {
    Ok(backend
        .sandbox
        .get_availability(force.unwrap_or(false))
        .await)
}

#[tauri::command]
pub async fn docker_sandbox_create(
    backend: State<'_, Backend>,
    session_id: String,
    options: Option<CreateSandboxOptions>,
) -> Result<Value, String> {
    let result = backend
        .sandbox
        .create_sandbox(&session_id, options.unwrap_or_default())
        .await
        .map_err(errors::plain)?;
    serde_json::to_value(result).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_status(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<docker::SandboxStatus, String> {
    Ok(backend.sandbox.get_status(&session_id).await)
}

#[tauri::command]
pub async fn docker_sandbox_start(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<docker::SandboxStatus, String> {
    backend
        .sandbox
        .start_sandbox(&session_id)
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_stop(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<docker::SandboxStatus, String> {
    backend
        .sandbox
        .stop_sandbox(&session_id)
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_remove(
    backend: State<'_, Backend>,
    session_id: String,
    options: Option<SandboxRemoveOptions>,
) -> Result<Value, String> {
    let result = backend
        .sandbox
        .remove_sandbox(&session_id, options.unwrap_or_default())
        .await
        .map_err(errors::plain)?;
    serde_json::to_value(result).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_rename(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<Value, String> {
    serde_json::to_value(backend.sandbox.rename_sandbox(&session_id).await).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_logs(
    backend: State<'_, Backend>,
    session_id: String,
    service: Option<String>,
    tail: Option<u32>,
) -> Result<Value, String> {
    let logs = backend
        .sandbox
        .get_logs(&session_id, service.as_deref(), tail)
        .await
        .map_err(errors::plain)?;
    serde_json::to_value(logs).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_exec(
    backend: State<'_, Backend>,
    session_id: String,
    command: String,
    options: Option<SandboxExecOptions>,
) -> Result<docker::SandboxExecResult, String> {
    backend
        .sandbox
        .exec_command(&session_id, &command, options.unwrap_or_default())
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_send_input(
    backend: State<'_, Backend>,
    pid: u32,
    stdin: String,
) -> Result<docker::SandboxExecResult, String> {
    backend
        .sandbox
        .send_input(pid, &stdin)
        .await
        .map_err(errors::plain)
}

#[tauri::command]
pub fn docker_sandbox_has_pid(backend: State<'_, Backend>, pid: u32) -> Value {
    json!({ "tracked": backend.sandbox.is_tracked_sandbox_pid(pid) })
}

#[tauri::command]
pub fn docker_sandbox_list(backend: State<'_, Backend>) -> Value {
    json!({ "sessionIds": backend.sandbox.list_sandbox_session_ids() })
}

#[tauri::command]
pub async fn docker_sandbox_open_folder(
    app: AppHandle,
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<Value, String> {
    let status = backend.sandbox.get_status(&session_id).await;
    let directory = status.metadata.as_ref().map(|m| m.directory.as_str());
    Ok(open_folder_result(
        directory,
        |d| std::path::Path::new(d).exists(),
        |d| {
            app.opener()
                .open_path(d, None::<&str>)
                .map_err(|e| e.to_string())
        },
    ))
}

#[tauri::command]
pub fn docker_sandbox_open_port(
    app: AppHandle,
    backend: State<'_, Backend>,
    session_id: String,
    port: Value,
) -> Value {
    open_port_result(
        &port,
        |p| backend.sandbox.is_published_port(&session_id, p),
        |url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        },
    )
}

#[tauri::command]
pub async fn docker_sandbox_insights(
    backend: State<'_, Backend>,
    session_id: String,
    service: Option<String>,
) -> Result<Value, String> {
    let insights = backend
        .sandbox
        .get_insights(&session_id, service.as_deref())
        .await
        .map_err(errors::plain)?;
    serde_json::to_value(insights).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_compose(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<Value, String> {
    let file = backend
        .sandbox
        .get_compose_file(&session_id)
        .await
        .map_err(errors::plain)?;
    serde_json::to_value(file).map_err(errors::plain)
}

#[tauri::command]
pub async fn docker_sandbox_activity(
    backend: State<'_, Backend>,
    session_id: String,
) -> Result<Value, String> {
    let entries = backend.sandbox.activity().get_activity(&session_id).await;
    Ok(json!({ "entries": entries }))
}

#[tauri::command]
pub async fn docker_sandbox_terminal_capability(
    backend: State<'_, Backend>,
) -> Result<docker::terminal::TerminalCapability, String> {
    Ok(docker::terminal::get_terminal_capability(backend.sandbox.engine()).await)
}

/// `{ sessionId, service?, cols?, rows? }`; any other key (e.g. a container name) is ignored.
#[tauri::command]
pub async fn docker_sandbox_terminal_open(
    backend: State<'_, Backend>,
    request: Request<'_>,
) -> Result<OpenTerminalResult, String> {
    let args: TerminalOpenArgs =
        serde_json::from_value(ipc_params(&request)).map_err(errors::plain)?;
    let sandbox = &backend.sandbox;
    let existing = sandbox
        .terminals()
        .find_session_terminal(&args.session_id, args.service.as_deref());
    terminal_open_with(
        &args,
        existing,
        |session_id, service| async move {
            sandbox
                .resolve_terminal_target(&session_id, service.as_deref())
                .await
        },
        |target, cols, rows| async move {
            sandbox.terminals().open_terminal(&target, cols, rows).await
        },
    )
    .await
}

/// Start publishing; returns the scrollback so far as `{ backlog: number[] }`.
#[tauri::command]
pub fn docker_sandbox_terminal_attach(
    backend: State<'_, Backend>,
    terminal_id: String,
) -> Result<Value, String> {
    let backlog = backend
        .sandbox
        .terminals()
        .attach_terminal(&terminal_id)
        .map_err(errors::plain)?;
    Ok(json!({ "backlog": backlog }))
}

#[tauri::command]
pub fn docker_sandbox_terminal_input(
    backend: State<'_, Backend>,
    terminal_id: String,
    data: String,
) -> Value {
    backend
        .sandbox
        .terminals()
        .write_to_terminal(&terminal_id, &data);
    json!({ "success": true })
}

#[tauri::command]
pub async fn docker_sandbox_terminal_resize(
    backend: State<'_, Backend>,
    terminal_id: String,
    cols: u16,
    rows: u16,
) -> Result<Value, String> {
    backend
        .sandbox
        .terminals()
        .resize_terminal(&terminal_id, cols, rows)
        .await
        .map_err(errors::plain)?;
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub fn docker_sandbox_terminal_backlog(backend: State<'_, Backend>, terminal_id: String) -> Value {
    json!({ "backlog": backend.sandbox.terminals().get_terminal_backlog(&terminal_id) })
}

#[tauri::command]
pub fn docker_sandbox_terminal_close(backend: State<'_, Backend>, terminal_id: String) -> Value {
    backend.sandbox.terminals().close_terminal(&terminal_id);
    json!({ "success": true })
}

#[cfg(test)]
mod tests {
    //! Port of `src/main/handlers/docker-sandbox-handlers.test.ts`.
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    // ---- docker-sandbox-open-folder ----

    #[test]
    fn opens_the_sandbox_directory_and_reports_the_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_string_lossy().into_owned();
        let opened = RefCell::new(None);
        let result = open_folder_result(
            Some(&dir),
            |d| std::path::Path::new(d).exists(),
            |d| {
                *opened.borrow_mut() = Some(d.to_string());
                Ok(())
            },
        );
        assert_eq!(opened.into_inner().as_deref(), Some(dir.as_str()));
        assert_eq!(result, json!({ "success": true, "path": dir }));
    }

    #[test]
    fn fails_clearly_when_the_chat_has_no_sandbox() {
        let called = RefCell::new(false);
        let result = open_folder_result(
            None,
            |_| true,
            |_| {
                *called.borrow_mut() = true;
                Ok(())
            },
        );
        assert!(!called.into_inner());
        assert_eq!(result["success"], false);
        assert!(result["error"]
            .as_str()
            .unwrap()
            .contains("no sandbox folder yet"));
    }

    #[test]
    fn fails_clearly_when_the_folder_was_deleted_outside_the_app() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("gone").to_string_lossy().into_owned();
        let called = RefCell::new(false);
        let result = open_folder_result(
            Some(&missing),
            |d| std::path::Path::new(d).exists(),
            |_| {
                *called.borrow_mut() = true;
                Ok(())
            },
        );
        assert!(!called.into_inner());
        assert_eq!(result["success"], false);
        assert!(result["error"].as_str().unwrap().contains(&missing));
    }

    #[test]
    fn surfaces_the_platform_error_when_the_os_cannot_open_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_string_lossy().into_owned();
        let result =
            open_folder_result(Some(&dir), |_| true, |_| Err("Failed to open path".into()));
        assert_eq!(result["success"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("Failed to open path"));
        assert!(error.contains(&dir));
    }

    #[test]
    fn passes_the_directory_through_verbatim() {
        let windows_path = r"C:\Users\dev\project\docker-sandboxes\fix-auth-a3f21c";
        let opened = RefCell::new(None);
        let result = open_folder_result(
            Some(windows_path),
            |_| true,
            |d| {
                *opened.borrow_mut() = Some(d.to_string());
                Ok(())
            },
        );
        assert_eq!(opened.into_inner().as_deref(), Some(windows_path));
        assert_eq!(result, json!({ "success": true, "path": windows_path }));
    }

    // ---- docker-sandbox-open-port ----

    #[test]
    fn opens_a_port_the_sandbox_publishes() {
        let opened = RefCell::new(None);
        let result = open_port_result(
            &json!(3000),
            |p| p == 3000,
            |u| {
                *opened.borrow_mut() = Some(u.to_string());
                Ok(())
            },
        );
        assert_eq!(
            opened.into_inner().as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            result,
            json!({ "success": true, "url": "http://localhost:3000" })
        );
    }

    #[test]
    fn refuses_a_port_this_sandbox_does_not_publish() {
        let opened = RefCell::new(false);
        let result = open_port_result(
            &json!(8080),
            |_| false,
            |_| {
                *opened.borrow_mut() = true;
                Ok(())
            },
        );
        assert!(!opened.into_inner());
        assert_eq!(result["success"], false);
        assert!(result["error"].as_str().unwrap().contains("not published"));
    }

    #[test]
    fn refuses_invalid_port_numbers() {
        // NaN arrives as `null` over JSON.
        for port in [json!(0), json!(-1), json!(70000), json!(1.5), Value::Null] {
            let checked = RefCell::new(false);
            let opened = RefCell::new(false);
            let result = open_port_result(
                &port,
                |_| {
                    *checked.borrow_mut() = true;
                    true
                },
                |_| {
                    *opened.borrow_mut() = true;
                    Ok(())
                },
            );
            assert!(!opened.into_inner(), "{port}");
            assert!(!checked.into_inner(), "{port}");
            assert_eq!(result["success"], false, "{port}");
        }
    }

    // ---- docker-sandbox-terminal-open ----

    fn target() -> TerminalTarget {
        TerminalTarget {
            session_id: "session_1".into(),
            service: "main".into(),
            container_name: "bedrock-sandbox-session-1-main-1".into(),
        }
    }

    fn opened(id: &str) -> OpenTerminalResult {
        OpenTerminalResult {
            terminal_id: id.into(),
            channel: format!("docker-sandbox:terminal:{id}"),
            service: "main".into(),
            cols: 80,
            rows: 24,
        }
    }

    type Calls = Mutex<Vec<String>>;

    async fn run_open(
        args: Value,
        existing: Option<OpenTerminalResult>,
        resolved: docker::Result<TerminalTarget>,
        calls: &Calls,
    ) -> Result<OpenTerminalResult, String> {
        let args: TerminalOpenArgs = serde_json::from_value(args).unwrap();
        terminal_open_with(
            &args,
            existing,
            |session_id, service| {
                calls
                    .lock()
                    .unwrap()
                    .push(format!("resolve {session_id} {service:?}"));
                async move { resolved }
            },
            |target, cols, rows| {
                calls.lock().unwrap().push(format!(
                    "open {} {cols} {rows}",
                    serde_json::to_string(&target).unwrap()
                ));
                async move { Ok(opened("t1")) }
            },
        )
        .await
    }

    #[tokio::test]
    async fn resolves_the_container_from_the_session_id_alone() {
        let calls = Calls::default();
        run_open(
            json!({ "sessionId": "session_1", "cols": 100, "rows": 30 }),
            None,
            Ok(target()),
            &calls,
        )
        .await
        .unwrap();
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls[0], "resolve session_1 None");
        assert!(calls[1].contains("bedrock-sandbox-session-1-main-1"));
        assert!(calls[1].ends_with(" 100 30"));
    }

    #[tokio::test]
    async fn ignores_any_container_name_the_renderer_tries_to_supply() {
        let calls = Calls::default();
        run_open(
            json!({ "sessionId": "session_1", "containerName": "someone-elses-postgres" }),
            None,
            Ok(target()),
            &calls,
        )
        .await
        .unwrap();
        let calls = calls.into_inner().unwrap();
        assert!(calls[1].contains("bedrock-sandbox-session-1-main-1"));
        assert!(calls[1].ends_with(" 80 24"));
        assert!(!calls.join("\n").contains("someone-elses-postgres"));
    }

    #[tokio::test]
    async fn propagates_the_rejection_for_a_service_not_in_the_sandbox() {
        let calls = Calls::default();
        let err = run_open(
            json!({ "sessionId": "session_1", "service": "db" }),
            None,
            Err(docker::Error::Message(
                "Service \"db\" is not part of this sandbox.".into(),
            )),
            &calls,
        )
        .await
        .unwrap_err();
        assert!(err.contains("not part of this sandbox"));
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls, vec!["resolve session_1 Some(\"db\")".to_string()]);
    }

    #[tokio::test]
    async fn reuses_the_chats_existing_shell() {
        let calls = Calls::default();
        let result = run_open(
            json!({ "sessionId": "session_1" }),
            Some(opened("existing")),
            Ok(target()),
            &calls,
        )
        .await
        .unwrap();
        assert_eq!(result.terminal_id, "existing");
        assert!(calls.into_inner().unwrap().is_empty());
    }

    #[test]
    fn docker_version_parsing() {
        assert_eq!(
            docker_version("Docker version 27.3.1, build ce12230\n"),
            "27.3.1"
        );
        assert_eq!(docker_version("Docker version x"), "Unknown");
    }
}
