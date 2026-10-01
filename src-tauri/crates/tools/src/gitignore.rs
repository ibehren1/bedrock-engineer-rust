//! Port of `src/preload/lib/gitignore-like-matcher.ts` (and its test).

use regex::Regex;

/// Gitignore-style path matcher used by `listFiles`.
#[derive(Debug, Clone)]
pub struct GitignoreLikeMatcher {
    patterns: Vec<(Regex, String, bool)>,
}

impl GitignoreLikeMatcher {
    pub fn new<S: AsRef<str>>(patterns: &[S]) -> Self {
        let patterns = patterns
            .iter()
            .filter_map(|p| {
                let p = p.as_ref();
                let (pattern, negation) = match p.strip_prefix('!') {
                    Some(rest) => (rest, true),
                    None => (p, false),
                };
                convert_pattern_to_regex(pattern).map(|re| (re, pattern.to_string(), negation))
            })
            .collect();
        GitignoreLikeMatcher { patterns }
    }

    /// `isIgnored(filePath)`.
    pub fn is_ignored(&self, file_path: &str) -> bool {
        let normalized = normalize(&file_path.replace('\\', "/"));
        let mut ignored = false;
        for (re, pattern, negation) in &self.patterns {
            let mut candidate = normalized.clone();
            if pattern.ends_with('/') && !candidate.ends_with('/') {
                candidate.push('/');
            }
            if re.is_match(&candidate) {
                if *negation {
                    if !self.is_parent_ignored(&normalized) {
                        ignored = false;
                    }
                } else {
                    ignored = true;
                }
            }
        }
        ignored
    }

    fn is_parent_ignored(&self, file_path: &str) -> bool {
        let parent = dirname(file_path);
        if parent == "." || parent == "/" {
            return false;
        }
        self.is_ignored(&parent)
    }
}

/// `escapeRegExp`: escapes `.*+?^${}()|[]\`.
fn escape_regexp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `convertGitignorePatternToRegExp`.
fn convert_pattern_to_regex(pattern: &str) -> Option<Regex> {
    let mut re = String::new();
    let body = if let Some(rest) = pattern.strip_prefix('/') {
        re.push('^');
        rest
    } else {
        re.push_str("(^|/)");
        pattern
    };
    for (i, part) in body.split('/').enumerate() {
        if i > 0 {
            re.push('/');
        }
        if part == "**" {
            re.push_str(".*");
        } else {
            let escaped = escape_regexp(part)
                .replace("\\*", "[^/]*")
                .replace("\\?", "[^/]");
            re.push_str(&escaped);
        }
    }
    if !body.ends_with('/') {
        re.push_str("($|/)");
    }
    Regex::new(&re).ok()
}

/// POSIX `path.normalize`.
pub(crate) fn normalize(p: &str) -> String {
    if p.is_empty() {
        return ".".to_string();
    }
    let absolute = p.starts_with('/');
    let trailing = p.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|l| *l != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            s => parts.push(s),
        }
    }
    let mut out = parts.join("/");
    if out.is_empty() && !absolute {
        out.push('.');
    }
    if trailing && !out.is_empty() {
        out.push('/');
    }
    if absolute {
        out.insert(0, '/');
    }
    out
}

/// POSIX `path.dirname`.
pub(crate) fn dirname(p: &str) -> String {
    if p.is_empty() {
        return ".".to_string();
    }
    let absolute = p.starts_with('/');
    let trimmed = p.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    match trimmed.rfind('/') {
        None => ".".to_string(),
        Some(idx) => {
            let d = trimmed[..idx].trim_end_matches('/');
            if d.is_empty() {
                if absolute {
                    "/".to_string()
                } else {
                    ".".to_string()
                }
            } else {
                d.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // gitignore-like-matcher.test.ts: 'gitignore-like-matcher'
    #[test]
    fn gitignore_like_matcher() {
        let matcher = GitignoreLikeMatcher::new(&["node_modules", "dist/**", "*.test.ts"]);
        assert!(matcher.is_ignored("node_modules/package.json"));
        assert!(matcher.is_ignored("dist/index.js"));
        assert!(matcher.is_ignored("src/index.test.ts"));
        assert!(!matcher.is_ignored("src/index.ts"));
    }

    // gitignore-like-matcher.test.ts: 'gitignore-like-matcher fullpath'
    #[test]
    fn gitignore_like_matcher_fullpath() {
        let matcher = GitignoreLikeMatcher::new(&["node_modules", "dist/**", "*.test.ts"]);
        assert!(matcher.is_ignored("/Users/user/work/dir/node_modules/package.json"));
        assert!(matcher.is_ignored("/Users/user/work/dir/dist/index.js"));
        assert!(matcher.is_ignored("/Users/user/work/dir/src/index.test.ts"));
        assert!(!matcher.is_ignored("/Users/user/work/dir/src/index.ts"));
    }

    #[test]
    fn negation_and_anchoring() {
        let m = GitignoreLikeMatcher::new(&["*.log", "!keep.log", "/build", "tmp/"]);
        assert!(m.is_ignored("a/debug.log"));
        assert!(!m.is_ignored("keep.log"));
        assert!(m.is_ignored("build/out.js"));
        assert!(!m.is_ignored("src/build"));
        assert!(m.is_ignored("tmp"));
        assert!(m.is_ignored("x/tmp/y"));
        // A negation cannot re-include a file whose parent is ignored.
        let m = GitignoreLikeMatcher::new(&["logs", "!logs/keep.txt"]);
        assert!(m.is_ignored("logs/keep.txt"));
        // `?` matches one non-slash character; regex metacharacters are literal.
        let m = GitignoreLikeMatcher::new(&["file?.txt", "a+b"]);
        assert!(m.is_ignored("file1.txt"));
        assert!(!m.is_ignored("file10.txt"));
        assert!(m.is_ignored("a+b"));
        assert!(!m.is_ignored("aab"));
    }

    #[test]
    fn path_helpers() {
        assert_eq!(normalize("./a//b/../c/"), "a/c/");
        assert_eq!(normalize("/a/./b"), "/a/b");
        assert_eq!(normalize(""), ".");
        assert_eq!(dirname("a/b/c"), "a/b");
        assert_eq!(dirname("a"), ".");
        assert_eq!(dirname("/a"), "/");
    }
}
