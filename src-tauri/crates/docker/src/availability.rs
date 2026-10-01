//! Probe the local Docker installation. Port of `src/main/api/docker/dockerAvailability.ts`.

use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;

use crate::runner::{CommandRunner, RunOptions};
use crate::types::{ComposeFlavor, DockerAvailability};
use crate::util::now_iso;
use crate::{Error, Result};

const PROBE_TIMEOUT: Duration = Duration::from_millis(8000);

struct ProbeResult {
    ok: bool,
    stdout: String,
    stderr: String,
}

/// Run a short-lived probe. Never fails — a missing binary and a non-zero exit both come
/// back as `ok: false`.
async fn probe(runner: &dyn CommandRunner, command: &str, args: &[&str]) -> ProbeResult {
    let owned: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let options = RunOptions {
        timeout: Some(PROBE_TIMEOUT),
        ..Default::default()
    };
    let result = runner.run(command, &owned, options).await;
    if result.timed_out {
        let stderr = if result.stderr.is_empty() {
            format!(
                "{command} {} timed out",
                args.first().copied().unwrap_or("")
            )
        } else {
            result.stderr
        };
        return ProbeResult {
            ok: false,
            stdout: result.stdout,
            stderr,
        };
    }
    let stderr = result.spawn_error.unwrap_or(result.stderr);
    ProbeResult {
        ok: result.raw_exit_code == Some(0),
        stdout: result.stdout,
        stderr,
    }
}

/// What is missing, for [`get_install_guidance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Docker,
    Daemon,
    Compose,
}

/// Platform-specific install steps, relayed to the agent when the sandbox can't start.
pub fn get_install_guidance(missing: Missing) -> String {
    let desktop = cfg!(target_os = "macos") || cfg!(windows);
    let text = match missing {
        Missing::Daemon if desktop => "Docker is installed but the daemon is not running. Start Docker Desktop and wait for its status to read \"running\", then retry.",
        Missing::Daemon => "Docker is installed but the daemon is not running. Start it with `sudo systemctl start docker` (and `sudo systemctl enable docker` to start it at boot), then retry.",
        Missing::Compose if desktop => "Docker Compose is missing. It ships with Docker Desktop — update Docker Desktop to a current version to get the `docker compose` plugin. The sandbox will fall back to a single container without it.",
        Missing::Compose => "Docker Compose is missing. Install the plugin with `sudo apt-get install docker-compose-plugin` (Debian/Ubuntu) or `sudo dnf install docker-compose-plugin` (Fedora/RHEL). The sandbox will fall back to a single container without it.",
        Missing::Docker if cfg!(target_os = "macos") => "Docker is not installed. Install Docker Desktop with `brew install --cask docker`, or download it from https://www.docker.com/products/docker-desktop/. Launch Docker Desktop once installed, then retry.",
        Missing::Docker if cfg!(windows) => "Docker is not installed. Install Docker Desktop with `winget install Docker.DockerDesktop`, or download it from https://www.docker.com/products/docker-desktop/. Launch Docker Desktop once installed, then retry.",
        Missing::Docker => "Docker is not installed. On Debian/Ubuntu: `sudo apt-get install docker.io docker-compose-plugin`. On Fedora/RHEL: `sudo dnf install docker docker-compose-plugin`. Then `sudo systemctl start docker` and add yourself to the `docker` group with `sudo usermod -aG docker $USER` (log out and back in for that to take effect).",
    };
    text.to_string()
}

/// Probe the CLI, daemon, and compose flavor. Compose v2 (`docker compose`) is preferred;
/// v1 (`docker-compose`) is the fallback; with neither, single containers still work.
pub async fn check_docker_availability(runner: &dyn CommandRunner) -> DockerAvailability {
    static VERSION: OnceLock<Regex> = OnceLock::new();
    static SEMVER: OnceLock<Regex> = OnceLock::new();
    let last_checked = now_iso();

    let version = probe(runner, "docker", &["--version"]).await;
    if !version.ok {
        let error = version.stderr.trim();
        return DockerAvailability {
            docker_installed: false,
            docker_version: None,
            daemon_running: false,
            compose: ComposeFlavor::None,
            compose_version: None,
            error: Some(if error.is_empty() {
                "Docker CLI not found on PATH".to_string()
            } else {
                error.to_string()
            }),
            install_guidance: Some(get_install_guidance(Missing::Docker)),
            last_checked,
        };
    }

    let docker_version = VERSION
        .get_or_init(|| Regex::new(r"Docker version (\S+?),?\s").unwrap())
        .captures(&version.stdout)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let info = probe(
        runner,
        "docker",
        &["info", "--format", "{{.ServerVersion}}"],
    )
    .await;
    if !info.ok {
        let error = info.stderr.trim();
        return DockerAvailability {
            docker_installed: true,
            docker_version: Some(docker_version),
            daemon_running: false,
            compose: ComposeFlavor::None,
            compose_version: None,
            error: Some(if error.is_empty() {
                "Docker daemon is not reachable".to_string()
            } else {
                error.to_string()
            }),
            install_guidance: Some(get_install_guidance(Missing::Daemon)),
            last_checked,
        };
    }

    let mut compose = ComposeFlavor::None;
    let mut compose_version = None;

    let plugin = probe(runner, "docker", &["compose", "version", "--short"]).await;
    if plugin.ok {
        compose = ComposeFlavor::Plugin;
        compose_version = Some(plugin.stdout.trim().to_string());
    } else {
        let standalone = probe(runner, "docker-compose", &["--version"]).await;
        if standalone.ok {
            compose = ComposeFlavor::Standalone;
            compose_version = Some(
                SEMVER
                    .get_or_init(|| Regex::new(r"(\d+\.\d+\.\d+)").unwrap())
                    .captures(&standalone.stdout)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            );
        }
    }

    log::debug!(
        target: "docker:availability",
        "Docker availability probed dockerVersion={docker_version} compose={compose:?} composeVersion={compose_version:?}"
    );

    DockerAvailability {
        docker_installed: true,
        docker_version: Some(docker_version),
        daemon_running: true,
        install_guidance: (compose == ComposeFlavor::None)
            .then(|| get_install_guidance(Missing::Compose)),
        compose,
        compose_version,
        error: None,
        last_checked,
    }
}

/// Fail with a message suitable for the agent when the sandbox cannot run. Compose being
/// absent is not fatal — single-container sandboxes still work.
pub fn assert_sandbox_usable(availability: &DockerAvailability) -> Result<()> {
    if !availability.docker_installed {
        return Err(Error::msg(format!(
            "{}\n\n{}",
            availability
                .error
                .as_deref()
                .unwrap_or("Docker is not installed"),
            get_install_guidance(Missing::Docker)
        )));
    }
    if !availability.daemon_running {
        return Err(Error::msg(format!(
            "{}\n\n{}",
            availability
                .error
                .as_deref()
                .unwrap_or("Docker daemon is not running"),
            get_install_guidance(Missing::Daemon)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{FakeRunner, RunResult};

    #[tokio::test]
    async fn reports_missing_cli() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| RunResult {
            exit_code: 127,
            stderr: "spawn docker ENOENT".into(),
            spawn_error: Some("spawn docker ENOENT".into()),
            ..Default::default()
        });
        let availability = check_docker_availability(runner.as_ref()).await;
        assert!(!availability.docker_installed);
        assert_eq!(availability.error.as_deref(), Some("spawn docker ENOENT"));
        assert!(availability.install_guidance.is_some());
        assert!(assert_sandbox_usable(&availability).is_err());
    }

    #[tokio::test]
    async fn detects_plugin_compose() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, args| match args.first().map(String::as_str) {
            Some("--version") => RunResult::ok("Docker version 27.3.1, build ce12230\n"),
            Some("info") => RunResult::ok("27.3.1\n"),
            Some("compose") => RunResult::ok("2.29.7\n"),
            _ => RunResult::failed(1, ""),
        });
        let availability = check_docker_availability(runner.as_ref()).await;
        assert!(availability.daemon_running);
        assert_eq!(availability.docker_version.as_deref(), Some("27.3.1"));
        assert_eq!(availability.compose, ComposeFlavor::Plugin);
        assert_eq!(availability.compose_version.as_deref(), Some("2.29.7"));
        assert!(availability.install_guidance.is_none());
        assert!(assert_sandbox_usable(&availability).is_ok());
    }

    #[tokio::test]
    async fn falls_back_to_standalone_then_none() {
        let runner = FakeRunner::new();
        runner.respond_with(
            |command, args| match (command, args.first().map(String::as_str)) {
                ("docker", Some("--version")) => {
                    RunResult::ok("Docker version 20.10.21, build x\n")
                }
                ("docker", Some("info")) => RunResult::ok("20.10.21\n"),
                ("docker-compose", _) => {
                    RunResult::ok("docker-compose version 1.29.2, build 5becea4c\n")
                }
                _ => RunResult::failed(1, "unknown command"),
            },
        );
        let availability = check_docker_availability(runner.as_ref()).await;
        assert_eq!(availability.compose, ComposeFlavor::Standalone);
        assert_eq!(availability.compose_version.as_deref(), Some("1.29.2"));
    }

    #[tokio::test]
    async fn reports_a_stopped_daemon() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, args| match args.first().map(String::as_str) {
            Some("--version") => RunResult::ok("Docker version 27.3.1, build x\n"),
            _ => RunResult::failed(1, "Cannot connect to the Docker daemon"),
        });
        let availability = check_docker_availability(runner.as_ref()).await;
        assert!(availability.docker_installed);
        assert!(!availability.daemon_running);
        let error = assert_sandbox_usable(&availability)
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("Cannot connect to the Docker daemon\n\n"));
    }
}
