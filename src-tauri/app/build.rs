use std::path::Path;

/// Must match `build.frontendDist` in tauri.conf.json (relative to this crate).
const FRONTEND_DIST: &str = "../../dist/renderer";

/// The file whose `tauri::generate_handler![...]` list is the single source of truth for the app
/// commands (see `commands::handler`).
const HANDLER_SOURCE: &str = "src/commands/mod.rs";

/// Generated permission set granting every app command (used by `capabilities/default.json`).
const ALL_COMMANDS_SET: &str = "permissions/app-commands.toml";

#[path = "src/command_list.rs"]
mod command_list;

/// Files bundled as app resources, staged under [`STAGED_RESOURCES`] (source relative to this
/// crate, destination relative to the staging dir). `bundle.resources` in tauri.conf.json maps the
/// staged copies into the app; keep the two lists in sync.
const RESOURCES: &[(&str, &str)] = &[
    (
        "../../src/renderer/src/assets/directory-agents",
        "directory-agents",
    ),
    ("../../docs/USER_GUIDE.md", "docs/USER_GUIDE.md"),
    ("../../notices", "notices"),
    ("../../LICENSE", "LICENSE"),
    ("../../NOTICE", "NOTICE"),
];

/// Where [`RESOURCES`] are staged (gitignored).
const STAGED_RESOURCES: &str = "bundle-resources";

/// The license the installers show (`bundle.licenseFile` in tauri.conf.json), staged from
/// `LICENSE` as RTF.
const INSTALLER_LICENSE: (&str, &str) = ("../../LICENSE", "installer-license.rtf");

fn main() {
    ensure_frontend_dist();
    stage_resources();

    // App command ACL: every command in `commands::handler()` gets `allow-<command>` /
    // `deny-<command>` permissions, and `app-commands` bundles all the `allow-*` ones. With an app
    // manifest, Tauri checks every app command against the capabilities, so a window only reaches
    // the commands its capability grants (camera previews: two; PDF export windows: none).
    println!("cargo:rerun-if-changed={HANDLER_SOURCE}");
    let source = std::fs::read_to_string(HANDLER_SOURCE).expect("read commands/mod.rs");
    let commands =
        command_list::handler_commands(&source).unwrap_or_else(|e| panic!("{HANDLER_SOURCE}: {e}"));
    write_if_changed(ALL_COMMANDS_SET, &command_list::all_commands_set(&commands));
    let commands: &'static [&'static str] = Vec::leak(
        commands
            .into_iter()
            .map(|c| &*String::leak(c))
            .collect::<Vec<&'static str>>(),
    );

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build");
}

fn write_if_changed(path: &str, contents: &str) {
    if std::fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return;
    }
    if let Some(dir) = Path::new(path).parent() {
        std::fs::create_dir_all(dir).expect("create permissions dir");
    }
    std::fs::write(path, contents).unwrap_or_else(|e| panic!("write {path}: {e}"));
}

/// `tauri::generate_context!` refuses to compile when `frontendDist` is missing, which would
/// make a plain `cargo build` / `cargo test --workspace` on a fresh clone depend on the JS
/// build. Write a stub page instead so the Rust side always builds; `npm run build:renderer`
/// (or `npm run tauri:build`, which runs it) replaces it with the real bundle.
fn ensure_frontend_dist() {
    let dist = Path::new(FRONTEND_DIST);
    let index = dist.join("index.html");
    if index.exists() {
        return;
    }
    println!(
        "cargo:warning=frontend bundle not found at {FRONTEND_DIST}; writing a stub. \
         Run `npm run build:renderer` for the real UI."
    );
    std::fs::create_dir_all(dist).expect("create frontendDist");
    std::fs::write(
        index,
        "<!doctype html><html><head><meta charset=\"UTF-8\" /><title>Bedrock Engineer</title></head>\
         <body><p>Frontend not built. Run <code>npm run build:renderer</code>.</p></body></html>\n",
    )
    .expect("write stub index.html");
}

/// Copies [`RESOURCES`] into [`STAGED_RESOURCES`] by content only, without extended attributes.
/// The macOS bundler runs `xattr -crs` on the .app and fails if an attribute can't be removed;
/// endpoint DLP agents tag files in the working tree with attributes like that
/// (`com.nextdlp.reveal.*`), and a plain copy (`cp`, `fs::copy`) carries them into the bundle.
/// Writing fresh files leaves them behind, so local `tauri build` works on such Macs.
fn stage_resources() {
    let staged = Path::new(STAGED_RESOURCES);
    if staged.exists() {
        std::fs::remove_dir_all(staged).expect("clear staged resources");
    }
    for (source, dest) in RESOURCES {
        println!("cargo:rerun-if-changed={source}");
        copy_contents(Path::new(source), &staged.join(dest));
    }
    let (source, dest) = INSTALLER_LICENSE;
    println!("cargo:rerun-if-changed={source}");
    let text = std::fs::read_to_string(source).unwrap_or_else(|e| panic!("read {source}: {e}"));
    std::fs::write(staged.join(dest), license_rtf(&text))
        .unwrap_or_else(|e| panic!("write {dest}: {e}"));
}

/// `LICENSE` as RTF for the installers' license pages, which wrap text to their own width: each
/// paragraph's hard-wrapped lines are joined (an 80-column file shows as ragged short lines there),
/// and the first paragraph is the bold title. RTF rather than plain text because the DMG bundler
/// pairs plain text with a fixed style table that bolds characters 39-42 of whatever text it gets.
fn license_rtf(text: &str) -> String {
    let paragraphs: Vec<String> = text
        .split("\n\n")
        .map(|p| {
            p.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace('\\', "\\\\")
                .replace('{', "\\{")
                .replace('}', "\\}")
        })
        .filter(|p| !p.is_empty())
        .collect();
    let mut rtf =
        String::from("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Helvetica;}}\\f0\\fs24\n");
    for (i, paragraph) in paragraphs.iter().enumerate() {
        if i == 0 {
            rtf.push_str(&format!("{{\\b {paragraph}}}\\par\\par\n"));
        } else {
            rtf.push_str(&format!("{paragraph}\\par\\par\n"));
        }
    }
    rtf.push_str("}\n");
    rtf
}

fn copy_contents(source: &Path, dest: &Path) {
    if source.is_dir() {
        std::fs::create_dir_all(dest).unwrap_or_else(|e| panic!("create {}: {e}", dest.display()));
        let entries =
            std::fs::read_dir(source).unwrap_or_else(|e| panic!("read {}: {e}", source.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("read {}: {e}", source.display()));
            copy_contents(&entry.path(), &dest.join(entry.file_name()));
        }
    } else {
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)
                .unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
        }
        let bytes =
            std::fs::read(source).unwrap_or_else(|e| panic!("read {}: {e}", source.display()));
        std::fs::write(dest, bytes).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
    }
}
