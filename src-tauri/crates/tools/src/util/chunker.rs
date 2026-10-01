//! Port of the parts of `src/preload/lib/contentChunker.ts` used by `BaseTool`.

use super::js;

const CHARS_PER_TOKEN: f64 = 4.0;

/// One chunk from [`split_content_by_token_limit`].
#[derive(Debug, Clone, PartialEq)]
pub struct ContentChunk {
    pub index: usize,
    pub total: usize,
    pub content: String,
    pub token_limit: f64,
}

/// `ContentChunker.estimateToken`: `ceil(length / 4)`.
pub fn estimate_token(content: &str) -> usize {
    (js::len(content) as f64 / CHARS_PER_TOKEN).ceil() as usize
}

/// `isContentTooLarge`.
pub fn is_content_too_large(content: &str, max_tokens: f64) -> bool {
    estimate_token(content) as f64 > max_tokens
}

/// `splitContentByTokenLimit`: chunks of `floor(maxTokens * 4 * 0.8)` UTF-16 units.
pub fn split_content_by_token_limit(content: &str, max_tokens: f64) -> Vec<ContentChunk> {
    let max_chars = (max_tokens * CHARS_PER_TOKEN * 0.8).floor() as usize;
    let len = js::len(content);
    if max_chars == 0 {
        return Vec::new();
    }
    let total = len.div_ceil(max_chars);
    (0..total)
        .map(|i| ContentChunk {
            index: i + 1,
            total,
            content: js::slice(content, i * max_chars, ((i + 1) * max_chars).min(len)).to_string(),
            token_limit: max_tokens,
        })
        .collect()
}

/// `BaseTool.processResultForModel` on an already-serialized result: `None` when the
/// content fits, else the first chunk plus the continuation notice.
pub fn truncate_for_model(serialized: &str, max_tokens: f64) -> Option<String> {
    if !is_content_too_large(serialized, max_tokens) {
        return None;
    }
    let chunks = split_content_by_token_limit(serialized, max_tokens);
    let first = chunks.first()?;
    let continuation = if chunks.len() > 1 {
        format!(
            "\n\n[Content truncated due to token limit. This is chunk {} of {}. The content was split to fit within the configured {} token limit.]",
            first.index,
            first.total,
            fmt_num(first.token_limit)
        )
    } else {
        String::new()
    };
    Some(format!("{}{}", first.content, continuation))
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate() {
        assert_eq!(estimate_token(""), 0);
        assert_eq!(estimate_token("abcde"), 2);
    }

    #[test]
    fn small_content_is_untouched() {
        assert_eq!(truncate_for_model("hello", 10.0), None);
    }

    #[test]
    fn large_content_keeps_first_chunk() {
        // maxTokens 10 -> 32 chars per chunk; 100 chars -> 4 chunks.
        let content = "x".repeat(100);
        let out = truncate_for_model(&content, 10.0).unwrap();
        assert!(out.starts_with(&"x".repeat(32)));
        assert!(!out.starts_with(&"x".repeat(33)));
        assert!(out.ends_with("[Content truncated due to token limit. This is chunk 1 of 4. The content was split to fit within the configured 10 token limit.]"));
    }
}
