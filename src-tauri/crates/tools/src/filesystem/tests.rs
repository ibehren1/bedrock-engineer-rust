//! Behavior tests for the filesystem tools, through `run_tool` (validation, chunking and
//! error wrapping included).

use super::*;
use crate::base::run_tool;
use crate::context::ToolContext;
use crate::types::ToolOutput;
use serde_json::{json, Value};

fn p(dir: &tempfile::TempDir, rel: &str) -> String {
    dir.path().join(rel).to_string_lossy().into_owned()
}

async fn run(tool: &dyn Tool, input: Value) -> crate::Result<ToolOutput> {
    run_tool(tool, input, &ToolContext::default()).await
}

fn text(out: ToolOutput) -> String {
    out.as_text().expect("text output").to_string()
}

/// The JSON response embedded in a wrapped error message.
fn response(err: &crate::ToolError) -> Value {
    serde_json::from_str(&err.message).expect("JSON error message")
}

#[tokio::test]
async fn create_folder_is_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let target = p(&dir, "a/b/c");
    let out = run(
        &CreateFolderTool,
        json!({"type": "createFolder", "path": target}),
    )
    .await
    .unwrap();
    assert_eq!(text(out), format!("Folder created: {target}"));
    assert!(dir.path().join("a/b/c").is_dir());
    // Idempotent.
    run(
        &CreateFolderTool,
        json!({"type": "createFolder", "path": target}),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn create_folder_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = p(&dir, "f");
    std::fs::write(&file, "x").unwrap();
    let err = run(
        &CreateFolderTool,
        json!({"type": "createFolder", "path": format!("{file}/sub")}),
    )
    .await
    .unwrap_err();
    let v = response(&err);
    assert_eq!(v["type"], "EXECUTION");
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .starts_with("Error creating folder: ENOTDIR: not a directory, mkdir '"),
        "{v}"
    );

    let err = run(&CreateFolderTool, json!({"type": "createFolder"}))
        .await
        .unwrap_err();
    assert_eq!(err.name, "Error");
    let v = response(&err);
    assert_eq!(
        v["error"],
        "Invalid input: Path is required, Path must be a string"
    );
    assert_eq!(v["type"], "VALIDATION");
    assert_eq!(v["input"], json!({"type": "createFolder"}));
}

#[tokio::test]
async fn write_to_file_creates_parents_and_echoes_content() {
    let dir = tempfile::tempdir().unwrap();
    let target = p(&dir, "x/y/z.txt");
    let out = run(
        &WriteToFileTool,
        json!({"type": "writeToFile", "path": target, "content": "hello"}),
    )
    .await
    .unwrap();
    assert_eq!(
        text(out),
        format!("Content written to file: {target}\n\nhello")
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");

    // Empty content is allowed; a missing one is not.
    run(
        &WriteToFileTool,
        json!({"type": "writeToFile", "path": target, "content": ""}),
    )
    .await
    .unwrap();
    let err = run(
        &WriteToFileTool,
        json!({"type": "writeToFile", "path": target}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        response(&err)["error"],
        "Invalid input: Content is required, Content must be a string"
    );
}

#[tokio::test]
async fn read_single_file_with_line_range() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(&dir, "a.txt");
    std::fs::write(&f, "l1\nl2\nl3\nl4").unwrap();

    let out = text(
        run(&ReadFilesTool, json!({"type": "readFiles", "paths": [f]}))
            .await
            .unwrap(),
    );
    assert_eq!(
        out,
        format!("File: {f}\n{}\nl1\nl2\nl3\nl4", "=".repeat(f.len() + 6))
    );

    let out = text(
        run(
            &ReadFilesTool,
            json!({"type": "readFiles", "paths": [f], "options": {"lines": {"from": 2, "to": 3}}}),
        )
        .await
        .unwrap(),
    );
    let info = " (lines 2 to 3)";
    assert_eq!(
        out,
        format!(
            "File: {f}{info}\n{}\nl2\nl3",
            "=".repeat(f.len() + info.len() + 6)
        )
    );
}

#[tokio::test]
async fn read_multiple_files_reports_errors_inline() {
    let dir = tempfile::tempdir().unwrap();
    let a = p(&dir, "a.txt");
    let missing = p(&dir, "missing.txt");
    std::fs::write(&a, "A").unwrap();
    let out = text(
        run(
            &ReadFilesTool,
            json!({"type": "readFiles", "paths": [a, missing]}),
        )
        .await
        .unwrap(),
    );
    let expected_a = format!("File: {a}\n{}\nA", "=".repeat(a.len() + 6));
    let expected_missing = format!(
        "## Error reading file: {missing}\nError: ENOENT: no such file or directory, open '{missing}'"
    );
    assert_eq!(out, format!("{expected_a}\n\n{expected_missing}"));
}

#[tokio::test]
async fn read_single_missing_file_is_an_execution_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = p(&dir, "missing.txt");
    let err = run(
        &ReadFilesTool,
        json!({"type": "readFiles", "paths": [missing]}),
    )
    .await
    .unwrap_err();
    let v = response(&err);
    assert_eq!(
        v["error"],
        format!(
            "Error reading file {missing}: ENOENT: no such file or directory, open '{missing}'"
        )
    );
    assert_eq!(v["cause"]["code"], "ENOENT");
    assert_eq!(v["cause"]["syscall"], "open");
}

#[tokio::test]
async fn read_files_validation() {
    let t = ReadFilesTool;
    assert_eq!(
        t.validate_input(&json!({})),
        vec!["Paths array is required", "Paths must be an array"]
    );
    assert_eq!(
        t.validate_input(&json!({"paths": []})),
        vec!["At least one path is required"]
    );
    assert_eq!(
        t.validate_input(&json!({"paths": ["a", 1]})),
        vec!["Path at index 1 must be a string"]
    );
    assert_eq!(
        t.validate_input(&json!({"paths": ["a"], "options": {"lines": {"from": 3, "to": 1}}})),
        vec!["Line range \"from\" must be less than or equal to \"to\""]
    );
}

#[tokio::test]
async fn read_files_encoding_and_excel_as_text() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(&dir, "data.xlsx");
    std::fs::write(&f, [0x68, 0x69]).unwrap();
    let out = text(
        run(
            &ReadFilesTool,
            json!({"type": "readFiles", "paths": [f], "options": {"encoding": "hex"}}),
        )
        .await
        .unwrap(),
    );
    assert!(out.ends_with("\n6869"), "{out}");
}

struct FakeDocs;

#[async_trait]
impl DocumentReader for FakeDocs {
    async fn extract_pdf_text(
        &self,
        path: &str,
        lines: Option<LineRange>,
    ) -> std::result::Result<String, String> {
        Ok(format!("pdf:{path}:{}", lines.is_some()))
    }
    async fn extract_docx_text(
        &self,
        _path: &str,
        _lines: Option<LineRange>,
    ) -> std::result::Result<String, String> {
        Err("bad docx".into())
    }
}

#[tokio::test]
async fn pdf_and_docx_go_through_the_document_reader() {
    let mut ctx = ToolContext::default();
    let err = run_tool(
        &ReadFilesTool,
        json!({"type": "readFiles", "paths": ["/x/a.PDF"]}),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(response(&err)["error"]
        .as_str()
        .unwrap()
        .starts_with("Error reading PDF file /x/a.PDF: "));

    ctx.documents = Some(std::sync::Arc::new(FakeDocs));
    let out = run_tool(
        &ReadFilesTool,
        json!({"type": "readFiles", "paths": ["/x/a.pdf"]}),
        &ctx,
    )
    .await
    .unwrap();
    assert!(text(out).ends_with("\npdf:/x/a.pdf:false"));
    let err = run_tool(
        &ReadFilesTool,
        json!({"type": "readFiles", "paths": ["/x/b.docx"]}),
        &ctx,
    )
    .await
    .unwrap_err();
    assert_eq!(
        response(&err)["error"],
        "Error reading DOCX file /x/b.docx: bad docx"
    );
}

fn make_tree(dir: &tempfile::TempDir) {
    std::fs::create_dir_all(dir.path().join("src/nested")).unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
    std::fs::write(dir.path().join("src/main.test.ts"), "").unwrap();
    std::fs::write(dir.path().join("src/nested/deep.txt"), "").unwrap();
    std::fs::write(dir.path().join("node_modules/pkg/index.js"), "").unwrap();
    std::fs::write(dir.path().join("README.md"), "").unwrap();
}

#[tokio::test]
async fn list_files_tree() {
    let dir = tempfile::tempdir().unwrap();
    make_tree(&dir);
    let root = dir.path().to_string_lossy().into_owned();
    let out = text(
        run(
            &ListFilesTool,
            json!({"type": "listFiles", "path": root, "options": {"ignoreFiles": []}}),
        )
        .await
        .unwrap(),
    );
    let expected = [
        "Directory Structure:",
        "",
        "├── 📄 README.md",
        "├── 📁 node_modules",
        "│   └── 📁 pkg",
        "│       └── 📄 index.js",
        "└── 📁 src",
        "    ├── 📄 main.rs",
        "    ├── 📄 main.test.ts",
        "    └── 📁 nested",
        "        └── 📄 deep.txt",
        "",
    ]
    .join("\n");
    assert_eq!(out, expected);
}

#[tokio::test]
async fn list_files_ignore_depth_and_lines() {
    let dir = tempfile::tempdir().unwrap();
    make_tree(&dir);
    let root = dir.path().to_string_lossy().into_owned();

    // Patterns from the input win; `*.test.ts` and `node_modules` are dropped.
    let out = text(
        run(
            &ListFilesTool,
            json!({"type": "listFiles", "path": root, "options": {"ignoreFiles": ["node_modules", "*.test.ts"]}}),
        )
        .await
        .unwrap(),
    );
    assert!(
        !out.contains("node_modules") && !out.contains("main.test.ts"),
        "{out}"
    );
    assert!(out.contains("deep.txt"));

    // Store defaults (agentChatConfig.ignoreFiles) apply when the input has none.
    let ctx = ToolContext::from_store(&json!({"agentChatConfig": {"ignoreFiles": ["src"]}}), None);
    let out = text(
        run_tool(
            &ListFilesTool,
            json!({"type": "listFiles", "path": root}),
            &ctx,
        )
        .await
        .unwrap(),
    );
    assert!(!out.contains("src") && out.contains("README.md"), "{out}");

    // maxDepth 0 lists the root and elides deeper levels.
    let out = text(
        run(&ListFilesTool, json!({"type": "listFiles", "path": root, "options": {"maxDepth": 0, "ignoreFiles": []}}))
            .await
            .unwrap(),
    );
    assert!(out.contains("└── 📁 src\n    ...\n"), "{out}");
    assert!(!out.contains("main.rs"));

    // Line range over the rendered tree.
    let out = text(
        run(
            &ListFilesTool,
            json!({"type": "listFiles", "path": root, "options": {"ignoreFiles": [], "lines": {"from": 1, "to": 2}}}),
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        out,
        "Directory Structure (lines 1 to 2):\n\n├── 📄 README.md\n├── 📁 node_modules"
    );
}

#[tokio::test]
async fn list_files_errors() {
    let dir = tempfile::tempdir().unwrap();
    let missing = p(&dir, "nope");
    let err = run(
        &ListFilesTool,
        json!({"type": "listFiles", "path": missing}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        response(&err)["error"],
        format!("Error listing directory structure: Error building file tree: ENOENT: no such file or directory, scandir '{missing}'")
    );
    let t = ListFilesTool;
    assert_eq!(
        t.validate_input(&json!({"path": "/", "options": {"maxDepth": -2, "ignoreFiles": "x"}})),
        vec![
            "maxDepth must be -1 or a non-negative number",
            "ignoreFiles must be an array"
        ]
    );
}

#[tokio::test]
async fn apply_diff_edit_replaces_first_occurrence() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(&dir, "code.ts");
    std::fs::write(&f, "const a = 1\nconst a = 1\n").unwrap();
    let out = run(
        &ApplyDiffEditTool,
        json!({"type": "applyDiffEdit", "path": f, "originalText": "const a = 1", "updatedText": "const b = $$2"}),
    )
    .await
    .unwrap();
    assert_eq!(
        out.into_value(),
        json!({
            "name": "applyDiffEdit", "success": true, "message": "Successfully applied diff edit",
            "result": {"path": f, "originalText": "const a = 1", "updatedText": "const b = $$2"}
        })
    );
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        "const b = $2\nconst a = 1\n"
    );

    let out = run(
        &ApplyDiffEditTool,
        json!({"type": "applyDiffEdit", "path": f, "originalText": "zzz", "updatedText": "y"}),
    )
    .await
    .unwrap();
    assert_eq!(
        out.into_value(),
        json!({"name": "applyDiffEdit", "success": false, "error": "Original text not found in file", "result": null})
    );
}

#[tokio::test]
async fn apply_diff_edit_error_shape() {
    let dir = tempfile::tempdir().unwrap();
    let missing = p(&dir, "missing.ts");
    let err = run(
        &ApplyDiffEditTool,
        json!({"type": "applyDiffEdit", "path": missing, "originalText": "a", "updatedText": "b"}),
    )
    .await
    .unwrap_err();
    let outer = response(&err);
    assert_eq!(outer["name"], "applyDiffEdit");
    assert_eq!(outer["success"], false);
    assert_eq!(outer["result"], Value::Null);
    let inner: Value = serde_json::from_str(outer["error"].as_str().unwrap()).unwrap();
    assert!(inner["error"]
        .as_str()
        .unwrap()
        .starts_with("Error applying diff edit: ENOENT"));
    assert_eq!(inner["path"], missing);
}

#[tokio::test]
async fn move_and_copy() {
    let dir = tempfile::tempdir().unwrap();
    let src = p(&dir, "a.txt");
    std::fs::write(&src, "data").unwrap();

    let copy_dest = p(&dir, "copies/b.txt");
    let out = run(
        &CopyFileTool,
        json!({"type": "copyFile", "source": src, "destination": copy_dest}),
    )
    .await
    .unwrap();
    assert_eq!(text(out), format!("File copied: {src} to {copy_dest}"));
    assert_eq!(std::fs::read_to_string(&copy_dest).unwrap(), "data");
    assert!(std::path::Path::new(&src).exists());

    let move_dest = p(&dir, "moved/c.txt");
    let out = run(
        &MoveFileTool,
        json!({"type": "moveFile", "source": src, "destination": move_dest}),
    )
    .await
    .unwrap();
    assert_eq!(text(out), format!("File moved: {src} to {move_dest}"));
    assert!(!std::path::Path::new(&src).exists());
    assert_eq!(std::fs::read_to_string(&move_dest).unwrap(), "data");

    let err = run(
        &MoveFileTool,
        json!({"type": "moveFile", "source": src, "destination": move_dest}),
    )
    .await
    .unwrap_err();
    let v = response(&err);
    assert_eq!(
        v["error"],
        format!(
            "Error moving file: ENOENT: no such file or directory, rename '{src}' -> '{move_dest}'"
        )
    );
    assert_eq!(v["source"], src);
    assert_eq!(v["destination"], move_dest);
    assert_eq!(v["cause"]["dest"], move_dest);

    let err = run(
        &CopyFileTool,
        json!({"type": "copyFile", "source": "", "destination": 1}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        response(&err)["error"],
        "Invalid input: Source path is required, Destination path must be a string"
    );
}

#[tokio::test]
async fn oversized_results_are_chunked() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(&dir, "big.txt");
    std::fs::write(&f, "y".repeat(1000)).unwrap();
    let ctx = ToolContext::from_store(&json!({"inferenceParams": {"maxTokens": 50}}), None);
    let out = text(
        run_tool(
            &ReadFilesTool,
            json!({"type": "readFiles", "paths": [f]}),
            &ctx,
        )
        .await
        .unwrap(),
    );
    // 50 tokens -> 160-char chunks.
    let (head, tail) = out.split_once("\n\n[Content truncated").unwrap();
    assert_eq!(head.chars().count(), 160);
    assert!(tail.contains("configured 50 token limit"));
}
