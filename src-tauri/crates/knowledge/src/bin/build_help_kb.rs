//! Build `docs/help-kb.db`, the user guide's knowledge base that ships with the app, from
//! `docs/USER_GUIDE.md` with the on-device embedding model.
//!
//! ```text
//! cargo run --release -p knowledge --bin build-help-kb            # rebuild if the guide changed
//! cargo run --release -p knowledge --bin build-help-kb -- --force # rebuild anyway
//! cargo run -p knowledge --bin build-help-kb -- --check           # exit 1 if stale (no model needed)
//! ```
//!
//! The model is taken from `$HELP_KB_MODELS_DIR`, else the app's own model folder if the model is
//! installed there, else downloaded once into `src-tauri/target/help-kb-models`.

use knowledge::help::{build, built_hash, embedding_spec, guide_hash, SHIPPED_FILE};
use knowledge::local::{is_installed, LocalModel};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The app's knowledge base model folder (`<app data>/Bedrock Engineer/knowledgebases/.models`).
fn app_models_dir() -> Option<PathBuf> {
    [dirs::data_dir(), dirs::config_dir()]
        .into_iter()
        .flatten()
        .map(|d| {
            d.join("Bedrock Engineer")
                .join("knowledgebases")
                .join(".models")
        })
        .find(|d| is_installed(d))
}

fn models_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("HELP_KB_MODELS_DIR") {
        return PathBuf::from(d);
    }
    app_models_dir().unwrap_or_else(|| repo_root().join("src-tauri/target/help-kb-models"))
}

async fn run(check: bool, force: bool) -> Result<ExitCode, String> {
    let root = repo_root();
    let guide_path = root.join("docs/USER_GUIDE.md");
    let out = root.join("docs").join(SHIPPED_FILE);
    let guide = std::fs::read_to_string(&guide_path)
        .map_err(|e| format!("{}: {e}", guide_path.display()))?;
    let current = built_hash(&out);
    let up_to_date = current.as_deref() == Some(guide_hash(&guide).as_str());

    if check {
        if up_to_date {
            println!("{SHIPPED_FILE} is up to date");
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("docs/{SHIPPED_FILE} is stale or missing; run: make help-kb");
        return Ok(ExitCode::FAILURE);
    }
    if up_to_date && !force {
        println!("{SHIPPED_FILE} is up to date");
        return Ok(ExitCode::SUCCESS);
    }

    let models = models_dir();
    let model = Arc::new(LocalModel::new(models.clone()));
    if !is_installed(&models) {
        println!(
            "Downloading the on-device model into {} …",
            models.display()
        );
        let client = reqwest::Client::new();
        model.download(&client, &|_| {}).await?;
    }
    let embedder = model.embedder(&embedding_spec())?;
    println!("Embedding {} …", guide_path.display());
    let started = std::time::Instant::now();
    let chunks = build(&guide, &out, embedder.as_ref()).await?;
    println!(
        "Wrote {} ({chunks} chunks, {} KB) in {:.1}s",
        rel(&out, &root).display(),
        std::fs::metadata(&out).map(|m| m.len() / 1024).unwrap_or(0),
        started.elapsed().as_secs_f64()
    );
    Ok(ExitCode::SUCCESS)
}

fn rel<'a>(p: &'a Path, root: &Path) -> &'a Path {
    p.strip_prefix(root).unwrap_or(p)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let force = args.iter().any(|a| a == "--force");
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("build-help-kb: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(check, force)) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("build-help-kb: {e}");
            ExitCode::FAILURE
        }
    }
}
