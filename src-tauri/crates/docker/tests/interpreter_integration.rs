//! Ports of `DockerExecutor.integration.test.ts` and `CodeInterpreterTool.integration.test.ts`.
//! Require Docker; `#[ignore]`, and each returns early when Docker is unavailable.
//!
//! Run with: `cargo test -p docker --test interpreter_integration -- --ignored`

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use docker::interpreter::{
    CodeInterpreter, CodeInterpreterOutput, DockerExecutor, InputFile, LogFacadeLogger,
    PartialExecutionConfig, SupportedLanguage, TaskStatus,
};
use docker::{Settings, TokioRunner};
use serde_json::{json, Value};

async fn executor() -> Option<DockerExecutor> {
    let executor = DockerExecutor::new(Arc::new(LogFacadeLogger), Arc::new(TokioRunner));
    let check = executor.check_docker_availability().await;
    if !check.available {
        eprintln!("Docker is not available. Skipping: {:?}", check.error);
        return None;
    }
    Some(executor)
}

async fn run(
    executor: &DockerExecutor,
    code: &str,
    workspace: &Path,
) -> docker::interpreter::CodeExecutionResult {
    executor
        .execute_code(code, SupportedLanguage::Python, workspace, None, None)
        .await
}

fn assert_no_python_error(stderr: &str) {
    if !stderr.is_empty() {
        // Image pull/build logs may land here on first run; Python errors must not.
        assert!(!stderr.contains("Traceback"), "{stderr}");
        assert!(!stderr.contains("Error:"), "{stderr}");
    }
}

// describe('DockerExecutor Integration Tests')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn docker_executor_integration() {
    let Some(executor) = executor().await else {
        return;
    };

    // should check actual Docker availability
    let check = executor.check_docker_availability().await;
    assert!(check.available && check.error.is_none());

    // should execute simple Python code successfully
    let ws = tempfile::tempdir().unwrap();
    let result = run(&executor, "print(\"Hello from Docker!\")", ws.path()).await;
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.stdout.trim(), "Hello from Docker!");
    assert_no_python_error(&result.stderr);
    assert!(result.execution_time > 0);

    // should execute Python code with calculations
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "\nimport math\nresult = 2 + 3 * 4\npi_value = math.pi\nprint(f\"Calculation result: {result}\")\nprint(f\"Pi value: {pi_value:.2f}\")\nprint(\"Python math works!\")\n",
        ws.path(),
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("Calculation result: 14"));
    assert!(result.stdout.contains("Pi value: 3.14"));
    assert!(result.stdout.contains("Python math works!"));

    // should handle Python code with file creation
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "\nwith open('output.txt', 'w') as f:\n    f.write('Hello from Python in Docker!\\n')\n    f.write('This is line 2\\n')\n\nwith open('output.txt', 'r') as f:\n    content = f.read()\n    print(\"File content:\")\n    print(content)\n\nprint(\"File operations completed successfully!\")\n",
        ws.path(),
    )
    .await;
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("Hello from Python in Docker!"));
    assert!(result
        .stdout
        .contains("File operations completed successfully!"));
    let written = std::fs::read_to_string(ws.path().join("output.txt")).unwrap();
    assert!(written.contains("This is line 2"));

    // should handle Python syntax errors correctly
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "print(\"Missing closing quote and parenthesis",
        ws.path(),
    )
    .await;
    assert_ne!(result.exit_code, 0);
    assert_eq!(result.stdout, "");
    assert!(result.stderr.contains("SyntaxError"));
    assert!(result.execution_time > 0);

    // should handle Python runtime errors correctly
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "\nprint(\"Starting execution...\")\nresult = 10 / 0\nprint(\"This should not be printed\")\n",
        ws.path(),
    )
    .await;
    assert_ne!(result.exit_code, 0);
    assert!(result.stdout.contains("Starting execution..."));
    assert!(result.stderr.contains("ZeroDivisionError"));

    // should handle execution timeout
    let ws = tempfile::tempdir().unwrap();
    let result = executor
        .execute_code(
            "\nimport time\nprint(\"Starting long operation...\")\ntime.sleep(10)\nprint(\"This should not be printed due to timeout\")\n",
            SupportedLanguage::Python,
            ws.path(),
            Some(&PartialExecutionConfig {
                timeout: Some(2.0),
                memory_limit: Some("128m".into()),
                cpu_limit: Some(0.5),
                environment: None,
            }),
            None,
        )
        .await;
    assert_eq!(result.exit_code, 124);
    assert!(result.stdout.contains("Starting long operation..."));
    assert!(result.stderr.contains("[Execution timed out]"));
    assert!(result.execution_time >= 2000);

    // should work with Python packages (standard library)
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "\nimport json\nimport datetime\nfrom collections import Counter\ndata = {\"name\": \"Docker Test\", \"version\": 1.0, \"active\": True}\nparsed_data = json.loads(json.dumps(data))\nprint(f\"JSON test: {parsed_data['name']}\")\nnow = datetime.datetime.now()\nprint(f\"Current time: {now.strftime('%Y-%m-%d %H:%M:%S')}\")\ncounter = Counter(['apple', 'banana', 'apple'])\nprint(f\"Word count: {dict(counter)}\")\nprint(\"All standard library tests passed!\")\n",
        ws.path(),
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("JSON test: Docker Test"));
    assert!(result.stdout.contains("All standard library tests passed!"));

    // should properly isolate executions (no cross-contamination)
    let ws = tempfile::tempdir().unwrap();
    let first = run(
        &executor,
        "\ntest_variable = \"First execution\"\nprint(f\"Set variable: {test_variable}\")\n",
        ws.path(),
    )
    .await;
    assert!(first.stdout.contains("Set variable: First execution"));
    let second = run(
        &executor,
        "\ntry:\n    print(f\"Variable from previous execution: {test_variable}\")\n    print(\"ERROR: Variable should not exist!\")\nexcept NameError:\n    print(\"SUCCESS: Variable isolation working correctly\")\n",
        ws.path(),
    )
    .await;
    assert!(second
        .stdout
        .contains("SUCCESS: Variable isolation working correctly"));

    // should handle multiple file operations
    let ws = tempfile::tempdir().unwrap();
    let result = run(
        &executor,
        "\nimport os\nfor filename in ['file1.txt', 'file2.txt', 'data.json']:\n    with open(filename, 'w') as f:\n        f.write(f\"Content of {filename}\\n\")\npython_files = [f for f in os.listdir('.') if not f.startswith('temp_') and f != 'data']\nprint(f\"Created {len(python_files)} files:\")\nprint(\"Multi-file operations completed!\")\n",
        ws.path(),
    )
    .await;
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("Created 3 files:"));
    for name in ["file1.txt", "file2.txt", "data.json"] {
        assert!(ws.path().join(name).exists());
    }

    // should mount and access multiple input files
    let ws = tempfile::tempdir().unwrap();
    let inputs = tempfile::tempdir().unwrap();
    std::fs::write(
        inputs.path().join("input1.txt"),
        "This is input file 1\nLine 2 of input1\nLine 3 of input1",
    )
    .unwrap();
    std::fs::write(
        inputs.path().join("input2.csv"),
        "name,age,city\nJohn,25,Tokyo\nJane,30,Osaka\nBob,35,Kyoto",
    )
    .unwrap();
    std::fs::write(
        inputs.path().join("config.json"),
        json!({ "version": "1.0", "environment": "test", "features": ["feature1", "feature2", "feature3"] }).to_string(),
    )
    .unwrap();
    let files: Vec<InputFile> = ["input1.txt", "input2.csv", "config.json"]
        .iter()
        .map(|name| InputFile {
            path: inputs.path().join(name).to_string_lossy().into_owned(),
        })
        .collect();
    let result = executor
        .execute_code(
            "\nimport os, json\nprint(f\"Files in /data: {sorted(os.listdir('/data'))}\")\nwith open('/data/input1.txt') as f:\n    lines = f.read().strip().split('\\n')\nprint(f\"Lines in input1.txt: {len(lines)}\")\nwith open('/data/input2.csv') as f:\n    rows = f.read().strip().split('\\n')\nprint(f\"CSV rows: {len(rows)}\")\nwith open('/data/config.json') as f:\n    config = json.load(f)\nprint(f\"Config version: {config['version']}\")\n",
            SupportedLanguage::Python,
            ws.path(),
            None,
            Some(&files),
        )
        .await;
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        result.stdout.contains("input1.txt")
            && result.stdout.contains("input2.csv")
            && result.stdout.contains("config.json")
    );
    assert!(result.stdout.contains("Lines in input1.txt: 3"));
    assert!(result.stdout.contains("CSV rows: 4"));
    assert!(result.stdout.contains("Config version: 1.0"));

    // should handle file mount errors gracefully
    let ws = tempfile::tempdir().unwrap();
    let valid = ws.path().join("valid.txt");
    std::fs::write(&valid, "This is a valid file").unwrap();
    let result = executor
        .execute_code(
            "\nimport os\nprint(f\"Files in /data: {sorted(os.listdir('/data'))}\")\nfor name in os.listdir('/data'):\n    path = f'/data/{name}'\n    if os.path.isfile(path):\n        print(open(path).read()[:50])\nprint(\"File mount error handling test completed\")\n",
            SupportedLanguage::Python,
            ws.path(),
            None,
            Some(&[
                InputFile { path: valid.to_string_lossy().into_owned() },
                InputFile { path: ws.path().join("nonexistent.txt").to_string_lossy().into_owned() },
            ]),
        )
        .await;
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(result.stdout.contains("valid.txt"));
    assert!(result.stdout.contains("This is a valid file"));
    assert!(!result.stdout.contains("nonexistent.txt"));

    executor.stop_all_containers().await;
}

fn tool(project: &Path) -> Arc<CodeInterpreter> {
    let project = project.to_string_lossy().into_owned();
    let settings: Arc<dyn Settings> =
        Arc::new(move |key: &str| (key == "projectPath").then(|| Value::String(project.clone())));
    CodeInterpreter::new(settings, Arc::new(LogFacadeLogger))
}

fn execution(output: CodeInterpreterOutput) -> docker::interpreter::CodeInterpreterResult {
    match output {
        CodeInterpreterOutput::Execution(result) => result,
        other => panic!("expected an execution result, got {other:?}"),
    }
}

fn task(output: CodeInterpreterOutput) -> docker::interpreter::AsyncTaskResult {
    match output {
        CodeInterpreterOutput::Task(result) => result,
        other => panic!("expected a task result, got {other:?}"),
    }
}

// describe('CodeInterpreterTool Integration Tests')
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires Docker"]
async fn code_interpreter_tool_integration() {
    if executor().await.is_none() {
        return;
    }
    let project = tempfile::tempdir().unwrap();
    let tool = tool(project.path());

    // Synchronous: simple code
    let result = execution(
        tool.execute(
            json!({ "type": "codeInterpreter", "code": "print(\"Hello from CodeInterpreter!\")" }),
        )
        .await
        .unwrap(),
    );
    assert!(result.success, "{:?}", result.error);
    assert_eq!(result.name, "codeInterpreter");
    assert!(result.output.contains("Hello from CodeInterpreter!"));
    assert!(result.error.is_none());
    assert!(result.execution_time > 0);
    assert_eq!(result.result.exit_code, 0);

    // Synchronous: calculations
    let result = execution(
        tool.execute(json!({ "code": "\nresult = 10 + 5 * 2\nprint(f\"Calculation result: {result}\")\nimport math\nprint(f\"Pi rounded: {round(math.pi, 2)}\")\n" }))
            .await
            .unwrap(),
    );
    assert!(result.output.contains("Calculation result: 20"));
    assert!(result.output.contains("Pi rounded: 3.14"));

    // Synchronous: file creation reports generated files
    let result = execution(
        tool.execute(json!({ "code": "\nimport csv\nwith open('people.csv', 'w', newline='') as file:\n    csv.writer(file).writerows([['Name', 'Age'], ['Alice', 25]])\nprint(\"CSV file created successfully!\")\nwith open('readme.txt', 'w') as f:\n    f.write('Created by CodeInterpreter!')\n" }))
            .await
            .unwrap(),
    );
    assert!(result.success);
    assert!(result.result.files[0].contains("people.csv"));
    assert!(result.result.files[1].contains("readme.txt"));

    // Synchronous: Python errors
    let result = execution(
        tool.execute(json!({ "code": "\nprint(\"This will work\")\nresult = 10 / 0\n" }))
            .await
            .unwrap(),
    );
    assert!(!result.success);
    assert!(result.output.contains("This will work"));
    assert!(result.error.unwrap().contains("ZeroDivisionError"));

    // Synchronous: syntax errors
    let result = execution(
        tool.execute(json!({ "code": "print(\"Missing closing quote and parenthesis" }))
            .await
            .unwrap(),
    );
    assert!(!result.success);
    assert!(result.result.stderr.contains("SyntaxError"));

    // Asynchronous: start returns a taskId
    let started = task(
        tool.execute(json!({ "code": "print(\"Hello from async execution!\")", "async": true }))
            .await
            .unwrap(),
    );
    assert!(started.success);
    assert!(!started.task_id.is_empty());
    assert!(matches!(
        started.status,
        TaskStatus::Pending | TaskStatus::Running
    ));
    assert!(started.message.contains("Task created and started"));
    assert_eq!(started.result.task_id, started.task_id);

    // Asynchronous: status
    let started = task(tool.execute(json!({ "code": "import time; time.sleep(1); print(\"Task completed\")", "async": true })).await.unwrap());
    let status = task(
        tool.execute(json!({ "code": "", "operation": "status", "taskId": started.task_id }))
            .await
            .unwrap(),
    );
    assert!(status.success);
    assert_eq!(status.task_id, started.task_id);
    assert!(matches!(
        status.status,
        TaskStatus::Pending | TaskStatus::Running | TaskStatus::Completed
    ));

    // Asynchronous: cancel
    let started = task(
        tool.execute(json!({ "code": "import time; time.sleep(10)", "async": true }))
            .await
            .unwrap(),
    );
    let cancelled = task(
        tool.execute(json!({ "code": "", "operation": "cancel", "taskId": started.task_id }))
            .await
            .unwrap(),
    );
    assert!(cancelled.success);
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(cancelled.message.contains("Task cancelled"));

    // Task list
    let listed = tool
        .execute(json!({ "code": "", "operation": "list" }))
        .await
        .unwrap();
    let value = serde_json::to_value(&listed).unwrap();
    assert_eq!(value["operation"], "list");
    assert!(value["tasks"].is_array());
    assert!(value["summary"]["total"].is_number());
    assert!(value["message"].as_str().unwrap().contains("Found"));

    tokio::time::sleep(Duration::from_millis(100)).await;
    tool.dispose().await;
}
