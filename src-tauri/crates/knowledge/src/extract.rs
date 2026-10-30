//! Text extraction for knowledgebase sources: plain text and code, Markdown, HTML, PDF, DOCX,
//! XLSX (and other spreadsheets calamine reads) and PPTX.
//!
//! PDF and DOCX reuse the extractors behind the `readFiles` tool. Spreadsheets become one
//! Markdown section per sheet (rows as tab-separated lines) and slide decks one section per
//! slide, so chunks carry the sheet or slide in their heading path.

use crate::chunk::TextKind;
use crate::html::html_to_text;
use quick_xml::events::Event;
use std::io::{Cursor, Read};
use std::path::Path;

/// Biggest file accepted (before extraction).
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

/// Extracted text of one document.
#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub title: String,
    pub text: String,
    pub kind: TextKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Markdown,
    Code,
    Html,
    Pdf,
    Docx,
    Spreadsheet,
    Pptx,
}

const MARKDOWN: &[&str] = &["md", "markdown", "mdx", "rst", "adoc", "asciidoc", "org"];
const TEXT: &[&str] = &["txt", "text", "log", "csv", "tsv", "rtf", "tex"];
const CODE: &[&str] = &[
    "json",
    "jsonl",
    "yaml",
    "yml",
    "toml",
    "xml",
    "ini",
    "cfg",
    "conf",
    "env",
    "properties",
    "rs",
    "ts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "rb",
    "go",
    "java",
    "kt",
    "kts",
    "scala",
    "swift",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "fs",
    "php",
    "pl",
    "lua",
    "r",
    "sql",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "vue",
    "svelte",
    "css",
    "scss",
    "less",
    "graphql",
    "gql",
    "proto",
    "tf",
    "hcl",
    "dockerfile",
    "makefile",
    "gradle",
    "dart",
    "ex",
    "exs",
    "erl",
    "hs",
    "clj",
    "elm",
    "zig",
    "nim",
    "jl",
    "m",
    "mm",
    "groovy",
    "vb",
];
const SPREADSHEET: &[&str] = &["xlsx", "xlsm", "xlsb", "xls", "ods"];

/// The format for a file name, by extension (`Dockerfile` / `Makefile` by name).
pub fn format_for_path(path: &Path) -> Option<Format> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .or_else(|| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .filter(|n| n == "dockerfile" || n == "makefile")
        })?;
    let ext = ext.as_str();
    Some(match ext {
        "pdf" => Format::Pdf,
        "docx" => Format::Docx,
        "pptx" => Format::Pptx,
        "html" | "htm" | "xhtml" => Format::Html,
        _ if SPREADSHEET.contains(&ext) => Format::Spreadsheet,
        _ if MARKDOWN.contains(&ext) => Format::Markdown,
        _ if TEXT.contains(&ext) => Format::Text,
        _ if CODE.contains(&ext) => Format::Code,
        _ => return None,
    })
}

/// File extensions the file picker offers.
pub fn supported_extensions() -> Vec<&'static str> {
    let mut v = vec!["pdf", "docx", "pptx", "html", "htm", "xhtml"];
    v.extend_from_slice(SPREADSHEET);
    v.extend_from_slice(MARKDOWN);
    v.extend_from_slice(TEXT);
    v.extend_from_slice(CODE);
    v
}

/// Adding a folder with at least this many supported files asks for confirmation first.
pub const LARGE_FOLDER_FILES: usize = 500;

/// Directories never descended into when adding a folder (besides hidden ones).
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "__pycache__", "venv"];

/// Every supported file under `dir`, recursively and sorted. Hidden entries, dependency and
/// build directories, symlinks and files over [`MAX_FILE_BYTES`] are skipped.
pub fn supported_files_in(dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries =
            std::fs::read_dir(&d).map_err(|e| format!("Cannot read {}: {e}", d.display()))?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            // `DirEntry::file_type` does not follow symlinks, so symlinked folders can't loop.
            let Ok(ft) = entry.file_type() else { continue };
            let path = entry.path();
            if ft.is_dir() {
                if !SKIPPED_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if ft.is_file()
                && format_for_path(&path).is_some()
                && entry.metadata().is_ok_and(|m| m.len() <= MAX_FILE_BYTES)
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn file_title(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Read and extract a file.
pub fn extract_file(path: &Path) -> Result<Extracted, String> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            format!("File not found (moved or deleted?): {}", path.display())
        }
        _ => format!("Cannot read {}: {e}", path.display()),
    })?;
    if !meta.is_file() {
        return Err(format!("Not a file: {}", path.display()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "File is too large ({} MB, limit {} MB)",
            meta.len() / (1024 * 1024),
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    let format = format_for_path(path).ok_or_else(|| {
        format!(
            "Unsupported file type: {}",
            path.extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_title(path))
        )
    })?;
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let mut out = extract_bytes(&bytes, format, None)?;
    if out.title.is_empty() || format != Format::Html {
        out.title = file_title(path);
    }
    Ok(out)
}

fn utf8(bytes: &[u8]) -> Result<String, String> {
    let head = &bytes[..bytes.len().min(8192)];
    if head.contains(&0) {
        return Err("File looks binary, not text".into());
    }
    let s = String::from_utf8_lossy(bytes);
    Ok(s.strip_prefix('\u{feff}').unwrap_or(&s).to_string())
}

/// Extract text from bytes of a known format. `url` is the page address for HTML.
pub fn extract_bytes(bytes: &[u8], format: Format, url: Option<&str>) -> Result<Extracted, String> {
    let (text, kind, title) = match format {
        Format::Text => (utf8(bytes)?, TextKind::Prose, String::new()),
        Format::Markdown => (utf8(bytes)?, TextKind::Markdown, String::new()),
        Format::Code => (utf8(bytes)?, TextKind::Code, String::new()),
        Format::Html => {
            let page = html_to_text(&utf8(bytes)?, url);
            (page.text, TextKind::Markdown, page.title)
        }
        Format::Pdf => (
            tools::documents::cleanup_text(&tools::documents::pdf_raw_text(bytes)?),
            TextKind::Prose,
            String::new(),
        ),
        Format::Docx => (
            tools::documents::cleanup_text(&tools::documents::docx_raw_text(bytes)?),
            TextKind::Prose,
            String::new(),
        ),
        Format::Spreadsheet => (spreadsheet_text(bytes)?, TextKind::Markdown, String::new()),
        Format::Pptx => (pptx_text(bytes)?, TextKind::Markdown, String::new()),
    };
    Ok(Extracted { title, text, kind })
}

/// Every sheet as `## <sheet>` followed by its rows, cells tab-separated.
fn spreadsheet_text(bytes: &[u8]) -> Result<String, String> {
    use calamine::{open_workbook_auto_from_rs, Data, Reader};
    let mut wb = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("Cannot read spreadsheet: {e}"))?;
    let mut out = String::new();
    for name in wb.sheet_names().to_vec() {
        let Ok(range) = wb.worksheet_range(&name) else {
            continue;
        };
        let mut rows = String::new();
        for row in range.rows() {
            let cells: Vec<String> = row
                .iter()
                .map(|c| match c {
                    Data::Empty => String::new(),
                    Data::String(s) => s.replace(['\t', '\n', '\r'], " "),
                    Data::Float(f) => {
                        if f.fract() == 0.0 && f.abs() < 1e15 {
                            format!("{}", *f as i64)
                        } else {
                            f.to_string()
                        }
                    }
                    other => other.to_string(),
                })
                .collect();
            if cells.iter().all(String::is_empty) {
                continue;
            }
            rows.push_str(cells.join("\t").trim_end());
            rows.push('\n');
        }
        if !rows.is_empty() {
            out.push_str(&format!("## {name}\n{rows}\n"));
        }
    }
    Ok(out.trim().to_string())
}

/// Slides in order as `## Slide N`, each with its text runs (paragraphs on their own lines).
fn pptx_text(bytes: &[u8]) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Cannot read PowerPoint file: {e}"))?;
    let mut slides: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let num = n
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()?;
            Some((num, n.to_string()))
        })
        .collect();
    slides.sort();
    let mut out = String::new();
    for (num, name) in slides {
        let mut xml = String::new();
        zip.by_name(&name)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        let text = drawingml_text(&xml)?;
        if !text.trim().is_empty() {
            out.push_str(&format!("## Slide {num}\n{}\n\n", text.trim()));
        }
    }
    Ok(out.trim().to_string())
}

/// The `<a:t>` runs of a DrawingML part, a newline after each `<a:p>`.
fn drawingml_text(xml: &str) -> Result<String, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == "t" => in_t = true,
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "t" => in_t = false,
                "p" => out.push('\n'),
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => out.push_str(&t.xml10_content()),
            Ok(Event::CData(t)) if in_t => out.push_str(&t.xml10_content()),
            Ok(Event::GeneralRef(r)) if in_t => {
                let entity = format!("&{};", r.xml10_content());
                out.push_str(&quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Invalid slide XML: {e}")),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn folder_walk_keeps_supported_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (p, body) in [
            ("a.md", "a"),
            ("sub/b.pdf", "b"),
            ("sub/deep/c.txt", "c"),
            ("photo.png", "x"),
            (".hidden.md", "x"),
            (".git/config.md", "x"),
            ("node_modules/pkg/readme.md", "x"),
        ] {
            let path = root.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let found: Vec<String> = supported_files_in(root)
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(found, ["a.md", "sub/b.pdf", "sub/deep/c.txt"]);
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_for_path(Path::new("a.PDF")), Some(Format::Pdf));
        assert_eq!(format_for_path(Path::new("a.md")), Some(Format::Markdown));
        assert_eq!(format_for_path(Path::new("a.rs")), Some(Format::Code));
        assert_eq!(
            format_for_path(Path::new("x/Dockerfile")),
            Some(Format::Code)
        );
        assert_eq!(
            format_for_path(Path::new("a.xlsx")),
            Some(Format::Spreadsheet)
        );
        assert_eq!(format_for_path(Path::new("a.png")), None);
        assert_eq!(format_for_path(Path::new("noext")), None);
    }

    #[test]
    fn text_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("notes.md");
        std::fs::write(&p, "\u{feff}# Title\nbody").unwrap();
        let e = extract_file(&p).unwrap();
        assert_eq!(e.title, "notes.md");
        assert_eq!(e.text, "# Title\nbody");
        assert_eq!(e.kind, TextKind::Markdown);

        let bin = dir.path().join("x.txt");
        std::fs::write(&bin, b"ab\0cd").unwrap();
        assert!(extract_file(&bin).unwrap_err().contains("binary"));

        let missing = extract_file(&dir.path().join("gone.txt")).unwrap_err();
        assert!(missing.contains("moved or deleted"), "{missing}");

        let png = dir.path().join("x.png");
        std::fs::write(&png, b"x").unwrap();
        assert!(extract_file(&png).unwrap_err().contains("Unsupported"));
    }

    #[test]
    fn html_file_keeps_page_title() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("page.html");
        std::fs::write(
            &p,
            "<html><head><title>My Page</title></head><body><p>hello there</p></body></html>",
        )
        .unwrap();
        let e = extract_file(&p).unwrap();
        assert_eq!(e.title, "My Page");
        assert!(e.text.contains("hello there"));
    }

    fn zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (name, body) in files {
                w.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn pptx_slides_in_order() {
        let slide = |t: &str| {
            format!(
                r#"<p:sld xmlns:a="a" xmlns:p="p"><p:cSld><p:spTree><p:sp><p:txBody>
                <a:p><a:r><a:t>{t}</a:t></a:r></a:p><a:p><a:r><a:t>A &amp; B</a:t></a:r></a:p>
                </p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
            )
        };
        let s1 = slide("First");
        let s10 = slide("Tenth");
        let s2 = slide("Second");
        let bytes = zip_bytes(&[
            ("ppt/slides/slide10.xml", &s10),
            ("ppt/slides/slide1.xml", &s1),
            ("ppt/slides/slide2.xml", &s2),
            ("ppt/slides/_rels/slide1.xml.rels", "<x/>"),
        ]);
        let e = extract_bytes(&bytes, Format::Pptx, None).unwrap();
        assert_eq!(
            e.text,
            "## Slide 1\nFirst\nA & B\n\n## Slide 2\nSecond\nA & B\n\n## Slide 10\nTenth\nA & B"
        );
        assert_eq!(e.kind, TextKind::Markdown);
    }

    #[test]
    fn bad_spreadsheet_is_an_error() {
        assert!(extract_bytes(b"not a workbook", Format::Spreadsheet, None).is_err());
    }

    #[test]
    fn docx_via_tools() {
        let bytes = zip_bytes(&[(
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello docx</w:t></w:r></w:p></w:body></w:document>"#,
        )]);
        let e = extract_bytes(&bytes, Format::Docx, None).unwrap();
        assert_eq!(e.text, "Hello docx");
    }
}
