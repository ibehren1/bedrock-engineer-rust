//! The TS converter has no test file, so parity is checked against its actual output:
//! `testdata/golden.json` holds `CodeGenerator.generateStrandsAgent()` results for the fixture
//! agents in that file, generated from `src/main/services/strandsAgentsConverter` with
//! `new Date()` pinned to [`FIXED_NOW`].

use super::*;
use chrono::TimeZone;
use serde_json::{json, Value};

const FIXED_NOW: &str = "2026-09-30T12:34:56.789Z";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(FIXED_NOW)
        .unwrap()
        .with_timezone(&Utc)
}

fn golden() -> Vec<Value> {
    serde_json::from_str(include_str!("../testdata/golden.json")).unwrap()
}

#[test]
fn output_matches_ts_generator_byte_for_byte() {
    for case in golden() {
        let agent = &case["agent"];
        let out = generate_strands_agent(agent, now()).unwrap();
        let name = agent["name"].as_str().unwrap();
        assert_eq!(
            out.python_code,
            case["pythonCode"].as_str().unwrap(),
            "agent.py: {name}"
        );
        assert_eq!(
            out.requirements_text,
            case["requirementsText"].as_str().unwrap(),
            "requirements.txt: {name}"
        );
        assert_eq!(
            out.readme_text,
            case["readmeText"].as_str().unwrap(),
            "README.md: {name}"
        );
        assert_eq!(
            out.config_yaml_text,
            case["configYamlText"].as_str().unwrap(),
            "config.yaml: {name}"
        );
    }
}

#[test]
fn fixed_now_is_iso_millis() {
    assert_eq!(
        Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "2026-01-02T03:04:05.000Z"
    );
}

#[test]
fn var_names_and_prompt_escaping() {
    assert_eq!(
        generator::sanitize_var_name("AWS Docs-Server"),
        "aws_docs_server"
    );
    // One underscore per UTF-16 code unit, as the JS regex did.
    assert_eq!(generator::sanitize_var_name("é🚀x"), "___x");
    assert_eq!(
        generator::process_system_prompt("  a\n\"\"\"b\\ \n"),
        "a\\n\\\"\\\"\\\"b\\\\ \\n"
    );
}

#[test]
fn tool_mapping_splits_supported_and_unsupported() {
    let m = generator::analyze_and_map_tools(&[json!("think"), json!("todo"), json!("someMcp")]);
    assert_eq!(m.supported_tools.len(), 1);
    assert_eq!(m.supported_tools[0].strands_tool.strands_name, "think");
    let reasons: Vec<_> = m
        .unsupported_tools
        .iter()
        .map(|t| (t.original_name.as_str(), t.reason.as_str()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            (
                "todo",
                "TODO tools are excluded from conversion. Can be replaced with Workflow tools"
            ),
            ("someMcp", "MCP tools are currently not supported"),
        ]
    );
}

#[test]
fn validation_errors_become_conversion_errors() {
    let dir = tempfile::tempdir().unwrap();
    let options = SaveOptions {
        output_directory: dir.path().to_string_lossy().into_owned(),
        overwrite: true,
        ..Default::default()
    };
    for (agent, message) in [
        (json!({ "system": "x" }), "Agent name is required"),
        (
            json!({ "name": "  ", "system": "x" }),
            "Agent name is required",
        ),
        (json!({ "name": "A" }), "Agent system prompt is required"),
        (
            json!({ "name": "A", "system": " \n" }),
            "Agent system prompt is required",
        ),
    ] {
        let r = convert_and_save_agent(&agent, &options, now());
        assert_eq!(
            r,
            SaveResult {
                success: false,
                output_directory: options.output_directory.clone(),
                saved_files: vec![],
                errors: vec![FileError {
                    file: "conversion".into(),
                    error: message.into()
                }],
            }
        );
    }
    let r = convert_and_save_agent(
        &json!({ "name": "A", "system": "x", "mcpServers": [{ "command": "c" }] }),
        &options,
        now(),
    );
    assert_eq!(
        r.errors[0].error,
        "Cannot read properties of undefined (reading 'toLowerCase')"
    );
}

#[test]
fn saves_the_three_files_and_serializes_like_ts() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = dir.path().join("nested/out");
    let options = SaveOptions {
        output_directory: out_dir.to_string_lossy().into_owned(),
        overwrite: true,
        ..Default::default()
    };
    let agent = json!({ "name": "Minimal", "system": "Hi" });
    let r = convert_and_save_agent(&agent, &options, now());
    assert!(r.success, "{r:?}");
    let names: Vec<_> = r
        .saved_files
        .iter()
        .map(|p| {
            Path::new(p)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, vec!["agent.py", "requirements.txt", "README.md"]);
    assert!(!out_dir.join("config.yaml").exists());
    let expected = generate_strands_agent(&agent, now()).unwrap();
    assert_eq!(
        std::fs::read_to_string(out_dir.join("agent.py")).unwrap(),
        expected.python_code
    );

    let v = serde_json::to_value(&r).unwrap();
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        vec!["success", "outputDirectory", "savedFiles", "errors"]
    );
    assert_eq!(v["errors"], json!([]));

    // Overwrite is what the handler asks for; the files are replaced.
    assert!(convert_and_save_agent(&agent, &options, now()).success);
}

#[test]
fn existing_files_without_overwrite_are_errors_and_config_is_optional() {
    let dir = tempfile::tempdir().unwrap();
    let options = SaveOptions {
        output_directory: dir.path().to_string_lossy().into_owned(),
        include_config: true,
        agent_file_name: Some("main.py".into()),
        overwrite: false,
    };
    std::fs::write(dir.path().join("README.md"), "mine").unwrap();
    let r = convert_and_save_agent(&json!({ "name": "A", "system": "x" }), &options, now());
    assert!(!r.success);
    assert_eq!(r.saved_files.len(), 3);
    assert!(dir.path().join("main.py").exists());
    assert!(dir.path().join("config.yaml").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
        "mine"
    );
    assert_eq!(
        r.errors,
        vec![FileError {
            file: "README.md".into(),
            error: "File already exists: README.md (use overwrite: true to replace)".into()
        }]
    );
}

#[test]
fn bad_save_options_are_directory_errors() {
    let output = generate_strands_agent(&json!({ "name": "A", "system": "x" }), now()).unwrap();
    let r = save_agent_to_directory(
        &output,
        &SaveOptions {
            output_directory: "  ".into(),
            ..Default::default()
        },
    );
    assert_eq!(
        r.errors,
        vec![FileError {
            file: "directory".into(),
            error: "Output directory is required".into()
        }]
    );
    let dir = tempfile::tempdir().unwrap();
    let r = save_agent_to_directory(
        &output,
        &SaveOptions {
            output_directory: dir.path().to_string_lossy().into_owned(),
            agent_file_name: Some("agent.txt".into()),
            ..Default::default()
        },
    );
    assert_eq!(r.errors[0].error, "Agent file name must end with .py");
    assert!(!r.success);
}
