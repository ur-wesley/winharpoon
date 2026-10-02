//! Repository gate mirroring `ESLint`'s `max-lines` rule: every source file must
//! stay at or under `MAX_LINES` lines.
//!
//! Counting mirrors the `ESLint` options: blank lines are skipped
//! (`skipBlankLines`), comments still count (`skipComments: false`) so that
//! comment-stuffing cannot evade the gate. Flip the consts below to change
//! the policy; the scanner handles both.
//!
//! OS-agnostic by construction: plain `std::fs`, runs wherever `cargo test`
//! runs (CI already runs it). Override the limit for a scratch check with
//! `MAX_LINES=... cargo test --test max_lines`.
//!
//! Scanned roots mirror Clippy's domain: `src/**/*.rs` plus `build.rs`.

use std::path::{Path, PathBuf};

const DEFAULT_MAX_LINES: usize = 600;
const SKIP_BLANK_LINES: bool = true;
const SKIP_COMMENTS: bool = false;

#[test]
fn max_lines() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("build.rs")];
    collect_rs_files(&root.join("src"), &mut files)
        .map_err(|err| format!("loc gate: scan failed: {err}"))?;
    files.sort();

    let max = std::env::var("MAX_LINES")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_LINES);

    let mut violations = Vec::new();
    for path in &files {
        let text =
            std::fs::read_to_string(path).map_err(|err| format!("loc gate: read failed: {err}"))?;
        let count = count_lines(&text);
        if count > max {
            let rel = path
                .strip_prefix(&root)
                .map_or_else(|_| path.clone(), Path::to_path_buf);
            violations.push(format!("{}:{count} exceeds {max}", rel.to_string_lossy()));
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "loc gate: {} file(s) exceed {max} lines:\n  {}",
            violations.len(),
            violations.join("\n  ")
        ))
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn count_lines(text: &str) -> usize {
    let mut counter = LineCounter::default();
    let mut count = 0_usize;
    for line in text.split('\n') {
        if counter.counts(line) {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Per-line classifier with block-comment state carried across lines.
#[derive(Default)]
struct LineCounter {
    in_block_comment: bool,
}

impl LineCounter {
    fn counts(&mut self, line: &str) -> bool {
        if SKIP_BLANK_LINES && line.trim().is_empty() {
            return false;
        }
        if SKIP_COMMENTS {
            self.has_code_outside_comments(line)
        } else {
            true
        }
    }

    /// True when the line holds code (or a string/char literal) outside of
    /// comments. String-aware so `//` inside `"http://…"` never starts a
    /// comment, and char literals / lifetimes never confuse the scanner.
    fn has_code_outside_comments(&mut self, line: &str) -> bool {
        let bytes = line.as_bytes();
        let mut i = 0_usize;
        let mut code_found = false;
        while i < bytes.len() {
            let current = bytes.get(i).copied().unwrap_or(0);
            let next = bytes.get(i.saturating_add(1)).copied().unwrap_or(0);
            if self.in_block_comment {
                if current == b'*' && next == b'/' {
                    self.in_block_comment = false;
                    i = i.saturating_add(2);
                } else {
                    i = i.saturating_add(1);
                }
            } else if current == b'/' && next == b'/' {
                break;
            } else if current == b'/' && next == b'*' {
                self.in_block_comment = true;
                i = i.saturating_add(2);
            } else if current == b'"' {
                code_found = true;
                i = skip_quoted(bytes, i.saturating_add(1), b'"');
            } else if current == b'\'' {
                code_found = true;
                i = skip_char_or_lifetime(bytes, i);
            } else if (current == b'r' || current == b'b')
                && (next == b'"' || next == b'#')
                && is_raw_prefix(bytes, i)
            {
                code_found = true;
                i = skip_raw_string(bytes, i);
            } else {
                if !current.is_ascii_whitespace() {
                    code_found = true;
                }
                i = i.saturating_add(1);
            }
        }
        code_found
    }
}

/// Skip a `"…"` string from just past its opening quote; honours `\` escapes.
fn skip_quoted(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    while i < bytes.len() {
        let current = bytes.get(i).copied().unwrap_or(0);
        i = i.saturating_add(1);
        if current == b'\\' {
            i = i.saturating_add(1);
        } else if current == quote {
            break;
        }
    }
    i
}

/// Skip a `'x'` char literal (or a `'lifetime`, which has no closing quote on
/// the same spot). Returns the index just past the literal.
fn skip_char_or_lifetime(bytes: &[u8], i: usize) -> usize {
    let second = bytes.get(i.saturating_add(2)).copied().unwrap_or(0);
    if second == b'\'' {
        i.saturating_add(3)
    } else if bytes.get(i.saturating_add(1)).copied().unwrap_or(0) == b'\\' {
        i.saturating_add(4)
    } else {
        i.saturating_add(1)
    }
}

/// True when `bytes[i]` starts a raw (byte-)string: `r"`, `br"`, `r#"`, ….
fn is_raw_prefix(bytes: &[u8], i: usize) -> bool {
    let mut j = i;
    if bytes.get(j).copied().unwrap_or(0) == b'b' {
        j = j.saturating_add(1);
    }
    if bytes.get(j).copied().unwrap_or(0) != b'r' {
        return false;
    }
    j = j.saturating_add(1);
    while bytes.get(j).copied().unwrap_or(0) == b'#' {
        j = j.saturating_add(1);
    }
    bytes.get(j).copied().unwrap_or(0) == b'"'
}

/// Skip a raw string from its leading `r`/`br`; honours `#` delimiters.
fn skip_raw_string(bytes: &[u8], i: usize) -> usize {
    let mut j = i.saturating_add(1);
    if bytes.get(i).copied().unwrap_or(0) == b'b' {
        j = j.saturating_add(1);
    }
    let mut hashes = 0_usize;
    while bytes.get(j).copied().unwrap_or(0) == b'#' {
        hashes = hashes.saturating_add(1);
        j = j.saturating_add(1);
    }
    j = j.saturating_add(1);
    while j < bytes.len() {
        if bytes.get(j).copied().unwrap_or(0) == b'"' {
            let mut k = j.saturating_add(1);
            let mut matched = 0_usize;
            while matched < hashes && bytes.get(k).copied().unwrap_or(0) == b'#' {
                matched = matched.saturating_add(1);
                k = k.saturating_add(1);
            }
            if matched == hashes {
                return k;
            }
        }
        j = j.saturating_add(1);
    }
    j
}
