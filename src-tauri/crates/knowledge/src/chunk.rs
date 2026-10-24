//! Splitting extracted text into overlapping chunks for embedding.
//!
//! Sizes are approximate tokens ([`estimate_tokens`]): CJK characters count one each, other
//! text about four characters per token. That keeps chunks inside the 512-token input of the
//! small local model as well as the Bedrock models.
//!
//! Markdown is split at headings first (each chunk remembers its heading path), then text is
//! packed paragraph by paragraph; a paragraph that is too big is split by lines, then by
//! sentences, then hard. Code is split on blank lines and lines only.

/// How to read the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Prose,
    Markdown,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// `"Install › Linux"`; empty outside any heading.
    pub heading_path: String,
    pub text: String,
}

/// Separator between heading levels in [`Chunk::heading_path`].
pub const HEADING_SEP: &str = " › ";

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF   // Hiragana, Katakana
        | 0x3400..=0x4DBF // CJK Extension A
        | 0x4E00..=0x9FFF // CJK Unified
        | 0xAC00..=0xD7AF // Hangul
        | 0xF900..=0xFAFF // CJK Compatibility
        | 0xFF00..=0xFFEF // Full-width forms
        | 0x3000..=0x303F // CJK punctuation
    )
}

/// Approximate token count.
pub fn estimate_tokens(text: &str) -> usize {
    let mut cjk = 0usize;
    let mut other = 0usize;
    for c in text.chars() {
        if is_cjk(c) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    cjk + other.div_ceil(4)
}

/// Split `text` into chunks of at most about `size` tokens, each starting with about
/// `overlap` tokens from the end of the previous chunk in the same section.
pub fn chunk_text(text: &str, kind: TextKind, size: usize, overlap: usize) -> Vec<Chunk> {
    let size = size.max(1);
    let overlap = overlap.min(size / 2);
    let sections = match kind {
        TextKind::Markdown => markdown_sections(text),
        _ => vec![(String::new(), text.to_string())],
    };
    let mut out = Vec::new();
    for (heading_path, body) in sections {
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        // Pieces leave room for the overlap carried into the next chunk.
        let pieces = split_pieces(body, kind, (size - overlap).max(1));
        for text in pack(&pieces, size, overlap, kind) {
            let text = text.trim().to_string();
            if !text.is_empty() {
                out.push(Chunk {
                    heading_path: heading_path.clone(),
                    text,
                });
            }
        }
    }
    out
}

/// `(heading path, body)` per Markdown section. Headings inside fenced code are text.
fn markdown_sections(text: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut body = String::new();
    let mut in_fence = false;
    let path = |stack: &[(usize, String)]| {
        stack
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(HEADING_SEP)
    };
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }
        let heading = (!in_fence).then(|| parse_heading(trimmed)).flatten();
        match heading {
            Some((level, title)) => {
                sections.push((path(&stack), std::mem::take(&mut body)));
                while stack.last().is_some_and(|(l, _)| *l >= level) {
                    stack.pop();
                }
                stack.push((level, title));
            }
            None => {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    sections.push((path(&stack), body));
    sections
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_string()))
}

/// Pieces no bigger than `size` tokens, in order. Each keeps its trailing separator so packing
/// can concatenate them as-is.
fn split_pieces(text: &str, kind: TextKind, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in split_keep(text, "\n\n") {
        if estimate_tokens(&para) <= size {
            out.push(para);
            continue;
        }
        for line in split_keep(&para, "\n") {
            if estimate_tokens(&line) <= size {
                out.push(line);
                continue;
            }
            let sentences = if kind == TextKind::Code {
                vec![line]
            } else {
                split_sentences(&line)
            };
            for s in sentences {
                if estimate_tokens(&s) <= size {
                    out.push(s);
                } else {
                    out.extend(hard_split(&s, size));
                }
            }
        }
    }
    out
}

/// Split after each `sep`, keeping it on the left part.
fn split_keep(text: &str, sep: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(sep) {
        let mut end = i + sep.len();
        // Fold runs of the separator into one piece.
        while rest[end..].starts_with('\n') {
            end += 1;
        }
        out.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

/// Split after `.`, `!`, `?` followed by whitespace, and after `。`, `！`, `？`.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        let end = match c {
            '。' | '！' | '？' => true,
            '.' | '!' | '?' => chars.peek().is_some_and(|n| n.is_whitespace()),
            _ => false,
        };
        if end {
            while let Some(n) = chars.peek().copied().filter(|n| n.is_whitespace()) {
                cur.push(n);
                chars.next();
            }
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Cut into pieces of at most `size` tokens, preferring whitespace boundaries.
fn hard_split(text: &str, size: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut last_space: Option<usize> = None;
    for c in text.chars() {
        cur.push(c);
        if c.is_whitespace() {
            last_space = Some(cur.len());
        }
        if estimate_tokens(&cur) > size {
            let cut = last_space
                .filter(|i| *i > cur.len() / 2)
                .unwrap_or_else(|| {
                    // Cut before the char just pushed.
                    cur.len() - c.len_utf8()
                });
            let rest = cur.split_off(cut);
            out.push(std::mem::take(&mut cur));
            cur = rest;
            last_space = None;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Greedy packing with overlap. The overlap is the tail of the previous chunk, cut at a
/// whitespace boundary when there is one.
fn pack(pieces: &[String], size: usize, overlap: usize, kind: TextKind) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut cur_has_new = false;
    for piece in pieces {
        if cur_has_new && estimate_tokens(&cur) + estimate_tokens(piece) > size {
            let carry = tail(&cur, overlap, kind);
            chunks.push(std::mem::take(&mut cur));
            cur = carry;
            // The carried tail plus this piece may still be too big: drop the carry.
            if estimate_tokens(&cur) + estimate_tokens(piece) > size {
                cur.clear();
            }
        }
        cur.push_str(piece);
        cur_has_new = true;
    }
    if cur_has_new && !cur.trim().is_empty() {
        chunks.push(cur);
    }
    chunks
}

fn tail(text: &str, tokens: usize, kind: TextKind) -> String {
    if tokens == 0 {
        return String::new();
    }
    let mut start = text.len();
    for (i, _) in text.char_indices().rev() {
        if estimate_tokens(&text[i..]) > tokens {
            break;
        }
        start = i;
    }
    let mut out = &text[start..];
    if start > 0 {
        // Start at a line (code) or word boundary instead of mid-word.
        let boundary = if kind == TextKind::Code {
            out.find('\n')
        } else {
            out.find(char::is_whitespace)
        };
        if let Some(b) = boundary.filter(|b| *b < out.len() / 2) {
            out = &out[b..];
        }
    }
    let out = out.trim_start();
    if out.is_empty() {
        String::new()
    } else {
        let sep = if kind == TextKind::Code { "\n" } else { " " };
        format!("{out}{sep}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_estimate_counts_cjk_per_char() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("日本語"), 3);
        assert_eq!(estimate_tokens("日本語abcd"), 4);
    }

    #[test]
    fn small_text_is_one_chunk() {
        let c = chunk_text("Hello world.\n\nSecond para.", TextKind::Prose, 100, 10);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "Hello world.\n\nSecond para.");
        assert_eq!(c[0].heading_path, "");
    }

    #[test]
    fn empty_text_has_no_chunks() {
        assert!(chunk_text("  \n\n ", TextKind::Prose, 100, 10).is_empty());
    }

    #[test]
    fn markdown_heading_paths() {
        let md = "intro\n# A\none\n## B\ntwo\n```\n# not a heading\n```\n# C\nthree\n";
        let c = chunk_text(md, TextKind::Markdown, 100, 0);
        let got: Vec<(&str, &str)> = c
            .iter()
            .map(|c| (c.heading_path.as_str(), c.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("", "intro"),
                ("A", "one"),
                ("A › B", "two\n```\n# not a heading\n```"),
                ("C", "three"),
            ]
        );
    }

    #[test]
    fn chunks_respect_size_and_overlap() {
        let para = (0..200)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let c = chunk_text(&para, TextKind::Prose, 50, 10);
        assert!(c.len() > 3, "{}", c.len());
        for ch in &c {
            assert!(estimate_tokens(&ch.text) <= 50, "{}", ch.text);
        }
        // Each chunk after the first starts with words from the end of the previous one.
        for w in c.windows(2) {
            let first_word = w[1].text.split_whitespace().next().unwrap();
            assert!(w[0].text.contains(first_word), "{first_word}");
        }
        // Nothing lost: the last word is in the last chunk.
        assert!(c.last().unwrap().text.ends_with("word199"));
    }

    #[test]
    fn japanese_sentences_split() {
        let text = "これは日本語の文です。".repeat(30);
        let c = chunk_text(&text, TextKind::Prose, 40, 0);
        assert!(c.len() > 5);
        for ch in &c {
            assert!(estimate_tokens(&ch.text) <= 40);
            assert!(ch.text.ends_with('。'), "{}", ch.text);
        }
    }

    #[test]
    fn huge_word_is_hard_split() {
        let text = "x".repeat(1000);
        let c = chunk_text(&text, TextKind::Prose, 50, 0);
        assert_eq!(c.iter().map(|c| c.text.len()).sum::<usize>(), 1000);
        for ch in &c {
            assert!(estimate_tokens(&ch.text) <= 50);
        }
    }

    #[test]
    fn code_splits_on_lines() {
        let code = (0..100)
            .map(|i| format!("let v{i} = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = chunk_text(&code, TextKind::Code, 40, 8);
        assert!(c.len() > 2);
        for ch in &c {
            assert!(ch.text.starts_with("let "), "{}", ch.text);
        }
    }
}
