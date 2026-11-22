use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    File,
    Url,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LinkRef {
    /// Start byte offset inside the scanned line.
    pub start: usize,
    /// End byte offset (exclusive) inside the scanned line.
    pub end: usize,
    pub kind: RefKind,
    /// File path string or URL string without line/column suffix.
    pub target: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedTarget {
    File {
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
    Url(String),
}

/// Scans a single terminal line for clickable file references (`path:line:col`, `path(line,col)`,
/// Windows drive paths, Unix paths, quoted paths) and `http(s)://` URLs.
pub fn detect(line: &str) -> Vec<LinkRef> {
    let mut found = Vec::new();
    let mut covered: Vec<(usize, usize)> = Vec::new();

    // 1. URLs first so `http://localhost:5173/foo` does not parse `:5173` as a line number.
    detect_urls(line, &mut found, &mut covered);

    // 2. Quoted paths with optional `:line[:col]` or `(line[,col])`.
    detect_quoted_paths(line, &mut found, &mut covered);

    // 3. Unquoted whitespace-delimited tokens (handles `src/renderer.rs:726:5`,
    //    `C:\Users\dev\main.rs:12:3`, `Foo.cs(42,10)`, `--> crates/app/src/main.rs:10:5`).
    detect_unquoted_paths(line, &mut found, &mut covered);

    found.sort_by_key(|r| r.start);
    found
}

/// Resolves a detected [`LinkRef`] against `cwd` using `exists` to verify file paths.
/// Strips leading `a/` or `b/` git diff prefixes if the verbatim relative path does not exist.
pub fn resolve(
    link: &LinkRef,
    cwd: &Path,
    mut exists: impl FnMut(&Path) -> bool,
) -> Option<ResolvedTarget> {
    match link.kind {
        RefKind::Url => Some(ResolvedTarget::Url(link.target.clone())),
        RefKind::File => {
            let candidate = Path::new(&link.target);
            let joined = if candidate.is_absolute() || is_windows_drive_path(&link.target) {
                candidate.to_path_buf()
            } else {
                cwd.join(candidate)
            };
            if exists(&joined) {
                return Some(ResolvedTarget::File {
                    path: joined,
                    line: link.line,
                    col: link.col,
                });
            }

            // Support `git diff` prefixes (`a/src/main.rs`, `b/src/main.rs`).
            if let Some(stripped) = link
                .target
                .strip_prefix("a/")
                .or_else(|| link.target.strip_prefix("b/"))
                .or_else(|| link.target.strip_prefix("a\\"))
                .or_else(|| link.target.strip_prefix("b\\"))
            {
                let alt = cwd.join(stripped);
                if exists(&alt) {
                    return Some(ResolvedTarget::File {
                        path: alt,
                        line: link.line,
                        col: link.col,
                    });
                }
            }
            None
        }
    }
}

fn overlaps(covered: &[(usize, usize)], start: usize, end: usize) -> bool {
    covered.iter().any(|&(s, e)| start < e && end > s)
}

fn detect_urls(line: &str, out: &mut Vec<LinkRef>, covered: &mut Vec<(usize, usize)>) {
    let mut search_from = 0;
    while search_from < line.len() {
        let rest = &line[search_from..];
        let next = [rest.find("https://"), rest.find("http://")]
            .into_iter()
            .flatten()
            .min();
        let Some(rel) = next else { break };
        let start = search_from + rel;
        let tail = &line[start..];
        let raw_end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`'))
            .map_or(line.len(), |idx| start + idx);

        let mut end = raw_end;
        while end > start {
            let ch = line[..end].chars().next_back().unwrap();
            if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?')
                || (ch == ')' && unmatched_closing(&line[start..end], '(', ')'))
                || (ch == ']' && unmatched_closing(&line[start..end], '[', ']'))
            {
                end -= ch.len_utf8();
            } else {
                break;
            }
        }

        if end > start + 7 && !overlaps(covered, start, end) {
            covered.push((start, end));
            out.push(LinkRef {
                start,
                end,
                kind: RefKind::Url,
                target: line[start..end].to_string(),
                line: None,
                col: None,
            });
        }
        search_from = raw_end.max(start + 1);
    }
}

fn unmatched_closing(s: &str, open: char, close: char) -> bool {
    let opens = s.chars().filter(|&c| c == open).count();
    let closes = s.chars().filter(|&c| c == close).count();
    closes > opens
}

fn detect_quoted_paths(line: &str, out: &mut Vec<LinkRef>, covered: &mut Vec<(usize, usize)>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let q = bytes[i];
        if q != b'"' && q != b'\'' {
            i += 1;
            continue;
        }
        let Some(close_rel) = line[i + 1..].find(q as char) else {
            i += 1;
            continue;
        };
        let inner_start = i + 1;
        let inner_end = inner_start + close_rel;
        let inner = &line[inner_start..inner_end];

        if !inner.is_empty() && !overlaps(covered, i, inner_end + 1) {
            // Case A: suffix inside quotes e.g. "src/foo bar.rs:12:3"
            if let Some((path, line_no, col_no)) = split_path_and_location(inner)
                && line_no.is_some()
                && looks_like_file_path(path, true)
            {
                covered.push((i, inner_end + 1));
                out.push(LinkRef {
                    start: inner_start,
                    end: inner_end,
                    kind: RefKind::File,
                    target: path.to_string(),
                    line: line_no,
                    col: col_no,
                });
                i = inner_end + 1;
                continue;
            }

            // Case B: `"src/foo bar.rs":12:3` or `"src/foo bar.rs", line 12`
            let after = &line[inner_end + 1..];
            if looks_like_file_path(inner, false) {
                let (loc_len, line_no, col_no) = parse_location_suffix_prefix(after);
                let span_end = inner_end + 1 + loc_len;
                covered.push((i, span_end));
                out.push(LinkRef {
                    start: inner_start,
                    end: if loc_len > 0 { span_end } else { inner_end },
                    kind: RefKind::File,
                    target: inner.to_string(),
                    line: line_no,
                    col: col_no,
                });
                i = span_end;
                continue;
            }
        }
        i = inner_end + 1;
    }
}

fn detect_unquoted_paths(line: &str, out: &mut Vec<LinkRef>, covered: &mut Vec<(usize, usize)>) {
    let mut i = 0;
    let len = line.len();
    while i < len {
        // Advance to next non-whitespace byte.
        let rest = &line[i..];
        let Some(first_ch) = rest.chars().next() else {
            break;
        };
        if first_ch.is_whitespace() {
            i += first_ch.len_utf8();
            continue;
        }
        let tok_end = rest
            .find(char::is_whitespace)
            .map_or(len, |offset| i + offset);
        let token = &line[i..tok_end];

        // Strip common leading/trailing wrappers (`(src/a.rs:1)`, `[src/a.rs]`, `src/a.rs,`).
        let (trim_start, trim_end) = trim_token_wrappers(token);
        let abs_start = i + trim_start;
        let abs_end = i + trim_end;
        if abs_end > abs_start && !overlaps(covered, abs_start, abs_end) {
            let candidate = &line[abs_start..abs_end];
            if let Some(mut link) = parse_unquoted_candidate(candidate, abs_start, abs_end) {
                // Handle Claude/tsc style `Update(path)` or `Read(path)` where wrapper is `fn(path)`.
                if link.target.contains('(') && !candidate.ends_with(')') {
                    // Nothing extra needed; trim_token_wrappers handled outer parens, but handle `Update(~/a/b.zsh)`.
                }
                if let Some(paren_rel) = candidate.find('(')
                    && candidate.ends_with(')')
                    && !looks_like_paren_location(&candidate[paren_rel..])
                {
                    let inner_start = abs_start + paren_rel + 1;
                    let inner_end = abs_end - 1;
                    if let Some(inner_link) = parse_unquoted_candidate(
                        &line[inner_start..inner_end],
                        inner_start,
                        inner_end,
                    ) {
                        link = inner_link;
                    }
                }
                covered.push((link.start, link.end));
                out.push(link);
            }
        }
        i = tok_end;
    }
}

fn parse_unquoted_candidate(candidate: &str, start: usize, end: usize) -> Option<LinkRef> {
    // Handle `fn(path/to/file.rs:12)` style like `Update(assets/shell/midterm.zsh)`.
    if let Some(open) = candidate.find('(')
        && candidate.ends_with(')')
        && !looks_like_paren_location(&candidate[open..])
    {
        let inner = &candidate[open + 1..candidate.len() - 1];
        if let Some((path, line_no, col_no)) = split_path_and_location(inner)
            && looks_like_file_path(path, line_no.is_some())
        {
            return Some(LinkRef {
                start: start + open + 1,
                end: end - 1,
                kind: RefKind::File,
                target: path.to_string(),
                line: line_no,
                col: col_no,
            });
        }
    }

    let (path, line_no, col_no) = split_path_and_location(candidate)?;
    if !looks_like_file_path(path, line_no.is_some()) {
        return None;
    }
    Some(LinkRef {
        start,
        end,
        kind: RefKind::File,
        target: path.to_string(),
        line: line_no,
        col: col_no,
    })
}

fn trim_token_wrappers(token: &str) -> (usize, usize) {
    let mut start = 0;
    let mut end = token.len();

    while start < end {
        let ch = token[start..end].chars().next().unwrap();
        if matches!(ch, '(' | '[' | '{' | '<' | '"' | '\'' | '`') {
            start += ch.len_utf8();
        } else {
            break;
        }
    }

    while end > start {
        let ch = token[start..end].chars().next_back().unwrap();
        if matches!(
            ch,
            ',' | ';' | '.' | '!' | '?' | '"' | '\'' | '`' | '>' | ']' | '}'
        ) {
            end -= ch.len_utf8();
        } else if (ch == ')' && unmatched_closing(&token[start..end], '(', ')')) || ch == ':' {
            end -= 1;
        } else {
            break;
        }
    }
    (start, end)
}

fn looks_like_paren_location(suffix: &str) -> bool {
    parse_paren_location(suffix).is_some()
}

fn parse_paren_location(suffix: &str) -> Option<(u32, Option<u32>)> {
    let inner = suffix.strip_prefix('(')?.strip_suffix(')')?;
    if let Some((l, c)) = inner.split_once(',') {
        let line = l.trim().parse::<u32>().ok()?;
        let col = c.trim().parse::<u32>().ok()?;
        Some((line, Some(col)))
    } else {
        let line = inner.trim().parse::<u32>().ok()?;
        Some((line, None))
    }
}

fn parse_location_suffix_prefix(after: &str) -> (usize, Option<u32>, Option<u32>) {
    if let Some(rest) = after.strip_prefix(':') {
        let digits1: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(line_no) = digits1.parse::<u32>() {
            let after_line = &rest[digits1.len()..];
            if let Some(rest2) = after_line.strip_prefix(':') {
                let digits2: String = rest2.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(col_no) = digits2.parse::<u32>() {
                    return (
                        1 + digits1.len() + 1 + digits2.len(),
                        Some(line_no),
                        Some(col_no),
                    );
                }
            }
            return (1 + digits1.len(), Some(line_no), None);
        }
    }
    if after.starts_with('(')
        && let Some(close) = after.find(')')
        && let Some((line_no, col_no)) = parse_paren_location(&after[..=close])
    {
        return (close + 1, Some(line_no), col_no);
    }
    (0, None, None)
}

fn split_path_and_location(raw: &str) -> Option<(&str, Option<u32>, Option<u32>)> {
    // Pattern 1: `path(line,col)` or `path(line)`
    if raw.ends_with(')')
        && let Some(open) = raw.rfind('(')
        && open > 0
        && let Some((line_no, col_no)) = parse_paren_location(&raw[open..])
    {
        return Some((&raw[..open], Some(line_no), col_no));
    }

    // Pattern 2: `path:line:col` or `path:line`
    // Careful with Windows drive letter `C:\...` where index 1 is `:`.
    let min_colon_pos = if is_windows_drive_path(raw) { 2 } else { 1 };
    if let Some(last_colon) = raw.rfind(':')
        && last_colon >= min_colon_pos
    {
        let last_part = &raw[last_colon + 1..];
        if !last_part.is_empty() && last_part.bytes().all(|b| b.is_ascii_digit()) {
            let last_num = last_part.parse::<u32>().ok()?;
            let prefix = &raw[..last_colon];
            if let Some(prev_colon) = prefix.rfind(':')
                && prev_colon >= min_colon_pos
            {
                let mid_part = &prefix[prev_colon + 1..];
                if !mid_part.is_empty() && mid_part.bytes().all(|b| b.is_ascii_digit()) {
                    let line_num = mid_part.parse::<u32>().ok()?;
                    return Some((&prefix[..prev_colon], Some(line_num), Some(last_num)));
                }
            }
            return Some((prefix, Some(last_num), None));
        }
    }

    Some((raw, None, None))
}

fn is_windows_drive_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn looks_like_file_path(path: &str, has_location: bool) -> bool {
    if path.is_empty() || path.len() > 512 {
        return false;
    }
    if path.contains("://") || path.starts_with('-') {
        return false;
    }
    if is_windows_drive_path(path) {
        return true;
    }

    let has_slash = path.contains('/') || path.contains('\\');
    if has_slash {
        // Avoid fractions like `10/20` or empty/trailing-only slashes like `/` or `a/b`.
        let file_name = path.rsplit(['/', '\\']).next().unwrap_or("");
        if file_name.is_empty() {
            return false;
        }
        if has_location {
            return true;
        }
        // Require either a leading path anchor (`./`, `../`, `/`, `~/`) or an extension/known basename.
        let anchored = path.starts_with('/')
            || path.starts_with("./")
            || path.starts_with("../")
            || path.starts_with("~/")
            || path.starts_with(".\\")
            || path.starts_with("..\\");
        return anchored || has_plausible_extension(file_name) || is_known_filename(file_name);
    }

    // Single-segment filename (no slashes): must have a valid file extension (or line number + extension)
    // and must not look like a pure version/number (`1.2.3`) or time (`0.15s`).
    if let Some((stem, ext)) = path.rsplit_once('.')
        && !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 10
        && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        && !ext.bytes().all(|b| b.is_ascii_digit())
    {
        if has_location {
            return true;
        }
        return matches_common_extension(ext);
    }

    false
}

fn has_plausible_extension(name: &str) -> bool {
    if let Some((stem, ext)) = name.rsplit_once('.') {
        !stem.is_empty()
            && !ext.is_empty()
            && ext.len() <= 10
            && ext.bytes().all(|b| b.is_ascii_alphanumeric())
            && !ext.bytes().all(|b| b.is_ascii_digit())
    } else {
        false
    }
}

fn is_known_filename(name: &str) -> bool {
    matches!(
        name,
        "Cargo.toml"
            | "Cargo.lock"
            | "Makefile"
            | "Dockerfile"
            | "README"
            | "LICENSE"
            | ".gitignore"
    )
}

fn matches_common_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "md"
            | "css"
            | "scss"
            | "html"
            | "py"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "sh"
            | "zsh"
            | "bash"
            | "ps1"
            | "txt"
            | "lock"
            | "wgsl"
            | "sql"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cargo_and_claude_paths() {
        let line = "   --> src/renderer.rs:726:5";
        let refs = detect(line);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "src/renderer.rs");
        assert_eq!(refs[0].line, Some(726));
        assert_eq!(refs[0].col, Some(5));
        assert_eq!(&line[refs[0].start..refs[0].end], "src/renderer.rs:726:5");

        let claude = "• Update(assets/shell/midterm.zsh)";
        let refs = detect(claude);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "assets/shell/midterm.zsh");
        assert_eq!(refs[0].line, None);
    }

    #[test]
    fn detects_windows_drive_and_paren_locations() {
        let line = r#"error in C:\Users\dev\icy\src\main.rs:42:10 and C:/app/ui.tsx(18,3)"#;
        let refs = detect(line);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].target, r"C:\Users\dev\icy\src\main.rs");
        assert_eq!(refs[0].line, Some(42));
        assert_eq!(refs[0].col, Some(10));
        assert_eq!(refs[1].target, "C:/app/ui.tsx");
        assert_eq!(refs[1].line, Some(18));
        assert_eq!(refs[1].col, Some(3));
    }

    #[test]
    fn detects_quoted_paths_and_urls() {
        let line = r#"preview at http://localhost:5173/app and "src/my file.tsx":19:2"#;
        let refs = detect(line);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].kind, RefKind::Url);
        assert_eq!(refs[0].target, "http://localhost:5173/app");
        assert_eq!(refs[1].kind, RefKind::File);
        assert_eq!(refs[1].target, "src/my file.tsx");
        assert_eq!(refs[1].line, Some(19));
        assert_eq!(refs[1].col, Some(2));
    }

    #[test]
    fn ignores_versions_and_resolves_git_diff_prefixes() {
        assert!(detect("rustc v1.99.1 finished in 0.15s").is_empty());

        let refs = detect("+++ b/crates/icy-core/src/links.rs");
        assert_eq!(refs.len(), 1);
        let cwd = Path::new("/repo");
        let resolved = resolve(&refs[0], cwd, |p| {
            p == Path::new("/repo/crates/icy-core/src/links.rs")
        });
        assert_eq!(
            resolved,
            Some(ResolvedTarget::File {
                path: PathBuf::from("/repo/crates/icy-core/src/links.rs"),
                line: None,
                col: None,
            })
        );
    }
}
