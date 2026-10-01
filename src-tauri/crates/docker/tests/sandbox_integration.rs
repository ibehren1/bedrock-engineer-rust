//! Port of `src/main/api/docker/sandbox.integration.test.ts`. Requires a working Docker
//! installation; every test is `#[ignore]` and also returns early when Docker is down.
//!
//! Run with: `cargo test -p docker --test sandbox_integration -- --ignored --test-threads=1`

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use docker::availability::check_docker_availability;
use docker::events::TerminalMessage;
use docker::naming::{to_folder_name, to_project_name};
use docker::{
    CreateSandboxOptions, RecordingSink, SandboxEvent, SandboxExecOptions, SandboxManager,
    SandboxPortMapping, SandboxRemoveOptions, SandboxRunState, SandboxServiceSpec, Settings,
    TokioRunner, DEFAULT_SANDBOX_IMAGE, WORKSPACE_MOUNT,
};
use serde_json::{json, Value};

struct Env {
    _project: tempfile::TempDir,
    _user_data: tempfile::TempDir,
    project: PathBuf,
    user_data: PathBuf,
    sink: Arc<RecordingSink>,
    manager: SandboxManager,
}

impl Env {
    fn set_chat_title(&self, session_id: &str, title: &str) {
        let dir = self.user_data.join("chat-sessions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{session_id}.json")),
            json!({ "id": session_id, "title": title, "messages": [] }).to_string(),
        )
        .unwrap();
    }

    async fn cleanup(&self, session_ids: &[&str]) {
        for id in session_ids {
            let _ = self
                .manager
                .remove_sandbox(
                    id,
                    SandboxRemoveOptions {
                        delete_data: Some(true),
                    },
                )
                .await;
        }
    }

    fn terminal_text(&self, channel: &str) -> String {
        let bytes: Vec<u8> = self
            .sink
            .on_channel(channel)
            .into_iter()
            .filter_map(|event| match event {
                SandboxEvent::Terminal(TerminalMessage::Data { bytes, .. }) => Some(bytes),
                _ => None,
            })
            .flatten()
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn terminal_exit(&self, channel: &str) -> Option<Option<i32>> {
        self.sink
            .on_channel(channel)
            .into_iter()
            .find_map(|event| match event {
                SandboxEvent::Terminal(TerminalMessage::Exit { exit_code }) => Some(exit_code),
                _ => None,
            })
    }
}

/// `None` when Docker is unavailable, so the test can skip.
async fn env() -> Option<Env> {
    let availability = check_docker_availability(&TokioRunner).await;
    if !(availability.docker_installed && availability.daemon_running) {
        eprintln!(
            "Skipping sandbox integration tests: {:?}",
            availability.error
        );
        return None;
    }
    let project_dir = tempfile::tempdir().unwrap();
    let user_data_dir = tempfile::tempdir().unwrap();
    let project = project_dir.path().canonicalize().unwrap();
    let user_data = user_data_dir.path().canonicalize().unwrap();
    let settings_project = project.to_string_lossy().into_owned();
    let settings_user_data = user_data.to_string_lossy().into_owned();
    let settings: Arc<dyn Settings> = Arc::new(move |key: &str| match key {
        "projectPath" => Some(Value::String(settings_project.clone())),
        "userDataPath" => Some(Value::String(settings_user_data.clone())),
        "dockerSandboxTool" => {
            Some(json!({ "memoryLimit": "512m", "cpuLimit": 1.0, "timeout": 120 }))
        }
        _ => None,
    });
    let sink = Arc::new(RecordingSink::default());
    let manager = SandboxManager::new(settings, sink.clone());
    Some(Env {
        _project: project_dir,
        _user_data: user_data_dir,
        project,
        user_data,
        sink,
        manager,
    })
}

fn session(suffix: &str) -> String {
    format!("session_{}_{suffix}", chrono_ms())
}

fn chrono_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn opts(timeout: f64) -> SandboxExecOptions {
    SandboxExecOptions {
        timeout: Some(timeout),
        ..Default::default()
    }
}

async fn wait_for(mut predicate: impl FnMut() -> bool, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    panic!("Timed out waiting for terminal output");
}

fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(3),
    )
    .is_ok()
}

fn dir_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

// describe('sandbox lifecycle')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn sandbox_lifecycle() {
    let Some(env) = env().await else { return };
    let id = session("life");
    let m = &env.manager;

    // creates a sandbox on first exec and reports it as running
    let result = m
        .exec_command(&id, "echo hello-from-sandbox", Default::default())
        .await
        .unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello-from-sandbox"));
    let status = m.get_status(&id).await;
    assert!(status.exists);
    assert_eq!(status.state, SandboxRunState::Running);
    let metadata = status.metadata.unwrap();
    assert_eq!(metadata.project_name, to_project_name(&id));
    assert_eq!(metadata.services[0].image, DEFAULT_SANDBOX_IMAGE);

    // writes compose, env, and gitignore files under the project directory
    let dir = m.get_sandbox_dir(&id).unwrap();
    assert!(dir.join("docker-compose.yml").exists());
    assert!(dir.join(".env").exists());
    assert!(dir.join("sandbox.json").exists());
    let gitignore = m.get_sandbox_root(None).unwrap().join(".gitignore");
    assert!(std::fs::read_to_string(gitignore).unwrap().contains('*'));

    // mounts the project directory at /workspace, read-write both ways
    std::fs::write(env.project.join("from-host.txt"), "host-content").unwrap();
    let read = m
        .exec_command(
            &id,
            &format!("cat {WORKSPACE_MOUNT}/from-host.txt"),
            Default::default(),
        )
        .await
        .unwrap();
    assert!(read.stdout.contains("host-content"));
    m.exec_command(
        &id,
        &format!("echo container-content > {WORKSPACE_MOUNT}/from-container.txt"),
        Default::default(),
    )
    .await
    .unwrap();
    assert!(
        std::fs::read_to_string(env.project.join("from-container.txt"))
            .unwrap()
            .contains("container-content")
    );

    // persists files written to /data on the host under the sandbox data folder
    m.exec_command(&id, "echo persisted > /data/note.txt", Default::default())
        .await
        .unwrap();
    let note = m
        .get_sandbox_dir(&id)
        .unwrap()
        .join("data")
        .join("main")
        .join("note.txt");
    assert!(std::fs::read_to_string(note).unwrap().contains("persisted"));

    // has network access, so apt can install packages
    let apt = m
        .exec_command(
            &id,
            "apt-get update -qq && apt-get install -y -qq curl && curl --version",
            opts(480.0),
        )
        .await
        .unwrap();
    assert_eq!(apt.exit_code, 0);
    assert!(apt.stdout.contains("curl"));

    // keeps installed packages across a stop and start
    m.stop_sandbox(&id).await.unwrap();
    assert_eq!(m.get_status(&id).await.state, SandboxRunState::Stopped);
    m.start_sandbox(&id).await.unwrap();
    assert_eq!(m.get_status(&id).await.state, SandboxRunState::Running);
    assert_eq!(
        m.exec_command(&id, "command -v curl", Default::default())
            .await
            .unwrap()
            .exit_code,
        0
    );

    // answers an interactive prompt through a stdin follow-up
    let first = m
        .exec_command(
            &id,
            "unset DEBIAN_FRONTEND; apt-get install less",
            opts(240.0),
        )
        .await
        .unwrap();
    if first.requires_input == Some(true) {
        let pid = first.process_info.as_ref().unwrap().pid;
        assert!(pid > 0);
        assert!(first.prompt.as_deref().is_some_and(|p| !p.is_empty()));
        m.send_input(pid, "Y").await.unwrap();
    }

    // rejects a stdin follow-up for an unknown PID with an actionable message
    let error = m.send_input(999_999, "Y").await.unwrap_err();
    assert!(error
        .to_string()
        .contains("No running sandbox command found"));

    // starts a detached process and exposes its output through logs
    let detached = m
        .exec_command(
            &id,
            "sh -c \"echo detached-marker >> /proc/1/fd/1\"",
            SandboxExecOptions {
                detach: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(detached.exit_code, 0);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let logs = m.get_logs(&id, None, Some(50)).await.unwrap();
    assert!(format!("{}{}", logs.stdout, logs.stderr).contains("detached-marker"));

    // reports a helpful error for an unknown service
    let error = m
        .exec_command(
            &id,
            "true",
            SandboxExecOptions {
                service: Some("nope".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("is not part of this sandbox"));

    env.cleanup(&[&id]).await;
}

// describe('published ports')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn published_ports_are_reachable_from_the_host() {
    let Some(env) = env().await else { return };
    let id = session("ports");
    let host_port = 38_411;
    let m = &env.manager;

    m.create_sandbox(
        &id,
        CreateSandboxOptions {
            services: Some(vec![SandboxServiceSpec {
                ports: Some(vec![SandboxPortMapping {
                    host: host_port,
                    container: 8000,
                }]),
                ..SandboxServiceSpec::named("main")
            }]),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    m.exec_command(
        &id,
        "apt-get update -qq && apt-get install -y -qq netcat-openbsd",
        opts(480.0),
    )
    .await
    .unwrap();
    m.exec_command(
        &id,
        "nohup sh -c 'while true; do printf \"HTTP/1.1 200 OK\\r\\nContent-Length: 2\\r\\n\\r\\nok\" | nc -l -p 8000 -q 1; done' >/dev/null 2>&1 &",
        SandboxExecOptions {
            detach: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(is_port_open(host_port));

    env.cleanup(&[&id]).await;
}

// describe('human-readable folder naming')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn human_readable_folder_naming() {
    let Some(env) = env().await else { return };
    let id = session("named");
    let m = &env.manager;

    // names the folder from the chat title at creation
    env.set_chat_title(&id, "Fix the auth bug");
    m.create_sandbox(&id, Default::default()).await.unwrap();
    let dir = m.get_sandbox_dir(&id).unwrap();
    assert_eq!(
        dir_name(&dir),
        to_folder_name(&id, Some("Fix the auth bug"))
    );
    assert!(dir_name(&dir).starts_with("fix-the-auth-bug-"));

    // renames the folder when the chat title changes, keeping the container
    m.exec_command(&id, "touch /marker-before-rename", Default::default())
        .await
        .unwrap();
    let before = m.get_sandbox_dir(&id).unwrap();
    let before_containers: Vec<String> = m
        .get_status(&id)
        .await
        .containers
        .into_iter()
        .map(|c| c.name)
        .collect();
    env.set_chat_title(&id, "Rework the login flow");
    assert!(m.rename_sandbox(&id).await.renamed);
    let after = m.get_sandbox_dir(&id).unwrap();
    assert!(dir_name(&after).starts_with("rework-the-login-flow-"));
    assert!(!before.exists());
    assert!(after.join("docker-compose.yml").exists());
    let status = m.get_status(&id).await;
    assert_eq!(status.state, SandboxRunState::Running);
    let names: Vec<String> = status.containers.into_iter().map(|c| c.name).collect();
    assert_eq!(names, before_containers);
    assert_eq!(
        m.exec_command(&id, "test -f /marker-before-rename", Default::default())
            .await
            .unwrap()
            .exit_code,
        0
    );

    // still resolves the sandbox by session id after the folder moved
    assert!(m.list_sandbox_session_ids().contains(&id));
    let status = m.get_status(&id).await;
    assert!(status.exists);
    let metadata = status.metadata.unwrap();
    assert_eq!(metadata.session_id, id);
    assert_eq!(
        PathBuf::from(metadata.directory),
        m.get_sandbox_dir(&id).unwrap()
    );

    // data written before the rename moves with the folder
    m.exec_command(
        &id,
        "echo survived > /data/rename-check.txt",
        Default::default(),
    )
    .await
    .unwrap();
    let check = m
        .get_sandbox_dir(&id)
        .unwrap()
        .join("data/main/rename-check.txt");
    assert!(std::fs::read_to_string(check).unwrap().contains("survived"));

    // is a no-op when the title has not changed
    assert!(!m.rename_sandbox(&id).await.renamed);

    // refuses to rename onto a folder another chat already owns
    let target = m
        .get_sandbox_root(None)
        .unwrap()
        .join(to_folder_name(&id, Some("Something else entirely")));
    std::fs::create_dir_all(&target).unwrap();
    let before = m.get_sandbox_dir(&id).unwrap();
    env.set_chat_title(&id, "Something else entirely");
    assert!(!m.rename_sandbox(&id).await.renamed);
    assert_eq!(m.get_sandbox_dir(&id).unwrap(), before);
    std::fs::remove_dir_all(&target).unwrap();

    env.cleanup(&[&id]).await;
}

// describe('teardown')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn teardown() {
    let Some(env) = env().await else { return };
    let m = &env.manager;

    // removing without deleteData keeps the folder but drops the sandbox
    let keep = session("keep");
    m.create_sandbox(&keep, Default::default()).await.unwrap();
    let dir = m.get_sandbox_dir(&keep).unwrap();
    let result = m
        .remove_sandbox(
            &keep,
            SandboxRemoveOptions {
                delete_data: Some(false),
            },
        )
        .await
        .unwrap();
    assert!(result.removed && !result.data_deleted);
    assert!(dir.join("docker-compose.yml").exists());
    assert!(!dir.join("sandbox.json").exists());
    assert!(!m.get_status(&keep).await.exists);
    std::fs::remove_dir_all(&dir).unwrap();

    // removing with deleteData deletes the whole folder
    let wipe = session("wipe");
    m.create_sandbox(&wipe, Default::default()).await.unwrap();
    let dir = m.get_sandbox_dir(&wipe).unwrap();
    assert!(
        m.remove_sandbox(
            &wipe,
            SandboxRemoveOptions {
                delete_data: Some(true)
            }
        )
        .await
        .unwrap()
        .data_deleted
    );
    assert!(!dir.exists());

    // lists only sessions that still have a sandbox
    let listed = session("listed");
    m.create_sandbox(&listed, Default::default()).await.unwrap();
    assert!(m.list_sandbox_session_ids().contains(&listed));
    m.remove_sandbox(
        &listed,
        SandboxRemoveOptions {
            delete_data: Some(true),
        },
    )
    .await
    .unwrap();
    assert!(!m.list_sandbox_session_ids().contains(&listed));
}

// describe('interactive terminal')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn interactive_terminal() {
    let Some(env) = env().await else { return };
    let id = session("term");
    let m = &env.manager;
    let terminals = m.terminals();

    // delivers a shell prompt without any input being sent
    m.create_sandbox(&id, Default::default()).await.unwrap();
    let target = m.resolve_terminal_target(&id, None).await.unwrap();
    let opened = terminals.open_terminal(&target, 80, 24).await.unwrap();
    let backlog = String::from_utf8_lossy(&terminals.attach_terminal(&opened.terminal_id).unwrap())
        .into_owned();
    wait_for(
        || format!("{backlog}{}", env.terminal_text(&opened.channel)).contains('#'),
        Duration::from_secs(10),
    )
    .await;
    let seen = format!("{backlog}{}", env.terminal_text(&opened.channel));
    let prompt = regex_like_root_prompt(&seen);
    assert!(prompt, "no root prompt in {seen:?}");
    terminals.close_session_terminals(&id);

    // refuses to open a shell when the container is not running
    let target = m.resolve_terminal_target(&id, None).await.unwrap();
    m.stop_sandbox(&id).await.unwrap();
    let error = terminals
        .open_terminal(&target, 80, 24)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("not running") || error.contains("exited"),
        "{error}"
    );
    assert!(terminals.find_session_terminal(&id, None).is_none());
    m.start_sandbox(&id).await.unwrap();

    // runs a command, honours resize, and handles Ctrl-C without killing the shell
    let target = m.resolve_terminal_target(&id, None).await.unwrap();
    let opened = terminals.open_terminal(&target, 80, 24).await.unwrap();
    terminals.attach_terminal(&opened.terminal_id).unwrap();
    terminals.write_to_terminal(&opened.terminal_id, "echo terminal-works\n");
    wait_for(
        || {
            env.terminal_text(&opened.channel)
                .contains("terminal-works")
        },
        Duration::from_secs(10),
    )
    .await;
    terminals
        .resize_terminal(&opened.terminal_id, 40, 10)
        .await
        .unwrap();
    terminals.write_to_terminal(&opened.terminal_id, "stty size\n");
    wait_for(
        || env.terminal_text(&opened.channel).contains("10 40"),
        Duration::from_secs(10),
    )
    .await;
    terminals.write_to_terminal(&opened.terminal_id, "sleep 30\n");
    tokio::time::sleep(Duration::from_millis(400)).await;
    terminals.write_to_terminal(&opened.terminal_id, "\x03");
    terminals.write_to_terminal(&opened.terminal_id, "echo still-alive\n");
    wait_for(
        || env.terminal_text(&opened.channel).contains("still-alive"),
        Duration::from_secs(10),
    )
    .await;
    assert!(env.terminal_exit(&opened.channel).is_none());

    // reports an exit when the shell ends
    terminals.write_to_terminal(&opened.terminal_id, "exit\n");
    wait_for(
        || env.terminal_exit(&opened.channel).is_some(),
        Duration::from_secs(15),
    )
    .await;
    assert_eq!(env.terminal_exit(&opened.channel), Some(Some(0)));

    // closes the terminal when the sandbox is stopped
    let target = m.resolve_terminal_target(&id, None).await.unwrap();
    let opened = terminals.open_terminal(&target, 80, 24).await.unwrap();
    assert_eq!(
        terminals
            .find_session_terminal(&id, None)
            .map(|t| t.terminal_id),
        Some(opened.terminal_id)
    );
    m.stop_sandbox(&id).await.unwrap();
    assert!(terminals.find_session_terminal(&id, None).is_none());

    // refuses a service that is not part of the sandbox
    let error = m
        .resolve_terminal_target(&id, Some("not-a-service"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not part of this sandbox"));

    env.cleanup(&[&id]).await;
}

/// `/root@[0-9a-f]+:\/workspace#/` without pulling a regex dev-dependency.
fn regex_like_root_prompt(text: &str) -> bool {
    text.match_indices("root@").any(|(at, _)| {
        let rest = &text[at + 5..];
        let hex_len = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        hex_len > 0 && rest[hex_len..].starts_with(":/workspace#")
    })
}

// describe('insights and activity')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn insights_and_activity() {
    let Some(env) = env().await else { return };
    let id = session("insights");
    let m = &env.manager;

    // reports the image, uptime and resource use of a running container
    m.exec_command(&id, "echo warm", Default::default())
        .await
        .unwrap();
    let first = m.get_insights(&id, None).await.unwrap();
    assert_eq!(first.image.as_deref(), Some(DEFAULT_SANDBOX_IMAGE));
    assert_eq!(first.status.as_deref(), Some("running"));
    assert!(first.started_at.as_deref().is_some_and(|s| !s.is_empty()));
    assert!(first.memory_limit.unwrap_or(0) > 0);
    assert!(
        first.cpu_percent.is_none(),
        "a CPU percentage needs two samples"
    );
    let second = m.get_insights(&id, None).await.unwrap();
    assert!(second.cpu_percent.unwrap_or(-1.0) >= 0.0);

    // reports network counters, and disk counters where the host provides them
    m.exec_command(
        &id,
        "dd if=/dev/zero of=/tmp/probe.bin bs=1M count=20 2>/dev/null; sync; cat /tmp/probe.bin > /dev/null",
        Default::default(),
    )
    .await
    .unwrap();
    m.get_insights(&id, None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let insights = m.get_insights(&id, None).await.unwrap();
    assert!(insights.net_rx.is_some() && insights.net_tx.is_some());
    assert!(insights.net_rx_per_second.unwrap_or(-1.0) >= 0.0);
    match insights.block_write {
        None => assert!(insights.block_read.is_none()),
        Some(written) => {
            assert!(written > 0);
            assert!(insights.block_write_per_second.unwrap_or(-1.0) >= 0.0);
        }
    }

    // records each command as start, settled and exit, and persists it
    m.exec_command(&id, "echo recorded-command", Default::default())
        .await
        .unwrap();
    m.activity().flush().await;
    let entries = m.activity().get_activity(&id).await;
    let entry = entries
        .iter()
        .find(|e| e.command == "echo recorded-command")
        .unwrap();
    assert_eq!(serde_json::to_value(entry.outcome).unwrap(), "completed");
    assert_eq!(entry.exit_code, Some(0));
    assert!(entry.ended_at.is_some());
    assert_eq!(serde_json::to_value(entry.source).unwrap(), "agent");
    let file = m
        .get_sandbox_dir(&id)
        .unwrap()
        .join(docker::activity::ACTIVITY_FILENAME);
    assert!(std::fs::read_to_string(file)
        .unwrap()
        .contains("echo recorded-command"));

    env.cleanup(&[&id]).await;
}

// describe('multi-service compose stacks')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn multi_service_compose_stacks() {
    let Some(env) = env().await else { return };
    let id = session("stack");
    let m = &env.manager;
    let terminals = m.terminals();

    m.create_sandbox(
        &id,
        CreateSandboxOptions {
            services: Some(vec![
                SandboxServiceSpec {
                    image: Some(DEFAULT_SANDBOX_IMAGE.into()),
                    ..SandboxServiceSpec::named("main")
                },
                SandboxServiceSpec {
                    image: Some(DEFAULT_SANDBOX_IMAGE.into()),
                    ..SandboxServiceSpec::named("sidecar")
                },
            ]),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let compose = m.get_compose_file(&id).await.unwrap();
    assert!(!compose.composeless);
    let contents = compose.contents.unwrap();
    assert!(contents.contains("main:") && contents.contains("sidecar:"));
    assert!(compose.path.unwrap().ends_with("docker-compose.yml"));

    let main_target = m.resolve_terminal_target(&id, Some("main")).await.unwrap();
    let sidecar_target = m
        .resolve_terminal_target(&id, Some("sidecar"))
        .await
        .unwrap();
    assert_ne!(main_target.container_name, sidecar_target.container_name);

    let main = terminals.open_terminal(&main_target, 80, 24).await.unwrap();
    let sidecar = terminals
        .open_terminal(&sidecar_target, 80, 24)
        .await
        .unwrap();
    assert_ne!(sidecar.terminal_id, main.terminal_id);
    terminals.attach_terminal(&main.terminal_id).unwrap();
    terminals.attach_terminal(&sidecar.terminal_id).unwrap();

    terminals.write_to_terminal(&main.terminal_id, "hostname\n");
    terminals.write_to_terminal(&sidecar.terminal_id, "hostname\n");
    wait_for(
        || {
            !env.terminal_text(&main.channel).is_empty()
                && !env.terminal_text(&sidecar.channel).is_empty()
        },
        Duration::from_secs(15),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    let host_of = |text: String| -> Option<String> {
        text.split(|c: char| !c.is_ascii_hexdigit())
            .find(|word| word.len() == 12)
            .map(str::to_string)
    };
    let main_host = host_of(env.terminal_text(&main.channel));
    let sidecar_host = host_of(env.terminal_text(&sidecar.channel));
    assert!(main_host.is_some() && sidecar_host.is_some());
    assert_ne!(main_host, sidecar_host);

    terminals.close_session_terminals(&id);
    env.cleanup(&[&id]).await;
}
