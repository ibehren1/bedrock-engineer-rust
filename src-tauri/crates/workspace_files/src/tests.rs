use super::*;
use crate::serve::{path_from_uri, serve};
use std::sync::mpsc;
use std::time::Duration;

fn project() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let root = canonical_root(d.path()).unwrap();
    (d, root)
}

fn write(root: &Path, rel: &str, content: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn header<'a>(s: &'a serve::Served, name: &str) -> Option<&'a str> {
    s.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[test]
fn no_root_and_missing_root() {
    assert_eq!(canonical_root(Path::new("")), Err(Error::NoRoot));
    assert_eq!(Error::NoRoot.to_string(), "No project folder is set");
    assert!(matches!(
        canonical_root(Path::new("/definitely/not/here")),
        Err(Error::RootMissing(_))
    ));
    assert!(list_dir(Path::new(""), "").is_err());
}

#[test]
fn resolve_accepts_relative_absolute_and_new_files() {
    let (_d, root) = project();
    write(&root, "a/b.txt", b"x");
    assert_eq!(resolve_in_root(&root, "").unwrap(), root);
    assert_eq!(
        resolve_in_root(&root, "a/b.txt").unwrap(),
        root.join("a/b.txt")
    );
    assert_eq!(
        resolve_in_root(&root, "./a/../a/b.txt").unwrap(),
        root.join("a/b.txt")
    );
    let abs = root.join("a/b.txt").display().to_string();
    assert_eq!(resolve_in_root(&root, &abs).unwrap(), root.join("a/b.txt"));
    // A file that doesn't exist yet resolves through its parent.
    assert_eq!(
        resolve_in_root(&root, "a/new.md").unwrap(),
        root.join("a/new.md")
    );
    // ... whose parent must exist.
    assert!(matches!(
        resolve_in_root(&root, "nope/new.md"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn resolve_rejects_escapes() {
    let (_d, root) = project();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), "s").unwrap();
    write(&root, "a/b.txt", b"x");
    for bad in ["..", "../", "a/../..", "../x.txt", "a/../../new.txt"] {
        assert_eq!(resolve_in_root(&root, bad), Err(Error::Outside), "{bad}");
    }
    let abs = outside.path().join("secret.txt").display().to_string();
    assert_eq!(resolve_in_root(&root, &abs), Err(Error::Outside));
    let abs_new = outside.path().join("new.txt").display().to_string();
    assert_eq!(resolve_in_root(&root, &abs_new), Err(Error::Outside));
    // Lexical escapes through folders that don't exist.
    assert_eq!(
        resolve_in_root(&root, "missing/../../../etc/x"),
        Err(Error::Outside)
    );
    assert_eq!(
        Error::Outside.to_string(),
        "Path is outside the project folder"
    );
}

#[cfg(unix)]
#[test]
fn resolve_rejects_symlink_escapes() {
    let (_d, root) = project();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), "s").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("out")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.txt"), root.join("s.txt")).unwrap();
    write(&root, "in.txt", b"i");
    std::os::unix::fs::symlink(root.join("in.txt"), root.join("link.txt")).unwrap();
    assert_eq!(
        resolve_in_root(&root, "out/secret.txt"),
        Err(Error::Outside)
    );
    assert_eq!(resolve_in_root(&root, "out/new.txt"), Err(Error::Outside));
    assert_eq!(resolve_in_root(&root, "s.txt"), Err(Error::Outside));
    assert_eq!(
        resolve_in_root(&root, "link.txt").unwrap(),
        root.join("in.txt")
    );
    assert!(read_text(&root, "s.txt", MAX_TEXT_BYTES).is_err());
    assert!(write_text(&root, "out/x.txt", "x", None).is_err());
    assert!(!outside.path().join("x.txt").exists());
    assert!(list_dir(&root, "out").is_err());
}

#[test]
fn list_orders_dirs_first_and_skips() {
    let (_d, root) = project();
    write(&root, "b.txt", b"hello");
    write(&root, "A.md", b"#");
    write(&root, "c.PNG", b"");
    write(&root, "noext", b"");
    write(&root, "zdir/x.txt", b"");
    write(&root, "Adir/x.txt", b"");
    write(&root, "attachments/x.txt", b"");
    for skipped in [
        ".git/HEAD",
        "node_modules/x/y.js",
        "target/debug/a",
        "docker-sandboxes/s/x",
    ] {
        write(&root, skipped, b"");
    }
    write(&root, ".DS_Store", b"");
    write(
        &root,
        ".gitignore",
        b"# comment\n\n*.log\ndist/\n/build\n!keep.log\n",
    );
    write(&root, "app.log", b"");
    write(&root, "keep.log", b"");
    write(&root, "dist/out.js", b"");
    write(&root, "build/out.js", b"");
    write(&root, "zdir/build/kept.js", b"");
    write(&root, "zdir/inner.log", b"");

    let listing = list_dir(&root, "").unwrap();
    assert_eq!(listing.root, root.display().to_string());
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Adir",
            "attachments",
            "zdir",
            ".gitignore",
            "A.md",
            "b.txt",
            "c.PNG",
            "keep.log",
            "noext"
        ]
    );
    let b = &listing.entries[5];
    assert_eq!(b.path, "b.txt");
    assert_eq!(b.kind, Kind::File);
    assert_eq!(b.size, 5);
    assert_eq!(b.ext, "txt");
    assert!(b.modified_ms > 0);
    assert_eq!(listing.entries[6].ext, "png");
    assert_eq!(listing.entries[8].ext, "");
    assert_eq!(listing.entries[0].kind, Kind::Dir);
    assert_eq!(listing.entries[0].size, 0);
    assert_eq!(listing.entries[0].ext, "");

    let sub = list_dir(&root, "zdir").unwrap();
    let paths: Vec<&str> = sub.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["zdir/build", "zdir/x.txt"]);

    let json = serde_json::to_value(&sub.entries[1]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "name": "x.txt", "path": "zdir/x.txt", "kind": "file", "size": 0,
            "modifiedMs": sub.entries[1].modified_ms, "ext": "txt"
        })
    );
    assert!(matches!(list_dir(&root, "b.txt"), Err(Error::NotADir(_))));
    assert!(matches!(list_dir(&root, "nope"), Err(Error::NotFound(_))));
    assert_eq!(list_dir(&root, ".."), Err(Error::Outside));
}

#[test]
fn read_text_caps_and_refuses_binary() {
    let (_d, root) = project();
    write(&root, "a.txt", "héllo".as_bytes());
    let t = read_text(&root, "a.txt", MAX_TEXT_BYTES).unwrap();
    assert_eq!(t.content, "héllo");
    assert!(!t.truncated);
    assert_eq!(t.size, 6);
    assert!(t.modified_ms > 0);
    // Cut inside the two-byte "é": the partial char is dropped.
    let t = read_text(&root, "a.txt", 2).unwrap();
    assert_eq!(t.content, "h");
    assert!(t.truncated);
    assert_eq!(t.size, 6);
    let t = read_text(&root, "a.txt", 3).unwrap();
    assert_eq!(t.content, "hé");

    write(&root, "nul.bin", b"abc\0def");
    assert_eq!(
        read_text(&root, "nul.bin", MAX_TEXT_BYTES),
        Err(Error::NotText)
    );
    write(&root, "latin1.txt", b"caf\xe9 au lait");
    assert_eq!(
        read_text(&root, "latin1.txt", MAX_TEXT_BYTES),
        Err(Error::NotText)
    );
    assert_eq!(Error::NotText.to_string(), "Not a text file");
    assert!(matches!(
        read_text(&root, "missing.txt", MAX_TEXT_BYTES),
        Err(Error::NotFound(_))
    ));
    fs::create_dir(root.join("d")).unwrap();
    assert!(matches!(
        read_text(&root, "d", MAX_TEXT_BYTES),
        Err(Error::NotAFile(_))
    ));
    // Big files: cap at 2 MB.
    write(&root, "big.txt", &vec![b'a'; MAX_TEXT_BYTES + 10]);
    let t = read_text(&root, "big.txt", MAX_TEXT_BYTES).unwrap();
    assert_eq!(t.content.len(), MAX_TEXT_BYTES);
    assert!(t.truncated);
}

#[test]
fn write_text_creates_replaces_and_detects_conflicts() {
    let (_d, root) = project();
    let w = write_text(&root, "new.md", "# hi", None).unwrap();
    assert_eq!(fs::read_to_string(root.join("new.md")).unwrap(), "# hi");
    let meta_ms = read_text(&root, "new.md", MAX_TEXT_BYTES)
        .unwrap()
        .modified_ms;
    assert_eq!(w.modified_ms, meta_ms);

    // Matching mtime: written.
    let w2 = write_text(&root, "new.md", "v2", Some(w.modified_ms as f64)).unwrap();
    assert_eq!(fs::read_to_string(root.join("new.md")).unwrap(), "v2");
    // Stale mtime: conflict, file untouched.
    let err = write_text(&root, "new.md", "v3", Some((w2.modified_ms - 5000) as f64)).unwrap_err();
    assert!(err.to_string().starts_with("CONFLICT:"), "{err}");
    assert_eq!(fs::read_to_string(root.join("new.md")).unwrap(), "v2");
    // Expected an existing file that is gone: conflict too.
    let err = write_text(&root, "gone.md", "x", Some(1.0)).unwrap_err();
    assert!(err.to_string().starts_with("CONFLICT:"));
    assert!(!root.join("gone.md").exists());

    // No temp files left behind.
    let leftovers: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    assert!(matches!(
        write_text(&root, "nodir/x.md", "x", None),
        Err(Error::NotFound(_))
    ));
    assert_eq!(write_text(&root, "../x.md", "x", None), Err(Error::Outside));
    fs::create_dir(root.join("d")).unwrap();
    assert!(matches!(
        write_text(&root, "d", "x", None),
        Err(Error::NotAFile(_))
    ));
    assert!(matches!(
        write_text(&root, "", "x", None),
        Err(Error::NotAFile(_))
    ));
}

#[cfg(unix)]
#[test]
fn write_text_keeps_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let (_d, root) = project();
    write(&root, "run.sh", b"echo");
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    write_text(&root, "run.sh", "echo hi", None).unwrap();
    let mode = fs::metadata(root.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
}

#[test]
fn exists_reports_kind_and_relative_path() {
    let (_d, root) = project();
    write(&root, "docs/a.md", b"");
    let abs = root.join("docs/a.md").display().to_string();
    let inputs: Vec<String> = ["docs/a.md", "docs", "missing.md", "../x", abs.as_str()]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let found = exists(&root, &inputs).unwrap();
    assert_eq!(
        found["docs/a.md"],
        Some(Found {
            path: "docs/a.md".into(),
            kind: Kind::File
        })
    );
    assert_eq!(found["docs"].as_ref().unwrap().kind, Kind::Dir);
    assert_eq!(found["missing.md"], None);
    assert_eq!(found["../x"], None);
    assert_eq!(found[&abs].as_ref().unwrap().path, "docs/a.md");
    let json = serde_json::to_value(&found).unwrap();
    assert_eq!(
        json["docs"],
        serde_json::json!({ "path": "docs", "kind": "dir" })
    );
    assert_eq!(json["missing.md"], serde_json::Value::Null);
}

#[test]
fn mime_types() {
    assert_eq!(mime_for("a.PDF"), "application/pdf");
    assert_eq!(
        mime_for("r.docx"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert_eq!(mime_for("x.png"), "image/png");
    assert_eq!(mime_for("x.jpg"), "image/jpeg");
    assert_eq!(mime_for("x.JPEG"), "image/jpeg");
    assert_eq!(mime_for("x.gif"), "image/gif");
    assert_eq!(mime_for("x.webp"), "image/webp");
    assert_eq!(mime_for("x.svg"), "image/svg+xml");
    assert_eq!(mime_for("index.html"), "text/plain; charset=utf-8");
    assert_eq!(mime_for("index.htm"), "text/plain; charset=utf-8");
    assert_eq!(mime_for("x.js"), "application/octet-stream");
    assert_eq!(mime_for("noext"), "application/octet-stream");
}

#[test]
fn serve_status_codes_and_decoding() {
    let (_d, root) = project();
    write(&root, "my docs/r é.pdf", b"%PDF-1.7 data");
    write(&root, "page.html", b"<script>alert(1)</script>");
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("s.txt"), "s").unwrap();

    let ok = serve(Some(&root), "GET", "/my%20docs%2Fr%20%C3%A9.pdf", None);
    assert_eq!(ok.status, 200);
    assert_eq!(ok.body, b"%PDF-1.7 data");
    assert_eq!(header(&ok, "content-type"), Some("application/pdf"));
    assert_eq!(header(&ok, "cache-control"), Some("no-store"));
    assert_eq!(header(&ok, "content-length"), Some("13"));
    assert_eq!(header(&ok, "x-content-type-options"), Some("nosniff"));

    let html = serve(Some(&root), "GET", "/page.html", None);
    assert_eq!(html.status, 200);
    assert_eq!(
        header(&html, "content-type"),
        Some("text/plain; charset=utf-8")
    );
    assert!(header(&html, "content-security-policy")
        .unwrap()
        .contains("sandbox"));

    assert_eq!(serve(Some(&root), "GET", "/missing.pdf", None).status, 404);
    assert_eq!(serve(Some(&root), "GET", "/my%20docs", None).status, 404);
    assert_eq!(
        serve(Some(&root), "GET", "/..%2F..%2Fetc%2Fpasswd", None).status,
        403
    );
    let abs = outside.path().join("s.txt");
    let enc = percent_encoding::utf8_percent_encode(
        &abs.display().to_string(),
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string();
    assert_eq!(
        serve(Some(&root), "GET", &format!("/{enc}"), None).status,
        403
    );
    assert_eq!(serve(None, "GET", "/x", None).status, 404);
    assert_eq!(serve(Some(&root), "POST", "/page.html", None).status, 405);
    let head = serve(Some(&root), "HEAD", "/page.html", None);
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    assert_eq!(header(&head, "content-length"), Some("25"));
    assert_eq!(
        path_from_uri("/a%2Fb%20c.txt").as_deref(),
        Some("a/b c.txt")
    );
    assert_eq!(path_from_uri("/%FF"), None);
}

#[test]
fn serve_ranges() {
    let (_d, root) = project();
    write(&root, "f.pdf", b"0123456789");
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=2-4"));
    assert_eq!(r.status, 206);
    assert_eq!(r.body, b"234");
    assert_eq!(header(&r, "content-range"), Some("bytes 2-4/10"));
    assert_eq!(header(&r, "content-length"), Some("3"));
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=7-"));
    assert_eq!(r.body, b"789");
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=-3"));
    assert_eq!(r.body, b"789");
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=5-100"));
    assert_eq!(r.body, b"56789");
    assert_eq!(header(&r, "content-range"), Some("bytes 5-9/10"));
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=10-"));
    assert_eq!(r.status, 416);
    assert_eq!(header(&r, "content-range"), Some("bytes */10"));
    // Multi-range: the whole file.
    let r = serve(Some(&root), "GET", "/f.pdf", Some("bytes=0-1,4-5"));
    assert_eq!(r.status, 200);
    assert_eq!(r.body.len(), 10);
}

#[test]
fn watcher_debounces_and_filters() {
    let (_d, root) = project();
    write(&root, ".gitignore", b"*.log\n");
    fs::create_dir(root.join("node_modules")).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = watch::ProjectWatcher::start(&root, move |c| {
        let _ = tx.send(c);
    })
    .unwrap();
    assert_eq!(watcher.root(), root.as_path());
    // FSEvents may report events from just before the watch started; let them settle.
    std::thread::sleep(Duration::from_millis(500));
    while rx.try_recv().is_ok() {}

    for i in 0..5 {
        fs::write(root.join("a.txt"), format!("v{i}")).unwrap();
    }
    fs::write(root.join("b.md"), "b").unwrap();
    fs::write(root.join("node_modules/x.js"), "x").unwrap();
    fs::write(root.join("debug.log"), "l").unwrap();

    let mut paths: Vec<String> = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline
        && !(paths.contains(&"a.txt".to_string()) && paths.contains(&"b.md".to_string()))
    {
        if let Ok(c) = rx.recv_timeout(Duration::from_millis(200)) {
            assert_eq!(c.root, root.display().to_string());
            let mut unique = c.paths.clone();
            unique.dedup();
            assert_eq!(unique.len(), c.paths.len(), "deduped: {:?}", c.paths);
            paths.extend(c.paths);
        }
    }
    assert!(paths.contains(&"a.txt".to_string()), "{paths:?}");
    assert!(paths.contains(&"b.md".to_string()), "{paths:?}");
    // Drain anything late, then check the filtered paths never showed up.
    std::thread::sleep(Duration::from_millis(600));
    while let Ok(c) = rx.try_recv() {
        paths.extend(c.paths);
    }
    assert!(
        !paths.iter().any(|p| p.starts_with("node_modules")),
        "{paths:?}"
    );
    assert!(!paths.iter().any(|p| p.ends_with(".log")), "{paths:?}");
    // Five rapid writes to one file arrive as far fewer batches entries than writes.
    assert!(
        paths.iter().filter(|p| *p == "a.txt").count() < 5,
        "{paths:?}"
    );

    drop(watcher);
    std::thread::sleep(Duration::from_millis(400));
    while rx.try_recv().is_ok() {}
    fs::write(root.join("after.txt"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert!(rx.try_recv().is_err());
}
