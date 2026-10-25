use super::*;
use crate::embed::fake::FakeEmbedder;

const GUIDE: &str = "# Guide

## 1. Install

Download the app and open it.

## 26. Accounting

### 26.4 A monthly budget

Set a budget on the Accounting page and get a notification at 80% and 100%.

### 26.5 How accurate it is

The figures are estimates from list prices.
";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

async fn built(dir: &Path, guide: &str) -> PathBuf {
    let out = dir.join("help-kb.db");
    build(guide, &out, &FakeEmbedder(LOCAL_MODEL_DIMENSIONS))
        .await
        .unwrap();
    out
}

#[tokio::test]
async fn builds_a_single_file_with_the_guide_hash() {
    let dir = tempfile::tempdir().unwrap();
    let out = built(dir.path(), GUIDE).await;
    assert_eq!(built_hash(&out), Some(guide_hash(GUIDE)));
    // No WAL or build folder left behind.
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["help-kb.db"]);
}

#[tokio::test]
async fn installs_once_and_searches_by_keyword_or_hybrid() {
    let dir = tempfile::tempdir().unwrap();
    let shipped = built(dir.path(), GUIDE).await;
    let root = dir.path().join("knowledgebases");
    let kb = HelpKb::new(shipped.clone(), &root);

    let (mode, hits) = kb.search("budget notification", 3, None).await.unwrap();
    assert_eq!(mode, SearchMode::Keyword);
    assert!(hits[0].heading_path.contains("26.4"), "{hits:?}");
    assert!(kb.installed_path().exists());

    let embedder: Arc<dyn Embedder> = Arc::new(FakeEmbedder(LOCAL_MODEL_DIMENSIONS));
    let (mode, hits) = kb
        .search("monthly budget alert", 3, Some(embedder))
        .await
        .unwrap();
    assert_eq!(mode, SearchMode::Hybrid);
    assert!(hits.iter().any(|h| h.heading_path.contains("26.4")));

    // A new guide ships: the installed copy is replaced on next open.
    let changed = format!("{GUIDE}\n## 27. New\n\nSomething new about widgets.\n");
    std::fs::remove_file(&shipped).unwrap();
    built(dir.path(), &changed).await;
    let kb = HelpKb::new(shipped, &root);
    assert_eq!(kb.guide_hash().unwrap(), Some(guide_hash(&changed)));
    let (_, hits) = kb.search("widgets", 3, None).await.unwrap();
    assert!(hits[0].heading_path.contains("27"));
}

#[tokio::test]
async fn missing_shipped_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let kb = HelpKb::new(dir.path().join("nope.db"), dir.path());
    assert!(kb.search("x", 3, None).await.is_err());
}

/// The shipped index must be rebuilt whenever the guide changes (the pre-commit hook does it;
/// see `build-help-kb`). If this fails, run: `make help-kb`.
#[test]
fn shipped_help_kb_matches_the_guide() {
    let root = repo_root();
    let guide = std::fs::read_to_string(root.join("docs/USER_GUIDE.md")).unwrap();
    let shipped = root.join("docs").join(SHIPPED_FILE);
    assert_eq!(
        built_hash(&shipped),
        Some(guide_hash(&guide)),
        "docs/{SHIPPED_FILE} is stale or missing; run `make help-kb`"
    );
}
