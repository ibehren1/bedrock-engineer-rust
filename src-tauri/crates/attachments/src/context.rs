//! Rebuild a chat's attachment context from disk (port of `attachmentContext.ts`).

use std::fs;
use std::path::Path;

use base64::Engine;
use serde_json::json;

use crate::file_naming::{utf16_len, utf16_prefix};
use crate::image_validation::{to_bedrock_image_format, validate_image_bytes};
use crate::manager::{list_attachments, AttachmentPaths};
use crate::types::*;

/// PDF / DOCX text extraction, supplied by the caller (the Electron build used `pdf-parse` and
/// `mammoth` through `extractPdfText` / `extractDocxText`). Errors are the message shown to the
/// model and reported in `skippedFiles`.
pub trait TextExtractor {
    fn extract_pdf_text(&self, path: &Path) -> Result<String, String>;
    fn extract_docx_text(&self, path: &Path) -> Result<String, String>;
}

/// Node-compatible `Math.round(n / d)` for non-negative integers (ties round up).
fn round_div(n: u64, d: u64) -> u64 {
    (2 * n + d) / (2 * d)
}

fn format_size(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", round_div(bytes, 1024))
    } else {
        // `toFixed(1)`: tenths rounded to nearest, ties toward the larger value.
        let tenths = round_div(bytes * 10, MB);
        format!("{}.{} MB", tenths / 10, tenths % 10)
    }
}

/// `Number.prototype.toLocaleString()` in en-US: thousands grouped with commas.
fn group_thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A text-extension file can still hold binary data (a `.txt` that is really gzip).
fn looks_binary(buffer: &[u8]) -> bool {
    buffer[..buffer.len().min(8192)].contains(&0)
}

fn read_extracted_text(
    file: &ChatAttachment,
    extractor: &dyn TextExtractor,
) -> Result<String, String> {
    let path = Path::new(&file.path);
    match file.kind {
        AttachmentKind::Pdf => extractor.extract_pdf_text(path),
        AttachmentKind::Docx => extractor.extract_docx_text(path),
        _ => {
            let buffer = fs::read(path).map_err(|e| e.to_string())?;
            if looks_binary(&buffer) {
                return Err("File contains binary data, so its text was not inlined.".into());
            }
            Ok(String::from_utf8_lossy(&buffer).into_owned())
        }
    }
}

/// Rebuild the attachment context from what is on disk right now. Called once per user send.
///
/// The first block is a `<chat_attachments>` text block listing every non-image file (inlined
/// within the per-file and total budgets); each valid image follows as a caption text block plus
/// an image block with base64 bytes.
pub fn build_attachment_context(
    paths: &AttachmentPaths,
    session_id: &str,
    extractor: &dyn TextExtractor,
) -> AttachmentContextResult {
    let AttachmentListing { directory, files } = list_attachments(paths, session_id);

    if files.is_empty() {
        return AttachmentContextResult {
            directory,
            blocks: vec![],
            file_count: 0,
            image_count: 0,
            total_text_chars: 0,
            truncated_files: vec![],
            skipped_files: vec![],
        };
    }

    let mut truncated_files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut total_text_chars = 0usize;

    for file in files.iter().filter(|f| f.kind != AttachmentKind::Image) {
        let attributes = format!(
            "name=\"{}\" path=\"{}\" size=\"{}\"",
            file.name,
            file.path,
            format_size(file.size)
        );

        if !file.extractable {
            lines.push(format!(
                "<file {attributes} content=\"not-extracted\" note=\"Not a text format. Inspect it with a tool if you need its contents.\"/>"
            ));
            continue;
        }

        if file.size == 0 {
            lines.push(format!("<file {attributes} content=\"empty\"/>"));
            continue;
        }

        if total_text_chars >= MAX_TEXT_CHARS_TOTAL {
            lines.push(format!(
                "<file {attributes} content=\"not-included\" note=\"The attachment text budget for this message was already full. Read it with readFiles.\"/>"
            ));
            truncated_files.push(file.name.clone());
            continue;
        }

        match read_extracted_text(file, extractor) {
            Ok(text) => {
                let budget = MAX_TEXT_CHARS_PER_FILE.min(MAX_TEXT_CHARS_TOTAL - total_text_chars);
                let (included, included_len) = utf16_prefix(&text, budget);
                total_text_chars += included_len;

                let text_len = utf16_len(&text);
                let suffix = if included_len < text_len {
                    truncated_files.push(file.name.clone());
                    format!(
                        "\n… [truncated: first {} of {} characters. Use readFiles with a line range for the rest.]",
                        group_thousands(included_len),
                        group_thousands(text_len)
                    )
                } else {
                    String::new()
                };

                lines.push(format!("<file {attributes}>\n{included}{suffix}\n</file>"));
            }
            Err(reason) => {
                lines.push(format!(
                    "<file {attributes} content=\"not-extracted\" note=\"{reason}\"/>"
                ));
                skipped_files.push(SkippedFile {
                    name: file.name.clone(),
                    reason,
                });
            }
        }
    }

    let mut header = vec![
        format!(
            "<chat_attachments folder=\"{directory}\" count=\"{}\">",
            files.len()
        ),
        "Files the user attached to this chat. They are real files on disk — re-read them at any"
            .to_string(),
        "time with readFiles; the folder path above is stable for this chat.".to_string(),
        String::new(),
    ];
    header.extend(lines);
    header.push("</chat_attachments>".to_string());

    let mut blocks = vec![json!({ "text": header.join("\n") })];

    // Images travel as their own blocks, re-validated because a file can be replaced on disk.
    let mut image_count = 0usize;
    for file in files.iter().filter(|f| f.kind == AttachmentKind::Image) {
        let Some(format) = to_bedrock_image_format(&file.name) else {
            skipped_files.push(SkippedFile {
                name: file.name.clone(),
                reason: "Unsupported image format".into(),
            });
            continue;
        };

        match fs::read(&file.path) {
            Ok(bytes) => {
                if let Some(problem) = validate_image_bytes(&file.name, &bytes, image_count) {
                    skipped_files.push(SkippedFile {
                        name: file.name.clone(),
                        reason: problem,
                    });
                    continue;
                }
                blocks.push(
                    json!({ "text": format!("Attached image: {} ({})", file.name, file.path) }),
                );
                blocks.push(json!({
                    "image": {
                        "format": format.as_str(),
                        "source": { "bytes": base64::engine::general_purpose::STANDARD.encode(&bytes) }
                    }
                }));
                image_count += 1;
            }
            Err(error) => skipped_files.push(SkippedFile {
                name: file.name.clone(),
                reason: error.to_string(),
            }),
        }
    }

    AttachmentContextResult {
        directory,
        blocks,
        file_count: files.len(),
        image_count,
        total_text_chars,
        truncated_files,
        skipped_files,
    }
}

#[cfg(test)]
mod tests {
    //! Port of `attachmentContext.test.ts`.
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::image_validation::tests::png_header;
    use crate::manager::tests::{Fixture, SESSION_ID};

    /// Stand-in for the mocked `extractPdfText` / `extractDocxText`.
    #[derive(Default)]
    struct MockExtractor {
        pdf: RefCell<Option<Result<String, String>>>,
        docx: RefCell<Option<Result<String, String>>>,
        pdf_calls: Cell<usize>,
        docx_calls: Cell<usize>,
    }

    impl TextExtractor for MockExtractor {
        fn extract_pdf_text(&self, _path: &Path) -> Result<String, String> {
            self.pdf_calls.set(self.pdf_calls.get() + 1);
            self.pdf
                .borrow()
                .clone()
                .unwrap_or_else(|| Ok(String::new()))
        }
        fn extract_docx_text(&self, _path: &Path) -> Result<String, String> {
            self.docx_calls.set(self.docx_calls.get() + 1);
            self.docx
                .borrow()
                .clone()
                .unwrap_or_else(|| Ok(String::new()))
        }
    }

    fn png() -> Vec<u8> {
        png_header(10, 10)
    }

    fn text_of(result: &AttachmentContextResult) -> String {
        result.blocks[0]["text"].as_str().unwrap().to_string()
    }

    fn build(fx: &Fixture, extractor: &MockExtractor) -> AttachmentContextResult {
        build_attachment_context(&fx.paths, SESSION_ID, extractor)
    }

    #[test]
    fn produces_nothing_for_a_chat_with_no_attachments() {
        let fx = Fixture::new();
        let result = build(&fx, &MockExtractor::default());
        assert!(result.blocks.is_empty());
        assert_eq!(result.file_count, 0);
    }

    #[test]
    fn inlines_a_text_file_with_its_name_and_absolute_path() {
        let fx = Fixture::new();
        let added = fx.add(SESSION_ID, &[("notes.txt", b"the quick brown fox")]);
        let result = build(&fx, &MockExtractor::default());

        assert!(text_of(&result).contains("the quick brown fox"));
        assert!(text_of(&result).contains(&added.added[0].path));
        assert_eq!(result.total_text_chars, "the quick brown fox".len());
    }

    #[test]
    fn extracts_pdf_and_docx_through_the_existing_extractors() {
        let fx = Fixture::new();
        let extractor = MockExtractor::default();
        *extractor.pdf.borrow_mut() = Some(Ok("pdf body".into()));
        *extractor.docx.borrow_mut() = Some(Ok("docx body".into()));
        fx.add(SESSION_ID, &[("spec.pdf", b"binary-ish")]);
        fx.add(SESSION_ID, &[("memo.docx", b"binary-ish")]);

        let result = build(&fx, &extractor);

        assert_eq!(extractor.pdf_calls.get(), 1);
        assert_eq!(extractor.docx_calls.get(), 1);
        assert!(text_of(&result).contains("pdf body"));
        assert!(text_of(&result).contains("docx body"));
    }

    #[test]
    fn truncates_a_file_past_the_per_file_cap_and_says_so() {
        let fx = Fixture::new();
        let huge = "x".repeat(MAX_TEXT_CHARS_PER_FILE + 500);
        fx.add(SESSION_ID, &[("huge.txt", huge.as_bytes())]);
        let result = build(&fx, &MockExtractor::default());

        assert_eq!(result.truncated_files, ["huge.txt"]);
        assert!(text_of(&result).contains("[truncated:"));
        assert!(text_of(&result).contains("first 120,000 of 120,500 characters"));
        assert_eq!(result.total_text_chars, MAX_TEXT_CHARS_PER_FILE);
    }

    #[test]
    fn stops_inlining_once_the_whole_folder_budget_is_spent() {
        let fx = Fixture::new();
        let per_file = "y".repeat(MAX_TEXT_CHARS_PER_FILE);
        let file_count = MAX_TEXT_CHARS_TOTAL / MAX_TEXT_CHARS_PER_FILE;
        for index in 0..file_count {
            fx.add(
                SESSION_ID,
                &[(&format!("file-{index}.txt"), per_file.as_bytes())],
            );
        }
        fx.add(SESSION_ID, &[("last.txt", b"never inlined")]);

        let result = build(&fx, &MockExtractor::default());

        assert_eq!(result.total_text_chars, MAX_TEXT_CHARS_TOTAL);
        assert!(result.truncated_files.contains(&"last.txt".to_string()));
        assert!(!text_of(&result).contains("never inlined"));
    }

    #[test]
    fn lists_a_spreadsheet_by_path_instead_of_injecting_its_bytes() {
        let fx = Fixture::new();
        fx.add(SESSION_ID, &[("data.xlsx", b"PK binary")]);
        let result = build(&fx, &MockExtractor::default());

        assert!(text_of(&result).contains("content=\"not-extracted\""));
        assert!(!text_of(&result).contains("binary"));
    }

    #[test]
    fn does_not_inline_a_binary_file_that_carries_a_text_extension() {
        let fx = Fixture::new();
        fx.add(
            SESSION_ID,
            &[("secretly-binary.txt", &[0x1f, 0x8b, 0x00, 0x41, 0x42])],
        );
        let result = build(&fx, &MockExtractor::default());

        assert!(text_of(&result).contains("content=\"not-extracted\""));
        let skipped: Vec<_> = result
            .skipped_files
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(skipped, ["secretly-binary.txt"]);
    }

    #[test]
    fn marks_an_empty_file_rather_than_emitting_an_empty_body() {
        let fx = Fixture::new();
        fx.add(SESSION_ID, &[("blank.txt", b"")]);
        let result = build(&fx, &MockExtractor::default());

        assert!(text_of(&result).contains("content=\"empty\""));
    }

    #[test]
    fn adds_an_image_block_with_the_format_bedrock_expects() {
        let fx = Fixture::new();
        fx.add(SESSION_ID, &[("photo.jpg", &png())]);
        let result = build(&fx, &MockExtractor::default());

        let image_block = result
            .blocks
            .iter()
            .find(|block| block.get("image").is_some())
            .unwrap();
        // .jpg has to be declared as jpeg, and base64 is accepted for the bytes.
        assert_eq!(image_block["image"]["format"], "jpeg");
        assert_eq!(
            image_block["image"]["source"]["bytes"],
            base64::engine::general_purpose::STANDARD.encode(png())
        );
        assert_eq!(result.image_count, 1);
    }

    #[test]
    fn skips_an_image_that_grew_past_the_limit_after_it_was_attached() {
        let fx = Fixture::new();
        let added = fx.add(SESSION_ID, &[("photo.png", &png())]);
        let mut grown = png();
        grown.extend(std::iter::repeat_n(0u8, 4 * 1024 * 1024));
        fs::write(&added.added[0].path, grown).unwrap();

        let result = build(&fx, &MockExtractor::default());

        assert_eq!(result.image_count, 0);
        assert_eq!(result.skipped_files[0].name, "photo.png");
        assert!(result.skipped_files[0].reason.contains("3.75 MB"));
    }

    #[test]
    fn reports_an_extraction_failure_instead_of_dropping_the_file_silently() {
        let fx = Fixture::new();
        let extractor = MockExtractor::default();
        *extractor.pdf.borrow_mut() = Some(Err("encrypted document".into()));
        fx.add(SESSION_ID, &[("locked.pdf", b"binary-ish")]);

        let result = build(&fx, &extractor);

        assert_eq!(
            result.skipped_files,
            [SkippedFile {
                name: "locked.pdf".into(),
                reason: "encrypted document".into()
            }]
        );
        assert!(text_of(&result).contains("encrypted document"));
    }

    #[test]
    fn size_and_number_formatting_match_node() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1536), "2 KB");
        assert_eq!(format_size(1024 * 1024 + 1024 * 1024 / 4), "1.3 MB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }
}
