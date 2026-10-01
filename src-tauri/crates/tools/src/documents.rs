//! PDF / DOCX text extraction: port of `extractPdfText` (`src/main/handlers/pdf-handlers.ts`)
//! and `extractDocxText` (`src/main/handlers/docx-handlers.ts`).
//!
//! The TS used `pdf-parse` (pdf.js) and `mammoth.extractRawText`. Here PDF text comes from
//! `pdf-extract` and DOCX text is read straight out of `word/document.xml` the way mammoth's
//! raw-text mode renders it (each paragraph followed by a blank line, `w:tab` as a tab,
//! `w:br` / `w:cr` as a newline). Both then go through the same `cleanupText` and line-range
//! filter as the TS handlers, so errors and line-range behavior match; PDF glyph spacing can
//! differ from pdf.js on some files.
//!
//! The sync functions ([`extract_pdf_text`], [`extract_docx_text`]) are what the app can reuse
//! for `attachments::TextExtractor`; [`NativeDocumentReader`] is the async
//! [`DocumentReader`] the `readFiles` tool uses.

use crate::filesystem::DocumentReader;
use crate::util::line_range::{filter_by_line_range, LineRange};
use async_trait::async_trait;
use quick_xml::events::Event;
use regex::Regex;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

/// `cleanupText`: normalize line endings, collapse 3+ newlines to 2, strip trailing spaces
/// and tabs on each line, trim.
pub fn cleanup_text(text: &str) -> String {
    static MANY_NL: OnceLock<Regex> = OnceLock::new();
    static TRAILING_WS: OnceLock<Regex> = OnceLock::new();
    let s = text.replace("\r\n", "\n").replace('\r', "\n");
    let s = MANY_NL
        .get_or_init(|| Regex::new(r"\n{3,}").unwrap())
        .replace_all(&s, "\n\n");
    let s = TRAILING_WS
        .get_or_init(|| Regex::new(r"(?m)[ \t]+$").unwrap())
        .replace_all(&s, "");
    // JS `trim()` strips Unicode whitespace, as does `str::trim`.
    s.trim().to_string()
}

/// `path.extname(p).toLowerCase()`.
fn extname_lower(path: &str) -> String {
    let base = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match base.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) => base[i..].to_lowercase(),
    }
}

/// `isPdfFile`.
pub fn is_pdf_file(path: &str) -> bool {
    extname_lower(path) == ".pdf"
}

/// `isDocxFile`.
pub fn is_docx_file(path: &str) -> bool {
    extname_lower(path) == ".docx"
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path)
        .map_err(|e| crate::util::node_io::NodeIoError::new(&e, "open", path, None).message)
}

/// Text of a PDF held in memory (no cleanup).
pub fn pdf_raw_text(bytes: &[u8]) -> Result<String, String> {
    // pdf-extract panics on some malformed inputs; keep that from taking the caller down.
    match std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes)) {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Invalid PDF structure".to_string()),
    }
}

/// `extractPdfText(filePath, lineRange)`.
pub fn extract_pdf_text(path: &str, lines: Option<&LineRange>) -> Result<String, String> {
    let result = (|| {
        if !is_pdf_file(path) {
            return Err(format!("File {path} is not a PDF file"));
        }
        let bytes = read_file(path)?;
        let text = cleanup_text(&pdf_raw_text(&bytes)?);
        Ok(filter_by_line_range(&text, lines))
    })();
    result.map_err(|e| {
        tracing::error!(file_path = path, error = %e, "PDF text extraction failed");
        format!("Failed to extract text from PDF {path}: {e}")
    })
}

/// mammoth's raw text for a `word/document.xml` body.
pub fn docx_xml_raw_text(xml: &str) -> Result<String, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = false;
    // Inside w:del / w:instrText / w:delText nothing is rendered.
    let mut skip_depth = 0usize;
    loop {
        match reader.read_event() {
            Err(e) => return Err(e.to_string()),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "t" if skip_depth == 0 => in_text = true,
                "del" | "instrText" | "delText" => skip_depth += 1,
                "tab" if skip_depth == 0 => out.push('\t'),
                "br" | "cr" if skip_depth == 0 => out.push('\n'),
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.local_name().as_ref() {
                "tab" if skip_depth == 0 => out.push('\t'),
                "br" | "cr" if skip_depth == 0 => out.push('\n'),
                "p" if skip_depth == 0 => out.push_str("\n\n"),
                _ => {}
            },
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "t" => in_text = false,
                "del" | "instrText" | "delText" => skip_depth = skip_depth.saturating_sub(1),
                "p" if skip_depth == 0 => out.push_str("\n\n"),
                _ => {}
            },
            Ok(Event::Text(t)) if in_text && skip_depth == 0 => {
                out.push_str(&t.xml10_content());
            }
            Ok(Event::GeneralRef(r)) if in_text && skip_depth == 0 => {
                let entity = format!("&{};", r.xml10_content());
                out.push_str(&quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?);
            }
            Ok(Event::CData(t)) if in_text && skip_depth == 0 => {
                out.push_str(&t.xml10_content());
            }
            Ok(_) => {}
        }
    }
    Ok(out)
}

/// Raw text of a `.docx` held in memory (no cleanup).
pub fn docx_raw_text(bytes: &[u8]) -> Result<String, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| {
        "Could not find file in options: end of central directory record not found".to_string()
    })?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| {
            "Could not find main document part. Are you sure this is a valid .docx file?"
                .to_string()
        })?
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    docx_xml_raw_text(&xml)
}

/// `extractDocxText(filePath, lineRange)`.
pub fn extract_docx_text(path: &str, lines: Option<&LineRange>) -> Result<String, String> {
    let result = (|| {
        if !is_docx_file(path) {
            return Err(format!("File {path} is not a DOCX file"));
        }
        let bytes = read_file(path)?;
        let text = cleanup_text(&docx_raw_text(&bytes)?);
        Ok(filter_by_line_range(&text, lines))
    })();
    result.map_err(|e| {
        tracing::error!(file_path = path, error = %e, "DOCX text extraction failed");
        format!("Failed to extract text from DOCX {path}: {e}")
    })
}

/// [`DocumentReader`] over [`extract_pdf_text`] / [`extract_docx_text`], run on the blocking
/// pool.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDocumentReader;

async fn blocking(
    path: &str,
    lines: Option<LineRange>,
    f: fn(&str, Option<&LineRange>) -> Result<String, String>,
) -> Result<String, String> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || f(&path, lines.as_ref()))
        .await
        .map_err(|e| e.to_string())?
}

#[async_trait]
impl DocumentReader for NativeDocumentReader {
    async fn extract_pdf_text(
        &self,
        path: &str,
        lines: Option<LineRange>,
    ) -> Result<String, String> {
        blocking(path, lines, extract_pdf_text).await
    }

    async fn extract_docx_text(
        &self,
        path: &str,
        lines: Option<LineRange>,
    ) -> Result<String, String> {
        blocking(path, lines, extract_docx_text).await
    }
}

#[cfg(test)]
pub(crate) mod test_files {
    use std::io::Write;

    /// A one-page PDF drawing each line with Helvetica.
    pub fn pdf(lines: &[&str]) -> Vec<u8> {
        let mut content = String::from("BT /F1 12 Tf 72 720 Td 14 TL\n");
        for l in lines {
            content.push_str(&format!("({l}) Tj T*\n"));
        }
        content.push_str("ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_string(),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    /// A minimal `.docx` whose body is `body_xml`.
    pub fn docx(body_xml: &str) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            z.start_file("[Content_Types].xml", opts).unwrap();
            z.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#).unwrap();
            z.start_file("word/document.xml", opts).unwrap();
            z.write_all(
                format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body_xml}</w:body></w:document>"#)
                    .as_bytes(),
            )
            .unwrap();
            z.finish().unwrap();
        }
        buf.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_matches_ts() {
        assert_eq!(
            cleanup_text("  a  \r\nb\t\r\r\n\n\nc \n\n\n\n d"),
            "a\nb\n\nc\n\n d"
        );
    }

    #[test]
    fn extension_checks() {
        assert!(is_pdf_file("/x/A.PDF"));
        assert!(!is_pdf_file("/x/.pdf"));
        assert!(is_docx_file("r.docx"));
        assert!(!is_docx_file("r.doc"));
    }

    #[test]
    fn docx_paragraphs_tabs_breaks_and_entities() {
        let body = r#"<w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:tab/><w:t xml:space="preserve">world &amp; co</w:t></w:r></w:p><w:p/><w:p><w:r><w:t>line</w:t><w:br/><w:t>two</w:t></w:r><w:del><w:r><w:delText>gone</w:delText></w:r></w:del></w:p>"#;
        assert_eq!(
            docx_xml_raw_text(&format!("<w:body xmlns:w=\"x\">{body}</w:body>")).unwrap(),
            "Hello\tworld & co\n\n\n\nline\ntwo\n\n"
        );
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("doc.docx");
        std::fs::write(&p, test_files::docx(body)).unwrap();
        let p = p.to_string_lossy().into_owned();
        assert_eq!(
            extract_docx_text(&p, None).unwrap(),
            "Hello\tworld & co\n\nline\ntwo"
        );
        let range = LineRange {
            from: Some(3.0),
            to: Some(3.0),
        };
        assert_eq!(extract_docx_text(&p, Some(&range)).unwrap(), "line");
    }

    #[test]
    fn docx_errors() {
        assert_eq!(
            extract_docx_text("/x/file.txt", None).unwrap_err(),
            "Failed to extract text from DOCX /x/file.txt: File /x/file.txt is not a DOCX file"
        );
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.docx");
        std::fs::write(&p, b"not a zip").unwrap();
        let p = p.to_string_lossy().into_owned();
        assert!(extract_docx_text(&p, None)
            .unwrap_err()
            .starts_with(&format!("Failed to extract text from DOCX {p}: ")));
        let missing = dir.path().join("missing.docx");
        let missing = missing.to_string_lossy().into_owned();
        assert_eq!(
            extract_docx_text(&missing, None).unwrap_err(),
            format!("Failed to extract text from DOCX {missing}: ENOENT: no such file or directory, open '{missing}'")
        );
    }

    #[test]
    fn pdf_text_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("doc.pdf");
        std::fs::write(&p, test_files::pdf(&["Hello PDF", "Second line"])).unwrap();
        let p = p.to_string_lossy().into_owned();
        let text = extract_pdf_text(&p, None).unwrap();
        assert!(text.contains("Hello PDF"), "{text:?}");
        assert!(text.contains("Second line"), "{text:?}");
        assert_eq!(
            extract_pdf_text("/x/a.txt", None).unwrap_err(),
            "Failed to extract text from PDF /x/a.txt: File /x/a.txt is not a PDF file"
        );
        let bad = dir.path().join("bad.pdf");
        std::fs::write(&bad, b"garbage").unwrap();
        let bad = bad.to_string_lossy().into_owned();
        assert!(extract_pdf_text(&bad, None)
            .unwrap_err()
            .starts_with(&format!("Failed to extract text from PDF {bad}: ")));
    }

    #[tokio::test]
    async fn native_reader_applies_line_range() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.docx");
        std::fs::write(
            &p,
            test_files::docx(
                "<w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p>",
            ),
        )
        .unwrap();
        let text = NativeDocumentReader
            .extract_docx_text(
                &p.to_string_lossy(),
                Some(LineRange {
                    from: Some(3.0),
                    to: None,
                }),
            )
            .await
            .unwrap();
        assert_eq!(text, "b");
    }
}
