//! Runs Python in throwaway containers. Port of `DockerExecutor.ts`.
//!
//! Every `docker` invocation goes through [`CommandRunner`] (the TS tests mock
//! `child_process.spawn`); running containers are tracked by a [`Killable`] handle so
//! [`DockerExecutor::stop_all_containers`] can SIGTERM them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::logger::ToolLogger;
use super::security::SecurityManager;
use super::types::{
    CodeExecutionResult, DockerBuildResult, DockerImageConfig, ExecutionConfig, InputFile,
    PartialExecutionConfig, PythonEnvironment, SupportedLanguage,
};
use crate::runner::{CommandRunner, KillSwitch, Killable, RunOptions};
use crate::util::{now_iso, resolve_one};
use crate::{Error, Result};

/// Official image used when the custom one cannot be built.
pub const FALLBACK_IMAGE: &str = "python:3.11-slim";
const MAX_INPUT_FILE_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerCheck {
    pub available: bool,
    pub error: Option<String>,
}

pub struct DockerExecutor {
    logger: Arc<dyn ToolLogger>,
    security: SecurityManager,
    runner: Arc<dyn CommandRunner>,
    running_containers: Mutex<HashMap<String, Arc<dyn Killable>>>,
    built_images: Mutex<HashSet<String>>,
}

/// `YYYYMMDD_HHMMSS` in local time, for code file names.
fn date_time_string() -> String {
    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
}

pub(crate) fn get_execution_command(language: SupportedLanguage, filename: &str) -> Vec<String> {
    match language {
        SupportedLanguage::Python => vec!["python".to_string(), filename.to_string()],
    }
}

pub(crate) fn get_file_extension(language: SupportedLanguage) -> &'static str {
    match language {
        SupportedLanguage::Python => ".py",
    }
}

/// The image recipe for each environment.
pub fn get_image_config(environment: PythonEnvironment) -> DockerImageConfig {
    match environment {
        PythonEnvironment::Basic => DockerImageConfig {
            name: "bedrock-python-basic",
            tag: "latest",
            environment,
            libraries: vec![
                "numpy==1.26.2",
                "pandas==2.1.4",
                "matplotlib==3.8.2",
                "requests==2.31.0",
            ],
            system_packages: vec!["gcc", "libffi-dev"],
            environment_variables: vec![
                ("PYTHONUNBUFFERED", "1"),
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("MPLBACKEND", "Agg"),
            ],
        },
        PythonEnvironment::Datascience => DockerImageConfig {
            name: "bedrock-python-datascience",
            tag: "latest",
            environment,
            libraries: vec![
                "numpy==1.26.2",
                "pandas==2.1.4",
                "scipy==1.11.4",
                "matplotlib==3.8.2",
                "seaborn==0.13.0",
                "plotly==5.17.0",
                "scikit-learn==1.3.2",
                "ipython==8.18.1",
                "requests==2.31.0",
                "beautifulsoup4==4.12.2",
                "lxml==4.9.3",
                "openpyxl==3.1.2",
                "pillow==10.1.0",
                "statsmodels==0.14.0",
            ],
            system_packages: vec![
                "gcc",
                "g++",
                "libffi-dev",
                "libssl-dev",
                "libjpeg-dev",
                "libpng-dev",
                "libfreetype6-dev",
                "pkg-config",
                "fonts-noto-cjk",
                "fonts-dejavu-core",
            ],
            environment_variables: vec![
                ("PYTHONUNBUFFERED", "1"),
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("DEBIAN_FRONTEND", "noninteractive"),
                ("MPLBACKEND", "Agg"),
            ],
        },
    }
}

/// The generated Dockerfile, byte for byte what the TS template produced.
pub fn generate_dockerfile(config: &DockerImageConfig) -> String {
    let env_vars = config
        .environment_variables
        .iter()
        .map(|(key, value)| format!("ENV {key}={value}"))
        .collect::<Vec<_>>()
        .join("\n");

    let system_packages = if config.system_packages.is_empty() {
        String::new()
    } else {
        format!(
            "# Install system dependencies\nRUN apt-get update && apt-get install -y \\\n    {} \\\n    && rm -rf /var/lib/apt/lists/*",
            config.system_packages.join(" \\\n    ")
        )
    };

    let libraries = if config.libraries.is_empty() {
        String::new()
    } else {
        format!(
            "# Install Python libraries\nRUN pip install --no-cache-dir \\\n    {}",
            config.libraries.join(" \\\n    ")
        )
    };

    format!(
        r#"# Auto-generated Dockerfile for CodeInterpreter
# Environment: {environment}
# Generated at: {generated}

FROM python:3.11-slim

# Set environment variables
{env_vars}

{system_packages}

# Upgrade pip
RUN pip install --upgrade pip setuptools wheel

{libraries}

# Create workspace and data directories
RUN mkdir -p /workspace /data
WORKDIR /workspace

# Set proper permissions
RUN chmod 755 /workspace /data

# Create non-root user for security
RUN useradd -m -u 1000 coderunner && \
    chown -R coderunner:coderunner /workspace /data

# Switch to non-root user
USER coderunner

# Verify installation (basic check)
RUN python -c "import sys; print(f'Python {{sys.version}}')"

# Default command
CMD ["python"]
"#,
        environment = config.environment.as_str(),
        generated = now_iso(),
    )
}

impl DockerExecutor {
    pub fn new(logger: Arc<dyn ToolLogger>, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            security: SecurityManager::new(logger.clone()),
            logger,
            runner,
            running_containers: Mutex::new(HashMap::new()),
            built_images: Mutex::new(HashSet::new()),
        }
    }

    /// Execute code in a Docker container. Never fails: errors come back as stderr with
    /// exit code 1, as the TS did.
    pub async fn execute_code(
        &self,
        code: &str,
        language: SupportedLanguage,
        execution_path: &Path,
        config: Option<&PartialExecutionConfig>,
        input_files: Option<&[InputFile]>,
    ) -> CodeExecutionResult {
        let started = Instant::now();
        self.logger.info(
            "Starting code execution",
            json!({
                "language": language,
                "codeLength": code.len(),
                "executionPath": execution_path.to_string_lossy(),
                "inputFileCount": input_files.map(<[InputFile]>::len).unwrap_or(0),
            }),
        );

        let outcome: Result<CodeExecutionResult> = async {
            let validation = self.security.validate_execution_config(config);
            if !validation.is_valid {
                return Err(Error::msg(format!(
                    "Invalid execution config: {}",
                    validation.errors.join(", ")
                )));
            }

            let validated = match input_files {
                Some(files) => self.validate_and_prepare_input_files(files),
                None => Vec::new(),
            };

            let sanitized = self.security.sanitize_code(code, "python");
            if !sanitized.warnings.is_empty() {
                self.logger.warn(
                    "Code security warnings",
                    json!({ "warnings": sanitized.warnings }),
                );
            }

            let filename =
                self.create_code_file(&sanitized.sanitized_code, language, execution_path)?;
            self.run_in_docker(
                &filename,
                language,
                execution_path,
                &validation.sanitized_config,
                &validated,
            )
            .await
        }
        .await;

        let execution_time = started.elapsed().as_millis() as u64;
        match outcome {
            Ok(result) => {
                self.logger.info(
                    "Code execution completed",
                    json!({
                        "executionTime": execution_time,
                        "exitCode": result.exit_code,
                        "hasOutput": !result.stdout.is_empty(),
                    }),
                );
                CodeExecutionResult {
                    execution_time,
                    ..result
                }
            }
            Err(error) => {
                self.logger.error(
                    "Code execution failed",
                    json!({ "executionTime": execution_time, "error": error.to_string() }),
                );
                CodeExecutionResult {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: 1,
                    execution_time,
                }
            }
        }
    }

    fn create_code_file(
        &self,
        code: &str,
        language: SupportedLanguage,
        execution_path: &Path,
    ) -> Result<String> {
        let filename = format!(
            "temp_{}{}",
            date_time_string(),
            get_file_extension(language)
        );
        let file_path = execution_path.join(&filename);
        std::fs::write(&file_path, code)?;
        self.logger.debug(
            "Code file created",
            json!({ "filename": filename, "filePath": file_path.to_string_lossy() }),
        );
        Ok(filename)
    }

    async fn run_in_docker(
        &self,
        filename: &str,
        language: SupportedLanguage,
        execution_path: &Path,
        config: &ExecutionConfig,
        input_files: &[InputFile],
    ) -> Result<CodeExecutionResult> {
        let args = self
            .build_docker_args(language, execution_path, filename, config, input_files)
            .await?;
        self.logger.info(
            "Starting Docker container",
            json!({ "args": args.join(" "), "timeout": config.timeout }),
        );

        let kill = KillSwitch::new();
        let container_id = format!("{}-{}", crate::util::now_ms(), uuid::Uuid::new_v4());
        self.running_containers
            .lock()
            .unwrap()
            .insert(container_id.clone(), kill.clone());

        let timeout_secs = if config.timeout > 0.0 {
            config.timeout
        } else {
            30.0
        };
        let options = RunOptions {
            timeout: Some(Duration::from_secs_f64(timeout_secs.min(u32::MAX as f64))),
            kill: Some(kill),
            ..Default::default()
        };
        let result = self.runner.run("docker", &args, options).await;
        self.running_containers
            .lock()
            .unwrap()
            .remove(&container_id);

        if let Some(error) = result.spawn_error {
            self.logger
                .error("Docker process error", json!({ "error": error }));
            return Err(Error::msg(error));
        }
        if result.timed_out {
            self.logger.warn(
                "Docker execution timed out",
                json!({ "timeout": config.timeout }),
            );
            return Ok(CodeExecutionResult {
                stdout: result.stdout,
                stderr: format!("{}\n[Execution timed out]", result.stderr),
                exit_code: 124,
                execution_time: 0,
            });
        }
        Ok(CodeExecutionResult {
            stdout: result.stdout,
            stderr: result.stderr,
            // `code || 0`: a signal death reports 0.
            exit_code: result.raw_exit_code.unwrap_or(0),
            execution_time: 0,
        })
    }

    async fn build_docker_args(
        &self,
        language: SupportedLanguage,
        execution_path: &Path,
        filename: &str,
        config: &ExecutionConfig,
        input_files: &[InputFile],
    ) -> Result<Vec<String>> {
        let mut args = vec!["run".to_string()];
        args.extend(self.security.generate_docker_security_args(Some(config)));

        args.push("-v".into());
        args.push(format!("{}:/workspace", execution_path.to_string_lossy()));
        args.push("-w".into());
        args.push("/workspace".into());

        let data_path = self.ensure_data_directory(execution_path)?;
        args.push("-v".into());
        args.push(format!("{}:/data", data_path.to_string_lossy()));

        if !input_files.is_empty() {
            args.extend(self.generate_file_volume_mounts(input_files));
        }

        let environment = config.environment.unwrap_or(PythonEnvironment::Datascience);
        args.push(self.ensure_docker_image(environment).await);
        args.extend(get_execution_command(language, filename));
        Ok(args)
    }

    fn ensure_data_directory(&self, execution_path: &Path) -> Result<PathBuf> {
        let data_path = execution_path.join("data");
        match std::fs::create_dir_all(&data_path) {
            Ok(()) => {
                self.logger.debug(
                    "Data directory ensured",
                    json!({
                        "dataPath": data_path.to_string_lossy(),
                        "executionPath": execution_path.to_string_lossy(),
                    }),
                );
                Ok(data_path)
            }
            Err(error) => {
                self.logger.error(
                    "Failed to create data directory",
                    json!({ "dataPath": data_path.to_string_lossy(), "error": error.to_string() }),
                );
                Err(Error::msg(format!(
                    "Failed to create data directory: {error}"
                )))
            }
        }
    }

    /// True when `docker images -q <name>` prints an id.
    pub async fn check_custom_image_availability(&self, image_name: &str) -> bool {
        let args = vec![
            "images".to_string(),
            "-q".to_string(),
            image_name.to_string(),
        ];
        let result = self
            .runner
            .run("docker", &args, RunOptions::timeout_ms(3000))
            .await;
        !result.timed_out && result.raw_exit_code == Some(0) && !result.stdout.trim().is_empty()
    }

    /// SIGTERM every running container.
    pub async fn stop_all_containers(&self) {
        let containers: Vec<Arc<dyn Killable>> = {
            let mut running = self.running_containers.lock().unwrap();
            running.drain().map(|(_, handle)| handle).collect()
        };
        self.logger.info(
            "Stopping all running containers",
            json!({ "count": containers.len() }),
        );
        for container in containers {
            if let Err(error) = container.kill() {
                self.logger
                    .warn("Failed to stop container", json!({ "error": error }));
            }
        }
    }

    /// Remove the images this executor built.
    pub async fn cleanup_built_images(&self) {
        let images: Vec<String> = self.built_images.lock().unwrap().drain().collect();
        self.logger
            .info("Cleaning up built images", json!({ "count": images.len() }));
        for image in images {
            if let Err(error) = self.remove_docker_image(&image).await {
                self.logger.warn(
                    "Failed to remove image",
                    json!({ "imageName": image, "error": error.to_string() }),
                );
            }
        }
    }

    async fn remove_docker_image(&self, image_name: &str) -> Result<()> {
        let args = vec!["rmi".to_string(), "-f".to_string(), image_name.to_string()];
        let result = self
            .runner
            .run("docker", &args, RunOptions::timeout_ms(30_000))
            .await;
        if let Some(error) = result.spawn_error {
            return Err(Error::msg(error));
        }
        if result.timed_out {
            return Err(Error::msg(format!("Image removal timed out: {image_name}")));
        }
        if result.raw_exit_code != Some(0) {
            return Err(Error::msg(format!("Failed to remove image {image_name}")));
        }
        Ok(())
    }

    /// Build the environment's image from a generated Dockerfile.
    pub async fn build_docker_image(&self, environment: PythonEnvironment) -> DockerBuildResult {
        let started = Instant::now();
        let config = get_image_config(environment);
        let image_name = format!("{}:{}", config.name, config.tag);
        self.logger.info(
            "Building Docker image",
            json!({
                "imageName": image_name,
                "environment": environment,
                "libraryCount": config.libraries.len(),
            }),
        );

        let build_dir =
            std::env::temp_dir().join(format!("docker-build-{}", crate::util::now_ms()));
        let prepared = std::fs::create_dir_all(&build_dir).and_then(|_| {
            let dockerfile = generate_dockerfile(&config);
            let path = build_dir.join("Dockerfile");
            std::fs::write(&path, &dockerfile)?;
            self.logger.debug(
                "Generated Dockerfile",
                json!({ "path": path.to_string_lossy(), "size": dockerfile.len() }),
            );
            Ok(())
        });

        let outcome = match prepared {
            Err(error) => Err(error.to_string()),
            Ok(()) => {
                let args = vec![
                    "build".to_string(),
                    "-t".to_string(),
                    image_name.clone(),
                    ".".to_string(),
                ];
                let options = RunOptions {
                    cwd: Some(build_dir.clone()),
                    timeout: Some(Duration::from_secs(5 * 60)),
                    ..Default::default()
                };
                let result = self.runner.run("docker", &args, options).await;
                if let Some(error) = result.spawn_error {
                    Err(error)
                } else if result.timed_out {
                    Err("Docker build timed out (5 minutes)".to_string())
                } else if result.raw_exit_code == Some(0) {
                    Ok(())
                } else if !result.stderr.is_empty() {
                    Err(result.stderr)
                } else if !result.stdout.is_empty() {
                    Err(result.stdout)
                } else {
                    Err(format!(
                        "Build failed with exit code {}",
                        result
                            .raw_exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "null".into())
                    ))
                }
            }
        };
        let _ = std::fs::remove_dir_all(&build_dir);

        let build_time = started.elapsed().as_millis() as u64;
        match outcome {
            Ok(()) => {
                self.built_images.lock().unwrap().insert(image_name.clone());
                self.logger.info(
                    "Docker image built successfully",
                    json!({ "imageName": image_name, "buildTime": build_time, "environment": environment }),
                );
                DockerBuildResult {
                    success: true,
                    image_name,
                    build_time,
                    error: None,
                }
            }
            Err(error) => {
                self.logger.error(
                    "Docker image build failed",
                    json!({ "imageName": image_name, "buildTime": build_time, "error": error }),
                );
                DockerBuildResult {
                    success: false,
                    image_name,
                    build_time,
                    error: Some(error),
                }
            }
        }
    }

    /// The environment's image, building it if necessary; the official slim image if the
    /// build fails.
    pub async fn ensure_docker_image(&self, environment: PythonEnvironment) -> String {
        let config = get_image_config(environment);
        let image_name = format!("{}:{}", config.name, config.tag);
        if self.check_custom_image_availability(&image_name).await {
            self.logger.debug(
                "Docker image already exists",
                json!({ "imageName": image_name }),
            );
            return image_name;
        }
        self.logger.info(
            "Docker image not found, building...",
            json!({ "imageName": image_name }),
        );
        let build = self.build_docker_image(environment).await;
        if build.success {
            image_name
        } else {
            self.logger.warn(
                "Failed to build custom image, using fallback",
                json!({ "imageName": image_name, "error": build.error }),
            );
            FALLBACK_IMAGE.to_string()
        }
    }

    /// Keep regular files up to 10MB; skip everything else with a warning.
    fn validate_and_prepare_input_files(&self, input_files: &[InputFile]) -> Vec<InputFile> {
        let mut validated = Vec::new();
        for input in input_files {
            let meta = match std::fs::metadata(&input.path) {
                Ok(meta) => meta,
                Err(error) => {
                    self.logger.warn(
                        "Failed to validate input file",
                        json!({ "path": input.path, "error": error.to_string() }),
                    );
                    continue;
                }
            };
            if !meta.is_file() {
                self.logger.warn(
                    "Input path is not a file, skipping",
                    json!({ "path": input.path }),
                );
                continue;
            }
            let resolved = resolve_one(Path::new(&input.path));
            if !resolved.is_absolute() {
                self.logger.warn(
                    "Invalid file path detected, skipping",
                    json!({ "path": input.path }),
                );
                continue;
            }
            if meta.len() > MAX_INPUT_FILE_SIZE {
                self.logger.warn(
                    "File too large, skipping",
                    json!({ "path": input.path, "size": meta.len(), "maxSize": MAX_INPUT_FILE_SIZE }),
                );
                continue;
            }
            let resolved = resolved.to_string_lossy().into_owned();
            self.logger.debug(
                "Input file validated",
                json!({ "path": resolved, "size": meta.len() }),
            );
            validated.push(InputFile { path: resolved });
        }
        self.logger.info(
            "Input files validation completed",
            json!({ "requested": input_files.len(), "validated": validated.len() }),
        );
        validated
    }

    /// `-v <host>:/data/<basename>` per input file.
    pub(crate) fn generate_file_volume_mounts(&self, input_files: &[InputFile]) -> Vec<String> {
        let mut args = Vec::new();
        for (index, input) in input_files.iter().enumerate() {
            let filename = basename(&input.path);
            let container_path = format!("/data/{filename}");
            args.push("-v".to_string());
            args.push(format!("{}:{container_path}", input.path));
            self.logger.debug(
                "Added file volume mount",
                json!({ "hostPath": input.path, "containerPath": container_path, "index": index }),
            );
        }
        if !input_files.is_empty() {
            self.logger.debug(
                "Input files will be mounted to /data directory",
                json!({ "fileCount": input_files.len() }),
            );
        }
        args
    }

    /// `docker --version` within 5 seconds.
    pub async fn check_docker_availability(&self) -> DockerCheck {
        let args = vec!["--version".to_string()];
        let result = self
            .runner
            .run("docker", &args, RunOptions::timeout_ms(5000))
            .await;
        if let Some(error) = result.spawn_error {
            return DockerCheck {
                available: false,
                error: Some(error),
            };
        }
        if result.timed_out {
            return DockerCheck {
                available: false,
                error: Some("Docker check timed out".to_string()),
            };
        }
        if result.raw_exit_code == Some(0) && result.stdout.contains("Docker version") {
            DockerCheck {
                available: true,
                error: None,
            }
        } else {
            DockerCheck {
                available: false,
                error: Some("Docker not found or not running".to_string()),
            }
        }
    }

    /// Test seam: track a running container handle (the TS tests poke `runningContainers`).
    #[cfg(test)]
    pub(crate) fn track_container(&self, id: &str, handle: Arc<dyn Killable>) {
        self.running_containers
            .lock()
            .unwrap()
            .insert(id.to_string(), handle);
    }

    #[cfg(test)]
    pub(crate) fn running_container_count(&self) -> usize {
        self.running_containers.lock().unwrap().len()
    }
}

/// `path.basename` for either separator.
pub(crate) fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    //! Port of `DockerExecutor.test.ts`. `jest.mock('child_process')` becomes a
    //! [`FakeRunner`]; the `jest.fn()` logger becomes a [`RecordingLogger`].
    use super::*;
    use crate::interpreter::logger::{Level, RecordingLogger};
    use crate::runner::{FakeRunner, RunResult};

    fn executor() -> (DockerExecutor, Arc<FakeRunner>, Arc<RecordingLogger>) {
        let runner = FakeRunner::new();
        let logger = Arc::new(RecordingLogger::default());
        (
            DockerExecutor::new(logger.clone(), runner.clone()),
            runner,
            logger,
        )
    }

    #[tokio::test]
    async fn checks_docker_availability_correctly() {
        let (executor, runner, _) = executor();

        // Test case 1: Docker is available
        runner.respond_with(|_, _| RunResult::ok("Docker version 20.10.21, build baeda1f"));
        let result = executor.check_docker_availability().await;
        assert!(result.available);
        assert_eq!(result.error, None);
        assert_eq!(
            runner.calls(),
            vec![("docker".to_string(), vec!["--version".to_string()])]
        );

        // Test case 2: Docker is not available (command not found exit code)
        runner.reset();
        runner.respond_with(|_, _| RunResult::failed(127, ""));
        let result = executor.check_docker_availability().await;
        assert!(!result.available);
        assert_eq!(
            result.error.as_deref(),
            Some("Docker not found or not running")
        );
    }

    #[tokio::test]
    async fn handles_docker_command_errors() {
        let (executor, runner, _) = executor();
        runner.respond_with(|_, _| RunResult {
            exit_code: 127,
            stderr: "spawn docker ENOENT".into(),
            spawn_error: Some("spawn docker ENOENT".into()),
            ..Default::default()
        });
        let result = executor.check_docker_availability().await;
        assert!(!result.available);
        assert_eq!(result.error.as_deref(), Some("spawn docker ENOENT"));
    }

    struct FakeProcess {
        killed: Mutex<Vec<&'static str>>,
        fail: bool,
    }

    impl Killable for FakeProcess {
        fn kill(&self) -> std::result::Result<(), String> {
            // Killable always delivers SIGTERM, which is what the TS asserted.
            self.killed.lock().unwrap().push("SIGTERM");
            if self.fail {
                Err("Process already stopped".to_string())
            } else {
                Ok(())
            }
        }
    }

    fn fake_process(fail: bool) -> Arc<FakeProcess> {
        Arc::new(FakeProcess {
            killed: Mutex::new(Vec::new()),
            fail,
        })
    }

    #[tokio::test]
    async fn stops_all_containers() {
        let (executor, _, logger) = executor();
        let first = fake_process(false);
        let second = fake_process(false);
        executor.track_container("container1", first.clone());
        executor.track_container("container2", second.clone());

        executor.stop_all_containers().await;

        assert_eq!(*first.killed.lock().unwrap(), ["SIGTERM"]);
        assert_eq!(*second.killed.lock().unwrap(), ["SIGTERM"]);
        assert_eq!(executor.running_container_count(), 0);
        assert!(logger.was_called_with(
            Level::Info,
            "Stopping all running containers",
            &json!({ "count": 2 })
        ));
    }

    #[tokio::test]
    async fn handles_errors_when_stopping_containers() {
        let (executor, _, logger) = executor();
        let process = fake_process(true);
        executor.track_container("container1", process.clone());

        executor.stop_all_containers().await;

        assert_eq!(*process.killed.lock().unwrap(), ["SIGTERM"]);
        assert_eq!(executor.running_container_count(), 0);
        assert!(logger.was_called_with(
            Level::Warn,
            "Failed to stop container",
            &json!({ "error": "Process already stopped" })
        ));
    }

    #[test]
    fn validates_basic_functionality_without_external_dependencies() {
        let (_executor, _, _) = executor();
        assert_eq!(
            get_execution_command(SupportedLanguage::Python, "test.py"),
            ["python", "test.py"]
        );
        assert_eq!(get_file_extension(SupportedLanguage::Python), ".py");
    }

    fn files(paths: &[&str]) -> Vec<InputFile> {
        paths
            .iter()
            .map(|p| InputFile {
                path: p.to_string(),
            })
            .collect()
    }

    #[test]
    fn generates_correct_volume_mount_commands_for_a_single_file() {
        let (executor, _, logger) = executor();
        let args = executor.generate_file_volume_mounts(&files(&["/home/user/data.csv"]));
        assert_eq!(args, ["-v", "/home/user/data.csv:/data/data.csv"]);
        assert!(logger.was_called_with(
            Level::Debug,
            "Added file volume mount",
            &json!({ "hostPath": "/home/user/data.csv", "containerPath": "/data/data.csv", "index": 0 })
        ));
        assert!(logger.was_called_with(
            Level::Debug,
            "Input files will be mounted to /data directory",
            &json!({ "fileCount": 1 })
        ));
    }

    #[test]
    fn generates_correct_volume_mount_commands_for_multiple_files() {
        let (executor, _, logger) = executor();
        let args = executor.generate_file_volume_mounts(&files(&[
            "/home/user/data1.csv",
            "/home/user/data2.json",
            "/home/user/script.py",
        ]));
        assert_eq!(
            args,
            [
                "-v",
                "/home/user/data1.csv:/data/data1.csv",
                "-v",
                "/home/user/data2.json:/data/data2.json",
                "-v",
                "/home/user/script.py:/data/script.py"
            ]
        );
        for (index, (host, container)) in [
            ("/home/user/data1.csv", "/data/data1.csv"),
            ("/home/user/data2.json", "/data/data2.json"),
            ("/home/user/script.py", "/data/script.py"),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(logger.was_called_with(
                Level::Debug,
                "Added file volume mount",
                &json!({ "hostPath": host, "containerPath": container, "index": index })
            ));
        }
        assert!(logger.was_called_with(
            Level::Debug,
            "Input files will be mounted to /data directory",
            &json!({ "fileCount": 3 })
        ));
    }

    #[test]
    fn handles_an_empty_input_files_array() {
        let (executor, _, logger) = executor();
        assert!(executor.generate_file_volume_mounts(&[]).is_empty());
        assert!(!logger.any_containing(Level::Debug, "Added file volume mount"));
        assert!(!logger.any_containing(
            Level::Debug,
            "Input files will be mounted to /data directory"
        ));
    }

    #[test]
    fn handles_files_with_the_same_basename() {
        let (executor, _, logger) = executor();
        let args = executor.generate_file_volume_mounts(&files(&[
            "/home/user/folder1/data.csv",
            "/home/user/folder2/data.csv",
        ]));
        // Both land on /data/data.csv — a known conflict, pinned as current behavior.
        assert_eq!(
            args,
            [
                "-v",
                "/home/user/folder1/data.csv:/data/data.csv",
                "-v",
                "/home/user/folder2/data.csv:/data/data.csv"
            ]
        );
        assert!(logger.was_called_with(
            Level::Debug,
            "Added file volume mount",
            &json!({ "hostPath": "/home/user/folder1/data.csv", "containerPath": "/data/data.csv", "index": 0 })
        ));
        assert!(logger.was_called_with(
            Level::Debug,
            "Added file volume mount",
            &json!({ "hostPath": "/home/user/folder2/data.csv", "containerPath": "/data/data.csv", "index": 1 })
        ));
    }

    #[tokio::test]
    async fn builds_the_full_docker_run_argv() {
        let (executor, runner, _) = executor();
        let workspace = tempfile::tempdir().unwrap();
        let input = workspace.path().join("in.csv");
        std::fs::write(&input, "a,b").unwrap();
        runner.respond_with(|_, args| match args.first().map(String::as_str) {
            Some("images") => RunResult::ok("sha256:abc\n"),
            Some("run") => RunResult::ok("hello\n"),
            _ => RunResult::ok(""),
        });

        let result = executor
            .execute_code(
                "print('hello')",
                SupportedLanguage::Python,
                workspace.path(),
                Some(&PartialExecutionConfig {
                    timeout: Some(30.0),
                    memory_limit: Some("128m".into()),
                    cpu_limit: Some(0.5),
                    environment: Some(PythonEnvironment::Basic),
                }),
                Some(&[InputFile {
                    path: input.to_string_lossy().into_owned(),
                }]),
            )
            .await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello\n");

        let run = runner
            .calls()
            .into_iter()
            .find(|(_, args)| args.first().map(String::as_str) == Some("run"))
            .unwrap()
            .1;
        let ws = workspace.path().to_string_lossy();
        assert_eq!(
            run[..7],
            [
                "run",
                "--rm",
                "--network=none",
                "--tmpfs=/tmp:rw,size=100m",
                "--tmpfs=/var/tmp:rw,size=50m",
                "--memory=128m",
                "--cpus=0.5"
            ]
        );
        assert_eq!(
            run[7..11],
            [
                "-v".to_string(),
                format!("{ws}:/workspace"),
                "-w".into(),
                "/workspace".into()
            ]
        );
        assert_eq!(
            run[11..13],
            [
                "-v".to_string(),
                format!("{}:/data", workspace.path().join("data").to_string_lossy())
            ]
        );
        assert_eq!(
            run[13..15],
            [
                "-v".to_string(),
                format!("{}:/data/in.csv", input.to_string_lossy())
            ]
        );
        assert_eq!(run[15], "bedrock-python-basic:latest");
        assert_eq!(run[16], "python");
        assert!(run[17].starts_with("temp_") && run[17].ends_with(".py"));
        assert!(workspace.path().join(&run[17]).exists());
    }

    #[tokio::test]
    async fn reports_invalid_config_and_timeouts() {
        let (executor, runner, _) = executor();
        let workspace = tempfile::tempdir().unwrap();
        let invalid = executor
            .execute_code(
                "print(1)",
                SupportedLanguage::Python,
                workspace.path(),
                Some(&PartialExecutionConfig {
                    memory_limit: Some("64g".into()),
                    ..Default::default()
                }),
                None,
            )
            .await;
        assert_eq!(invalid.exit_code, 1);
        assert!(invalid
            .stderr
            .starts_with("Invalid execution config: Memory limit must be one of"));

        runner.respond_with(|_, args| match args.first().map(String::as_str) {
            Some("images") => RunResult::ok("sha256:abc\n"),
            _ => RunResult {
                stdout: "Starting long operation...\n".into(),
                exit_code: 124,
                timed_out: true,
                ..Default::default()
            },
        });
        let timed_out = executor
            .execute_code(
                "import time",
                SupportedLanguage::Python,
                workspace.path(),
                None,
                None,
            )
            .await;
        assert_eq!(timed_out.exit_code, 124);
        assert!(timed_out.stderr.contains("[Execution timed out]"));
    }

    #[tokio::test]
    async fn falls_back_to_the_official_image_when_the_build_fails() {
        let (executor, runner, _) = executor();
        runner.respond_with(|_, args| match args.first().map(String::as_str) {
            Some("images") => RunResult::ok(""),
            Some("build") => RunResult::failed(1, "no network"),
            _ => RunResult::ok(""),
        });
        assert_eq!(
            executor
                .ensure_docker_image(PythonEnvironment::Datascience)
                .await,
            FALLBACK_IMAGE
        );
        let build = runner
            .calls()
            .into_iter()
            .find(|(_, args)| args.first().map(String::as_str) == Some("build"))
            .unwrap();
        assert_eq!(
            build.1,
            ["build", "-t", "bedrock-python-datascience:latest", "."]
        );
    }

    #[test]
    fn generates_the_dockerfile_template() {
        let dockerfile = generate_dockerfile(&get_image_config(PythonEnvironment::Basic));
        assert!(dockerfile.starts_with("# Auto-generated Dockerfile for CodeInterpreter\n# Environment: basic\n# Generated at: "));
        assert!(dockerfile.contains(
            "ENV PYTHONUNBUFFERED=1\nENV PYTHONDONTWRITEBYTECODE=1\nENV MPLBACKEND=Agg\n"
        ));
        assert!(dockerfile.contains("RUN apt-get update && apt-get install -y \\\n    gcc \\\n    libffi-dev \\\n    && rm -rf /var/lib/apt/lists/*"));
        assert!(dockerfile.contains(
            "RUN pip install --no-cache-dir \\\n    numpy==1.26.2 \\\n    pandas==2.1.4"
        ));
        assert!(dockerfile.contains("RUN python -c \"import sys; print(f'Python {sys.version}')\""));
        assert!(dockerfile.contains("RUN useradd -m -u 1000 coderunner && \\\n    chown -R coderunner:coderunner /workspace /data"));
        assert!(dockerfile.ends_with("CMD [\"python\"]\n"));
    }
}
