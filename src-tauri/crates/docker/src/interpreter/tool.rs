//! The `codeInterpreter` operations. Port of `CodeInterpreterTool.ts` minus the `BaseTool`
//! plumbing, which the tools crate supplies when it wraps this as a Tool.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{json, Value};

use super::executor::{basename, DockerExecutor};
use super::files::{FileKind, FileManager};
use super::logger::ToolLogger;
use super::tasks::TaskManager;
use super::types::{
    iso_from_ms, AsyncTaskResult, AsyncTaskResultBody, CodeInterpreterInput, CodeInterpreterResult,
    CodeInterpreterResultBody, Operation, PartialExecutionConfig, PythonEnvironment,
    SupportedLanguage, TaskListResult, TaskListResultBody, TaskManagerConfig, TaskStatus,
    TaskSummary, WorkspaceConfig,
};
use crate::manager::Settings;
use crate::runner::{CommandRunner, TokioRunner};
use crate::util::now_iso;
use crate::{Error, Result};

pub const TOOL_NAME: &str = "codeInterpreter";
pub const TOOL_DESCRIPTION: &str = "Execute Python code in a secure Docker environment or manage async tasks. Supports multiple operations: execute (run code), status (check task), cancel (stop task), list (show tasks). No internet access for security.";

const CODE_DESCRIPTION: &str = r#"Python code to execute in a secure Docker container with no internet access.

REQUIRED ONLY for operation="execute" (or when operation is omitted).
NOT needed for operations: "status", "cancel", "list".

CRITICAL FILE HANDLING RULES:
- Input files: Mounted at /data/ directory (READ-ONLY). Access via inputFiles parameter.
- Output files: MUST be created in current working directory (/workspace) to persist on host.
- NEVER attempt to write to /data/ - it will fail with permission error.
- Only files in /workspace are automatically detected and made available on host system.

ENVIRONMENT-SPECIFIC LIBRARIES:
Basic Environment:
- Core: numpy==1.26.2, pandas==2.1.4
- Visualization: matplotlib==3.8.2
- Web: requests==2.31.0
- System: gcc, libffi-dev

Data Science Environment (recommended for analysis):
- Scientific: numpy==1.26.2, pandas==2.1.4, scipy==1.11.4
- Visualization: matplotlib==3.8.2, seaborn==0.13.0, plotly==5.17.0
- ML/Stats: scikit-learn==1.3.2, statsmodels==0.14.0
- Data Processing: openpyxl==3.1.2, beautifulsoup4==4.12.2, lxml==4.9.3
- Image: pillow==10.1.0
- Utils: ipython==8.18.1, requests==2.31.0
- System: gcc, g++, image processing libraries

ENVIRONMENT VARIABLES SET:
- MPLBACKEND='Agg' (for headless plotting)
- PYTHONUNBUFFERED='1' (immediate output)

BEST PRACTICES:
- Use specific library versions as listed above
- Save outputs with descriptive names: 'analysis_results.csv', 'visualization.png'
- Include error handling for file operations
- Create summary reports when performing complex analysis
- Use matplotlib with Agg backend for plot generation

RECOMMENDED OUTPUT PATTERNS:
# Data analysis results
df_results.to_csv('analysis_summary.csv', index=False)

# Visualizations
plt.figure(figsize=(10, 6))
# ... plotting code ...
plt.savefig('chart_analysis.png', dpi=300, bbox_inches='tight')
plt.close()

# Statistical reports
with open('statistical_report.txt', 'w') as f:
    f.write(f"Analysis Summary:\n{summary_text}")

# Processed datasets
cleaned_data.to_excel('processed_data.xlsx', index=False)"#;

/// The Bedrock tool specification (`CodeInterpreterTool.toolSpec`).
pub fn tool_spec() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": TOOL_DESCRIPTION,
        "inputSchema": {
            "json": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": CODE_DESCRIPTION
                    },
                    "environment": {
                        "type": "string",
                        "description": "Python environment selection:\n- \"basic\": Lightweight environment with core libraries (numpy, pandas, matplotlib, requests)\n- \"datascience\": Full data science stack with ML, statistics, and advanced visualization libraries\nDefault: \"datascience\" (recommended for most analytical tasks)",
                        "enum": ["basic", "datascience"]
                    },
                    "inputFiles": {
                        "type": "array",
                        "description": "Input files to mount in the container at /data/ directory (READ-ONLY access).\nPerfect for analyzing existing datasets without risk of modification.\nSupported formats: CSV, Excel (.xlsx, .xls), JSON, images, text files, etc.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "Absolute path to input file on host system.\nExample: \"/Users/user/data/sales.csv\" becomes \"/data/sales.csv\" in container.\nFile will be READ-ONLY - use pandas.read_csv('/data/sales.csv') to access."
                                }
                            },
                            "required": ["path"]
                        }
                    },
                    "async": {
                        "type": "boolean",
                        "description": "Enable asynchronous execution mode. When true, the tool returns immediately with a taskId for monitoring.\n- false (default): Synchronous execution - waits for completion\n- true: Asynchronous execution - returns taskId immediately for long-running tasks"
                    },
                    "operation": {
                        "type": "string",
                        "description": "Operation type for task management:\n- \"execute\" (default): Execute code (sync or async based on async parameter)\n- \"status\": Check status of an async task (requires taskId)\n- \"cancel\": Cancel a running async task (requires taskId)\n- \"list\": List all tasks with optional status filtering",
                        "enum": ["execute", "status", "cancel", "list"]
                    },
                    "taskId": {
                        "type": "string",
                        "description": "Task identifier for async operations. Required when operation is \"status\" or \"cancel\".\nObtained from the response when starting an async execution."
                    },
                    "statusFilter": {
                        "type": "string",
                        "description": "Filter tasks by status (for \"list\" operation only).\nOptional parameter to show only tasks with specific status.",
                        "enum": ["pending", "running", "completed", "failed", "cancelled"]
                    }
                },
                "required": []
            }
        }
    })
}

const MATPLOTLIB_FONT_CONFIG: &str = "# Matplotlib Japanese font configuration\nimport matplotlib\nimport matplotlib.pyplot as plt\nimport os\n\n# Set Japanese font for matplotlib\nif os.path.exists('/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc'):\n    plt.rcParams['font.family'] = ['Noto Sans CJK JP', 'DejaVu Sans']\nelse:\n    # Fallback to DejaVu Sans\n    plt.rcParams['font.family'] = 'DejaVu Sans'\n\n# Disable font warnings\nimport warnings\nwarnings.filterwarnings('ignore', category=UserWarning, module='matplotlib')\n\n# User code starts here\n";

/// Prepend the Japanese font setup when the code looks like it plots.
pub fn add_matplotlib_japanese_font_support(code: &str) -> String {
    if !code.contains("matplotlib") && !code.contains("pyplot") && !code.contains("plt") {
        return code.to_string();
    }
    format!("{MATPLOTLIB_FONT_CONFIG}{code}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

/// Any of the three result shapes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CodeInterpreterOutput {
    Execution(CodeInterpreterResult),
    Task(AsyncTaskResult),
    List(TaskListResult),
}

impl CodeInterpreterOutput {
    pub fn success(&self) -> bool {
        match self {
            CodeInterpreterOutput::Execution(r) => r.success,
            CodeInterpreterOutput::Task(r) => r.success,
            CodeInterpreterOutput::List(r) => r.success,
        }
    }
}

pub struct CodeInterpreter {
    settings: Arc<dyn Settings>,
    logger: Arc<dyn ToolLogger>,
    executor: Arc<DockerExecutor>,
    files: Arc<FileManager>,
    tasks: Arc<TaskManager>,
    session_id: String,
    current_workspace: Mutex<Option<PathBuf>>,
}

fn generate_session_id() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("session_{}_{}", crate::util::now_ms(), &random[..6])
}

fn home_dir() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

impl CodeInterpreter {
    pub fn new(settings: Arc<dyn Settings>, logger: Arc<dyn ToolLogger>) -> Arc<Self> {
        Self::with_runner(settings, logger, Arc::new(TokioRunner))
    }

    pub fn with_runner(
        settings: Arc<dyn Settings>,
        logger: Arc<dyn ToolLogger>,
        runner: Arc<dyn CommandRunner>,
    ) -> Arc<Self> {
        let base_path = settings
            .get("projectPath")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(home_dir);
        let session_id = generate_session_id();
        let workspace = WorkspaceConfig {
            base_path,
            session_id: session_id.clone(),
            max_files: 20,
            max_file_size: 1024 * 1024,
            cleanup_on_exit: true,
        };
        Arc::new(Self {
            files: Arc::new(FileManager::new(logger.clone(), workspace)),
            executor: Arc::new(DockerExecutor::new(logger.clone(), runner)),
            tasks: Arc::new(TaskManager::new(
                logger.clone(),
                TaskManagerConfig::default(),
            )),
            settings,
            logger,
            session_id,
            current_workspace: Mutex::new(None),
        })
    }

    pub fn executor(&self) -> &Arc<DockerExecutor> {
        &self.executor
    }

    /// The workspace of the last run, for the UI (`getCurrentWorkspacePath`).
    pub fn current_workspace_path(&self) -> Option<PathBuf> {
        self.current_workspace.lock().unwrap().clone()
    }

    fn validate_environment(&self, environment: Option<&str>) -> PythonEnvironment {
        match environment {
            None | Some("") => PythonEnvironment::Datascience,
            Some("basic") => PythonEnvironment::Basic,
            Some("datascience") => PythonEnvironment::Datascience,
            Some(other) => {
                self.logger.warn(
                    "Invalid environment specified, using default",
                    json!({ "requested": other, "fallback": "datascience" }),
                );
                PythonEnvironment::Datascience
            }
        }
    }

    /// User settings (`codeInterpreterTool`) over the per-environment defaults.
    pub fn get_execution_config(&self, environment: Option<&str>) -> PartialExecutionConfig {
        let environment = self.validate_environment(environment);
        let mut config = PartialExecutionConfig {
            timeout: Some(30.0),
            memory_limit: Some(
                if environment == PythonEnvironment::Datascience {
                    "256m"
                } else {
                    "128m"
                }
                .to_string(),
            ),
            cpu_limit: Some(0.5),
            environment: Some(environment),
        };
        if let Some(Value::Object(user)) = self.settings.get("codeInterpreterTool") {
            if let Some(timeout) = user.get("timeout").and_then(Value::as_f64) {
                config.timeout = Some(timeout);
            }
            if let Some(memory) = user.get("memoryLimit").and_then(Value::as_str) {
                config.memory_limit = Some(memory.to_string());
            }
            if let Some(cpu) = user.get("cpuLimit").and_then(Value::as_f64) {
                config.cpu_limit = Some(cpu);
            }
        }
        config
    }

    /// Input validation over the raw tool input, mirroring the TS dynamic checks.
    pub fn validate_input(input: &Value) -> ValidationResult {
        let mut errors = Vec::new();
        let operation = input.get("operation");
        let operation_str = operation.and_then(Value::as_str);

        let has_operation = operation.is_some_and(|v| !v.is_null() && v != "");
        if has_operation
            && !matches!(
                operation_str,
                Some("execute" | "status" | "cancel" | "list")
            )
        {
            errors.push(
                "Invalid operation. Must be \"execute\", \"status\", \"cancel\", or \"list\""
                    .to_string(),
            );
        }

        let task_id_missing = input
            .get("taskId")
            .map(|v| v.is_null() || v == "" || v == false)
            .unwrap_or(true);
        if matches!(operation_str, Some("status" | "cancel")) && task_id_missing {
            errors.push("taskId is required for status and cancel operations".to_string());
        }

        if !has_operation || operation_str == Some("execute") {
            let code = input.get("code");
            let falsy = match code {
                None | Some(Value::Null) => true,
                Some(Value::String(s)) => s.is_empty(),
                Some(Value::Bool(b)) => !b,
                Some(Value::Number(n)) => n.as_f64() == Some(0.0),
                _ => false,
            };
            if falsy {
                errors.push("Code is required for execute operations".to_string());
                return ValidationResult {
                    is_valid: false,
                    errors,
                };
            }
            match code {
                Some(Value::String(s)) => {
                    if s.trim().is_empty() {
                        errors.push("Code cannot be empty".to_string());
                    }
                }
                _ => {
                    errors.push("Code must be a string".to_string());
                    return ValidationResult {
                        is_valid: false,
                        errors,
                    };
                }
            }
        }

        if let Some(files) = input.get("inputFiles").filter(|v| !v.is_null()) {
            match files.as_array() {
                None => errors.push("inputFiles must be an array".to_string()),
                Some(files) => {
                    for (index, file) in files.iter().enumerate() {
                        let ok = file
                            .get("path")
                            .and_then(Value::as_str)
                            .is_some_and(|p| !p.is_empty());
                        if !ok {
                            errors.push(format!(
                                "inputFiles[{index}].path is required and must be a string"
                            ));
                        }
                    }
                }
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
        }
    }

    /// Validate and run one tool call.
    pub async fn execute(self: &Arc<Self>, input: Value) -> Result<CodeInterpreterOutput> {
        let validation = Self::validate_input(&input);
        if !validation.is_valid {
            return Err(Error::InvalidInput(format!(
                "Invalid input: {}",
                validation.errors.join(", ")
            )));
        }
        let input: CodeInterpreterInput = serde_json::from_value(input)
            .map_err(|error| Error::InvalidInput(format!("Invalid input: {error}")))?;

        let operation = input.operation.unwrap_or(Operation::Execute);
        self.logger.info(
            "CodeInterpreter operation requested",
            json!({
                "operation": operation,
                "async": input.run_async,
                "taskId": input.task_id,
                "codeLength": input.code.len(),
            }),
        );

        let task_id = input.task_id.clone().unwrap_or_default();
        Ok(match operation {
            Operation::Execute if input.run_async == Some(true) => {
                CodeInterpreterOutput::Task(self.execute_async(input))
            }
            Operation::Execute => CodeInterpreterOutput::Execution(self.execute_sync(&input).await),
            Operation::Status => CodeInterpreterOutput::Task(self.get_task_status(&task_id)),
            Operation::Cancel => CodeInterpreterOutput::Task(self.cancel_task(&task_id)),
            Operation::List => CodeInterpreterOutput::List(self.list_tasks(input.status_filter)),
        })
    }

    async fn execute_sync(&self, input: &CodeInterpreterInput) -> CodeInterpreterResult {
        self.logger.info(
            "Executing Python code synchronously",
            json!({ "codeLength": input.code.len(), "sessionId": self.session_id }),
        );
        match self.try_execute_sync(input).await {
            Ok(result) => result,
            Err(error) => {
                let message = error.to_string();
                self.logger.error(
                    "CodeInterpreter execution failed",
                    json!({ "error": message }),
                );
                CodeInterpreterResult {
                    success: false,
                    name: TOOL_NAME.to_string(),
                    code: input.code.clone(),
                    message: "Execution failed".to_string(),
                    output: String::new(),
                    error: Some(message.clone()),
                    execution_time: 0,
                    result: CodeInterpreterResultBody {
                        code: input.code.clone(),
                        stdout: String::new(),
                        stderr: message,
                        exit_code: 1,
                        files: Vec::new(),
                    },
                }
            }
        }
    }

    async fn try_execute_sync(
        &self,
        input: &CodeInterpreterInput,
    ) -> Result<CodeInterpreterResult> {
        let check = self.executor.check_docker_availability().await;
        if !check.available {
            return Err(Error::msg(format!(
                "Docker is not available: {}",
                check.error.unwrap_or_default()
            )));
        }

        self.files.initialize_workspace()?;
        let workspace = self.files.get_workspace_path()?;
        *self.current_workspace.lock().unwrap() = Some(workspace.clone());

        // Clean up workspaces older than 24 hours in the background.
        let files = self.files.clone();
        std::thread::spawn(move || files.cleanup_old_workspaces(24));

        let processed = add_matplotlib_japanese_font_support(&input.code);
        let config = self.get_execution_config(input.environment.as_deref());
        let execution = self
            .executor
            .execute_code(
                &processed,
                SupportedLanguage::Python,
                &workspace,
                Some(&config),
                input.input_files.as_deref(),
            )
            .await;

        let generated: Vec<String> = self
            .files
            .list_files()
            .into_iter()
            .filter(|f| f.kind == FileKind::File && !f.name.starts_with("temp_"))
            .map(|f| workspace.join(&f.name).to_string_lossy().into_owned())
            .collect();

        let mut output = execution.stdout.clone();
        if let Some(files) = input.input_files.as_ref().filter(|f| !f.is_empty()) {
            let names: Vec<String> = files
                .iter()
                .map(|f| format!("/data/{}", basename(&f.path)))
                .collect();
            output.push_str(&format!("\n[Input files mounted: {}]", names.join(", ")));
        }
        if !generated.is_empty() {
            let names: Vec<String> = generated
                .iter()
                .map(|p| {
                    Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                })
                .collect();
            output.push_str(&format!("\n[Generated files: {}]", names.join(", ")));
        }

        let success = execution.exit_code == 0;
        self.logger.info(
            "Code execution completed",
            json!({
                "success": success,
                "exitCode": execution.exit_code,
                "executionTime": execution.execution_time,
                "generatedFiles": generated.len(),
            }),
        );

        Ok(CodeInterpreterResult {
            success,
            name: TOOL_NAME.to_string(),
            code: input.code.clone(),
            message: if success {
                "Code executed successfully"
            } else {
                "Code execution failed"
            }
            .to_string(),
            output,
            error: (!success && !execution.stderr.is_empty()).then(|| execution.stderr.clone()),
            execution_time: execution.execution_time,
            result: CodeInterpreterResultBody {
                code: input.code.clone(),
                stdout: execution.stdout,
                stderr: execution.stderr,
                exit_code: execution.exit_code,
                files: generated,
            },
        })
    }

    fn execute_async(self: &Arc<Self>, input: CodeInterpreterInput) -> AsyncTaskResult {
        if !self.tasks.can_start_new_task() {
            return AsyncTaskResult {
                success: false,
                name: TOOL_NAME.to_string(),
                task_id: String::new(),
                status: TaskStatus::Failed,
                message: "Cannot start new task - concurrent limit reached".to_string(),
                progress: None,
                result: AsyncTaskResultBody {
                    task_id: String::new(),
                    status: TaskStatus::Failed,
                    created_at: now_iso(),
                    started_at: None,
                    completed_at: None,
                    execution_result: None,
                },
            };
        }

        let environment = self.validate_environment(input.environment.as_deref());
        let task = self
            .tasks
            .create_task(&input.code, environment, input.input_files.clone());

        let this = self.clone();
        let task_id = task.task_id.clone();
        tokio::spawn(async move {
            this.execute_task_in_background(&task_id).await;
        });

        AsyncTaskResult {
            success: true,
            name: TOOL_NAME.to_string(),
            task_id: task.task_id.clone(),
            status: task.status,
            message: "Task created and started".to_string(),
            progress: None,
            result: AsyncTaskResultBody {
                task_id: task.task_id,
                status: task.status,
                created_at: iso_from_ms(task.created_at),
                started_at: None,
                completed_at: None,
                execution_result: None,
            },
        }
    }

    async fn execute_task_in_background(&self, task_id: &str) {
        let Some(task) = self.tasks.get_task(task_id) else {
            self.logger.error(
                "Background task execution failed",
                json!({ "taskId": task_id, "error": format!("Task not found: {task_id}") }),
            );
            return;
        };
        self.tasks
            .update_task_status(task_id, TaskStatus::Running, Some(10));
        let input = CodeInterpreterInput {
            kind: Some(TOOL_NAME.to_string()),
            code: task.code,
            environment: Some(task.environment.as_str().to_string()),
            input_files: task.input_files,
            ..Default::default()
        };
        let result = self.execute_sync(&input).await;
        self.tasks.set_task_result(task_id, result);
    }

    fn get_task_status(&self, task_id: &str) -> AsyncTaskResult {
        let Some(task) = self.tasks.get_task(task_id) else {
            return AsyncTaskResult {
                success: false,
                name: TOOL_NAME.to_string(),
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                message: "Task not found".to_string(),
                progress: None,
                result: AsyncTaskResultBody {
                    task_id: task_id.to_string(),
                    status: TaskStatus::Failed,
                    created_at: now_iso(),
                    started_at: None,
                    completed_at: None,
                    execution_result: None,
                },
            };
        };
        AsyncTaskResult {
            success: true,
            name: TOOL_NAME.to_string(),
            task_id: task.task_id.clone(),
            status: task.status,
            message: format!("Task status: {}", task.status.as_str()),
            progress: task.progress,
            result: AsyncTaskResultBody {
                task_id: task.task_id,
                status: task.status,
                created_at: iso_from_ms(task.created_at),
                started_at: task.started_at.map(iso_from_ms),
                completed_at: task.completed_at.map(iso_from_ms),
                execution_result: task.result,
            },
        }
    }

    fn cancel_task(&self, task_id: &str) -> AsyncTaskResult {
        let cancelled = self.tasks.cancel_task(task_id);
        let task = self.tasks.get_task(task_id);
        let status = task
            .as_ref()
            .map(|t| t.status)
            .unwrap_or(TaskStatus::Failed);
        AsyncTaskResult {
            success: cancelled,
            name: TOOL_NAME.to_string(),
            task_id: task_id.to_string(),
            status,
            message: if cancelled {
                "Task cancelled"
            } else {
                "Failed to cancel task"
            }
            .to_string(),
            progress: None,
            result: AsyncTaskResultBody {
                task_id: task_id.to_string(),
                status,
                created_at: task
                    .map(|t| iso_from_ms(t.created_at))
                    .unwrap_or_else(now_iso),
                started_at: None,
                completed_at: None,
                execution_result: None,
            },
        }
    }

    fn list_tasks(&self, status_filter: Option<TaskStatus>) -> TaskListResult {
        self.logger
            .info("Listing tasks", json!({ "statusFilter": status_filter }));
        let tasks = self.tasks.get_all_tasks(status_filter);
        let summary: TaskSummary = self.tasks.get_task_stats();
        let message = match status_filter {
            Some(filter) => format!(
                "Found {} tasks with status '{}'",
                tasks.len(),
                filter.as_str()
            ),
            None => format!("Found {} total tasks", tasks.len()),
        };
        TaskListResult {
            success: true,
            name: TOOL_NAME.to_string(),
            operation: "list".to_string(),
            tasks: tasks.clone(),
            summary,
            message,
            result: TaskListResultBody {
                tasks,
                summary,
                status_filter,
            },
        }
    }

    /// Cancel tasks, stop running containers, and remove this session's workspace.
    pub async fn dispose(&self) {
        self.tasks.dispose();
        self.executor.stop_all_containers().await;
        self.files.cleanup();
    }
}

#[cfg(test)]
mod tests {
    //! Port of `CodeInterpreterTool.test.ts`, run against the real validation instead of a
    //! copy of it, plus the operations that need no Docker.
    use super::*;
    use crate::interpreter::logger::LogFacadeLogger;
    use crate::runner::{FakeRunner, RunResult};

    fn interpreter(settings: Arc<dyn Settings>) -> Arc<CodeInterpreter> {
        CodeInterpreter::with_runner(settings, Arc::new(LogFacadeLogger), FakeRunner::new())
    }

    fn no_settings() -> Arc<dyn Settings> {
        Arc::new(|_: &str| None)
    }

    #[test]
    fn simplified_input_validation_works_correctly() {
        // Test 1: valid simple code input
        let valid = CodeInterpreter::validate_input(
            &json!({ "type": "codeInterpreter", "code": "print(\"Hello, World!\")" }),
        );
        assert!(valid.is_valid);
        assert!(valid.errors.is_empty());

        // Test 2: missing code
        let missing = CodeInterpreter::validate_input(&json!({ "type": "codeInterpreter" }));
        assert!(!missing.is_valid);
        assert!(missing
            .errors
            .iter()
            .any(|e| e.starts_with("Code is required")));

        // Test 3: whitespace-only code
        let empty =
            CodeInterpreter::validate_input(&json!({ "type": "codeInterpreter", "code": "   " }));
        assert!(!empty.is_valid);
        assert!(empty.errors.contains(&"Code cannot be empty".to_string()));

        // Test 4: non-string code
        let number =
            CodeInterpreter::validate_input(&json!({ "type": "codeInterpreter", "code": 123 }));
        assert!(!number.is_valid);
        assert!(number.errors.contains(&"Code must be a string".to_string()));

        // Test 5: valid complex code
        let complex = CodeInterpreter::validate_input(&json!({
            "type": "codeInterpreter",
            "code": "\nimport pandas as pd\ndata = {'name': ['Alice', 'Bob'], 'age': [25, 30]}\ndf = pd.DataFrame(data)\nprint(df)\ndf.to_csv('output.csv')\nprint(\"File saved!\")\n"
        }));
        assert!(complex.is_valid);
    }

    #[test]
    fn validates_operations_task_ids_and_input_files() {
        let status = CodeInterpreter::validate_input(&json!({ "operation": "status" }));
        assert_eq!(
            status.errors,
            ["taskId is required for status and cancel operations"]
        );
        let list = CodeInterpreter::validate_input(&json!({ "operation": "list" }));
        assert!(list.is_valid, "list needs no code");
        let bad = CodeInterpreter::validate_input(&json!({ "operation": "explode", "code": "x" }));
        assert!(bad.errors[0].starts_with("Invalid operation."));
        let files =
            CodeInterpreter::validate_input(&json!({ "code": "x", "inputFiles": [{ "path": 1 }] }));
        assert_eq!(
            files.errors,
            ["inputFiles[0].path is required and must be a string"]
        );
        let not_array = CodeInterpreter::validate_input(&json!({ "code": "x", "inputFiles": "a" }));
        assert_eq!(not_array.errors, ["inputFiles must be an array"]);
    }

    #[test]
    fn simplified_api_structure_is_as_expected() {
        let spec = tool_spec();
        let properties = spec["inputSchema"]["json"]["properties"]
            .as_object()
            .unwrap();
        assert!(properties.contains_key("code"));
        // Should NOT have complex properties
        for removed in ["action", "language", "files", "config"] {
            assert!(!properties.contains_key(removed));
        }

        let output = serde_json::to_value(CodeInterpreterResult {
            success: true,
            name: TOOL_NAME.into(),
            code: "print(\"Hello!\")".into(),
            message: "Code executed successfully".into(),
            output: "Hello!\n".into(),
            error: None,
            execution_time: 1000,
            result: CodeInterpreterResultBody {
                code: "print(\"Hello!\")".into(),
                stdout: "Hello!\n".into(),
                stderr: String::new(),
                exit_code: 0,
                files: vec![],
            },
        })
        .unwrap();
        for key in ["success", "name", "output", "executionTime", "result"] {
            assert!(output.get(key).is_some(), "missing {key}");
        }
        assert_eq!(output["name"], "codeInterpreter");
        for key in ["stdout", "stderr", "exitCode", "files"] {
            assert!(output["result"].get(key).is_some(), "missing result.{key}");
        }
        assert!(output.get("error").is_none());
    }

    #[test]
    fn uses_fixed_configuration_values() {
        let tool = interpreter(no_settings());
        let basic = tool.get_execution_config(Some("basic"));
        assert_eq!(basic.timeout, Some(30.0));
        assert_eq!(basic.memory_limit.as_deref(), Some("128m"));
        assert_eq!(basic.cpu_limit, Some(0.5));
        assert_eq!(
            tool.get_execution_config(None).memory_limit.as_deref(),
            Some("256m"),
            "datascience is the default and gets more memory"
        );

        let configured = interpreter(Arc::new(|key: &str| {
            (key == "codeInterpreterTool")
                .then(|| json!({ "memoryLimit": "1g", "cpuLimit": 2.0, "timeout": 60 }))
        }));
        let config = configured.get_execution_config(Some("nonsense"));
        assert_eq!(config.timeout, Some(60.0));
        assert_eq!(config.memory_limit.as_deref(), Some("1g"));
        assert_eq!(config.cpu_limit, Some(2.0));
        assert_eq!(config.environment, Some(PythonEnvironment::Datascience));
    }

    #[test]
    fn adds_font_support_only_for_plotting_code() {
        assert_eq!(add_matplotlib_japanese_font_support("print(1)"), "print(1)");
        let plotted = add_matplotlib_japanese_font_support("import matplotlib.pyplot as plt");
        assert!(plotted.starts_with("# Matplotlib Japanese font configuration\n"));
        assert!(plotted.ends_with("# User code starts here\nimport matplotlib.pyplot as plt"));
    }

    #[tokio::test]
    async fn rejects_invalid_input_with_the_base_tool_message() {
        let tool = interpreter(no_settings());
        let error = tool.execute(json!({ "code": "  " })).await.unwrap_err();
        assert_eq!(error.to_string(), "Invalid input: Code cannot be empty");
    }

    #[tokio::test]
    async fn reports_docker_missing_as_a_failed_result() {
        let runner = FakeRunner::new();
        runner.respond_with(|_, _| RunResult::failed(1, ""));
        let tool = CodeInterpreter::with_runner(no_settings(), Arc::new(LogFacadeLogger), runner);
        let output = tool.execute(json!({ "code": "print(1)" })).await.unwrap();
        let CodeInterpreterOutput::Execution(result) = output else {
            panic!("expected an execution result");
        };
        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Docker is not available: Docker not found or not running")
        );
        assert_eq!(result.result.exit_code, 1);
    }

    #[tokio::test]
    async fn manages_async_tasks_without_docker() {
        let tool = interpreter(no_settings());
        let list = tool.execute(json!({ "operation": "list" })).await.unwrap();
        let value = serde_json::to_value(&list).unwrap();
        assert_eq!(value["operation"], "list");
        assert_eq!(value["message"], "Found 0 total tasks");
        assert_eq!(value["summary"]["total"], 0);

        let missing = tool
            .execute(json!({ "operation": "status", "taskId": "task_nope" }))
            .await
            .unwrap();
        assert!(!missing.success());

        let cancel = tool
            .execute(json!({ "operation": "cancel", "taskId": "task_nope" }))
            .await
            .unwrap();
        let value = serde_json::to_value(&cancel).unwrap();
        assert_eq!(value["message"], "Failed to cancel task");
        assert_eq!(value["status"], "failed");
    }
}
