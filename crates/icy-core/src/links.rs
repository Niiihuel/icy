use std::path::{Path, PathBuf};

use crate::git::{DiffKind, DiffLine, EditCounts};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    File,
    Diff,
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
/// Image files icy can preview from any CLI's output (raster formats via resvg, plus SVG).
pub fn is_image_path(target: &str) -> bool {
    let ext = target.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
    )
}

pub fn detect(line: &str) -> Vec<LinkRef> {
    let mut found = Vec::new();
    let mut covered: Vec<(usize, usize)> = Vec::new();

    if let Some(edit) = detect_edit_summary(line) {
        covered.push((edit.start, edit.end));
        found.push(edit);
    }

    // 1. URLs and `file://` URIs first so `:port` or `file://` are not parsed as plain paths.
    detect_urls(line, &mut found, &mut covered);

    // 2. Quoted paths with optional `:line[:col]` or `(line[,col])`.
    detect_quoted_paths(line, &mut found, &mut covered);

    // 3. Unquoted whitespace-delimited tokens (handles `src/renderer.rs:726:5`,
    //    `C:\Users\dev\main.rs:12:3`, `Foo.cs(42,10)`, `--> crates/app/src/main.rs:10:5`).
    detect_unquoted_paths(line, &mut found, &mut covered);

    found.sort_by_key(|r| r.start);
    found
}

/// Converts an OSC 8 hyperlink URI (`file://...`, `http://...`, `https://...`) into a [`LinkRef`].
pub fn parse_hyperlink_uri(uri: &str, start: usize, end: usize) -> Option<LinkRef> {
    let uri = uri.trim();
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return Some(LinkRef {
            start,
            end,
            kind: RefKind::Url,
            target: uri.to_string(),
            line: None,
            col: None,
        });
    }
    if let Some((target, line, col)) = parse_file_uri(uri) {
        return Some(LinkRef {
            start,
            end,
            kind: RefKind::File,
            target,
            line,
            col,
        });
    }
    None
}

fn parse_file_uri(uri: &str) -> Option<(String, Option<u32>, Option<u32>)> {
    let rest = uri.strip_prefix("file://")?;
    let path_part = rest.strip_prefix("localhost").unwrap_or(rest);
    if path_part.is_empty() {
        return None;
    }
    let (before_frag, frag) = path_part
        .split_once('#')
        .map_or((path_part, None), |(p, f)| (p, Some(f)));
    let before_query = before_frag.split_once('?').map_or(before_frag, |(p, _)| p);
    let mut decoded = percent_decode(before_query);
    // `file:///C:/foo/bar.rs` -> `C:/foo/bar.rs` on Windows drive paths.
    if decoded.starts_with('/') && is_windows_drive_path(&decoded[1..]) {
        decoded.remove(0);
    }
    let (frag_line, frag_col) = frag.and_then(parse_uri_fragment).unwrap_or((None, None));
    let (path, loc_line, loc_col) = split_path_and_location(&decoded)?;
    if path.is_empty() {
        return None;
    }
    Some((
        path.to_string(),
        loc_line.or(frag_line),
        loc_col.or(frag_col),
    ))
}

fn parse_uri_fragment(frag: &str) -> Option<(Option<u32>, Option<u32>)> {
    // Supports `#L42`, `#L42-L50`, `#L42C5`, `#L42:5`, `#42`, `#42:5`.
    let first = frag.split('-').next().unwrap_or(frag);
    let trimmed = first.strip_prefix(['L', 'l']).unwrap_or(first);
    if let Some((l, c)) = trimmed.split_once([':', 'C', 'c']) {
        let line = l.parse::<u32>().ok()?;
        let col = c.parse::<u32>().ok();
        return Some((Some(line), col));
    }
    let line = trimmed.parse::<u32>().ok()?;
    Some((Some(line), None))
}

pub(crate) fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// Agent summaries include "└ Updated app/main.tsx +5 -1", "• Edited app/main.tsx
// (+5 -1)", and "● Update(app/main.tsx)". Ordinary mentions use the file viewer.
fn detect_edit_summary(line: &str) -> Option<LinkRef> {
    let line = line.trim_end();
    let trimmed = line.trim_start();
    let body = trim_cli_prefix(trimmed);
    let marked = body != trimmed;
    let (path, start) = if let Some((verb, path)) = body.split_once('(')
        && marked
        && matches!(verb, "Update" | "Write" | "Edit" | "MultiEdit")
    {
        (path.strip_suffix(')')?, line.len() - path.len())
    } else {
        let (verb, rest) = body.split_once(char::is_whitespace)?;
        if !matches!(
            verb,
            "Updated"
                | "Edited"
                | "Added"
                | "Created"
                | "Deleted"
                | "Removed"
                | "Modified"
                | "Edit"
                | "Write"
                | "Update"
                | "Patch"
        ) {
            return None;
        }
        if matches!(verb, "Edit" | "Write" | "Update" | "Patch") && !marked {
            return None;
        }
        let rest = rest.trim_start();
        let without_deletions = strip_line_count(rest, &['-', '−']);
        let without_additions = strip_line_count(without_deletions.unwrap_or(rest), &['+']);
        let path = match (without_additions, without_deletions) {
            (Some(path), Some(_)) => path,
            (Some(path), None) | (None, Some(path)) if marked => path,
            (None, None) if marked && (!rest.contains(char::is_whitespace) || is_quoted(rest)) => {
                rest
            }
            _ => return None,
        };
        (path, line.len() - rest.len())
    };

    let path = path.trim();
    let target = path.trim_matches(['`', '"', '\'']);
    let (target, location, col) = split_path_and_location(target)?;
    if !looks_like_file_path(target, location.is_some()) {
        return None;
    }
    let start = start + line[start..].find(path)?;
    Some(LinkRef {
        start,
        end: start + path.len(),
        kind: RefKind::Diff,
        target: target.into(),
        line: location,
        col,
    })
}

fn trailing_line_count<'a>(text: &'a str, signs: &[char]) -> Option<(&'a str, u32)> {
    let (prefix, count) = text.rsplit_once(char::is_whitespace)?;
    let digits = count.trim_matches(['(', ')']).strip_prefix(signs)?;
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((prefix.trim_end(), digits.parse().ok()?))
}

fn strip_line_count<'a>(text: &'a str, signs: &[char]) -> Option<&'a str> {
    trailing_line_count(text, signs).map(|(prefix, _)| prefix)
}

fn is_quoted(text: &str) -> bool {
    text.starts_with(['`', '"', '\'']) && text.ends_with(&text[..1])
}

fn trim_cli_prefix(line: &str) -> &str {
    line.trim_start_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '└' | '┌'
                    | '│'
                    | '├'
                    | '⎿'
                    | '•'
                    | '●'
                    | '∙'
                    | '←'
                    | '→'
                    | '┃'
                    | '▏'
                    | '▸'
                    | '▹'
                    | '◆'
                    | '◇'
                    | '✓'
                    | '✔'
                    | '✦'
                    | '✱'
            )
    })
}

fn is_tui_border(c: char) -> bool {
    matches!(
        c,
        '│' | '┃'
            | '▏'
            | '▕'
            | '┆'
            | '┇'
            | '┊'
            | '┋'
            | '╭'
            | '╮'
            | '╰'
            | '╯'
            | '┌'
            | '┐'
            | '└'
            | '┘'
            | '├'
            | '┤'
            | '┬'
            | '┴'
            | '┼'
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditPreview {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub lines: Vec<DiffLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<EditCounts>,
}

fn reported_edit_counts(row: &str) -> Option<EditCounts> {
    let body = trim_cli_prefix(row);
    let (label, rest) = body.split_once(' ')?;
    if !matches!(label, "Added" | "Removed") {
        return None;
    }
    let (number, unit) = rest.split_once(' ')?;
    let count = number.parse().ok()?;
    let (unit, remaining) = unit.split_once(", ").unwrap_or((unit, ""));
    if !matches!(unit, "line" | "lines") {
        return None;
    }
    let other_label = if label == "Added" {
        "removed "
    } else {
        "added "
    };
    let other = if let Some(rest) = remaining.strip_prefix(other_label) {
        let (number, unit) = rest.split_once(' ')?;
        if !matches!(unit, "line" | "lines") {
            return None;
        }
        number.parse().ok()?
    } else if remaining.is_empty() {
        0
    } else {
        return None;
    };
    Some(if label == "Added" {
        EditCounts {
            additions: count,
            deletions: other,
        }
    } else {
        EditCounts {
            additions: other,
            deletions: count,
        }
    })
}

/// Reads the historical patch printed under a CLI edit header, stopping at the next output block.
pub fn edit_preview(header: &str, rows: impl IntoIterator<Item = impl AsRef<str>>) -> EditPreview {
    let header = header.trim_end();
    let deleted = trailing_line_count(header, &['-', '−']);
    let added = trailing_line_count(deleted.map_or(header, |(rest, _)| rest), &['+']);
    let mut counts = (added.is_some() || deleted.is_some()).then(|| EditCounts {
        additions: added.map_or(0, |(_, count)| count),
        deletions: deleted.map_or(0, |(_, count)| count),
    });
    let mut lines = Vec::new();
    for row in rows {
        let row = trim_cli_prefix(row.as_ref().trim_end());
        if lines.is_empty()
            && let Some(reported) = reported_edit_counts(row)
        {
            counts = Some(reported);
            continue;
        }
        let digits = row.bytes().take_while(u8::is_ascii_digit).count();
        let (number, body) = row.split_at(digits);
        let Ok(number) = number.parse() else { break };
        if !body.is_empty() && !body.starts_with(char::is_whitespace) {
            break;
        }
        let marked = body.strip_prefix(' ').unwrap_or(body);
        let (kind, text) = if let Some(text) = marked.strip_prefix('+') {
            (DiffKind::Add, text)
        } else if let Some(text) = marked.strip_prefix(['-', '−']) {
            (DiffKind::Del, text)
        } else {
            (
                DiffKind::Ctx,
                body.strip_prefix("   ")
                    .unwrap_or_else(|| body.trim_start()),
            )
        };
        lines.push(DiffLine {
            kind,
            old_num: (kind != DiffKind::Add).then_some(number),
            new_num: (kind != DiffKind::Del).then_some(number),
            text: text.to_string(),
        });
    }
    if !lines
        .iter()
        .any(|line| matches!(line.kind, DiffKind::Add | DiffKind::Del))
    {
        lines.clear();
    }
    EditPreview { lines, counts }
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
        RefKind::File | RefKind::Diff => {
            let expanded = expand_tilde(&link.target);
            let candidate = Path::new(expanded.as_ref());
            let joined = if candidate.is_absolute() || is_windows_drive_path(expanded.as_ref()) {
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

fn expand_tilde(path: &str) -> std::borrow::Cow<'_, str> {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    if let (Some(rest), Some(home)) = (rest, crate::platform::home_dir()) {
        return std::borrow::Cow::Owned(home.join(rest).to_string_lossy().into_owned());
    }
    std::borrow::Cow::Borrowed(path)
}

fn overlaps(covered: &[(usize, usize)], start: usize, end: usize) -> bool {
    covered.iter().any(|&(s, e)| start < e && end > s)
}

fn detect_urls(line: &str, out: &mut Vec<LinkRef>, covered: &mut Vec<(usize, usize)>) {
    let mut search_from = 0;
    while search_from < line.len() {
        let rest = &line[search_from..];
        let next = [
            rest.find("https://"),
            rest.find("http://"),
            rest.find("file://"),
        ]
        .into_iter()
        .flatten()
        .min();
        let Some(rel) = next else { break };
        let start = search_from + rel;
        let tail = &line[start..];
        let raw_end = tail
            .find(|c: char| {
                c.is_whitespace()
                    || is_tui_border(c)
                    || matches!(c, '"' | '\'' | '<' | '>' | '`' | '*')
            })
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
            let raw = &line[start..end];
            if let Some(link) = parse_hyperlink_uri(raw, start, end) {
                covered.push((start, end));
                out.push(link);
            }
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
        if is_tui_border(ch) || matches!(ch, '(' | '[' | '{' | '<' | '"' | '\'' | '`' | '*' | '@') {
            start += ch.len_utf8();
        } else {
            break;
        }
    }

    while end > start {
        let ch = token[start..end].chars().next_back().unwrap();
        if is_tui_border(ch)
            || matches!(
                ch,
                ',' | ';' | '.' | '!' | '?' | '"' | '\'' | '`' | '*' | '>' | ']' | '}'
            )
        {
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

    is_known_filename(path)
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
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "svg"
    )
}

#[cfg(test)]
mod image_path_tests {
    use super::*;

    #[test]
    fn recognises_previewable_images() {
        assert!(is_image_path("/tmp/scratchpad/e1.png"));
        assert!(is_image_path("docs/Shot.JPEG"));
        assert!(is_image_path("icon.svg"));
        assert!(!is_image_path("src/main.rs"));
        assert!(!is_image_path("archive.png.zip"));
        assert!(!is_image_path("Makefile"));
    }
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
    fn recognises_edit_links_and_preserves_normal_mentions() {
        for (header, path, target_line) in [
            (
                "  └ Updated app/terminal.tsx +5 -1   ",
                "app/terminal.tsx",
                None,
            ),
            (
                "• Edited `app/my file.tsx` (+5 −1)",
                "app/my file.tsx",
                None,
            ),
            (
                "● Update(app/terminal.tsx:49)",
                "app/terminal.tsx",
                Some(49),
            ),
            ("• Deleted Makefile +0 -10", "Makefile", None),
        ] {
            let items = detect(header);
            assert_eq!(items.len(), 1, "{header}");
            assert_eq!(items[0].kind, RefKind::Diff, "{header}");
            assert_eq!(items[0].target, path);
            assert_eq!(items[0].line, target_line);
        }
        for mention in [
            "See app/terminal.tsx:49 for details.",
            "• Updated app/terminal.tsx to document selection.",
            "● Read(app/terminal.tsx)",
            "See Makefile.",
        ] {
            assert_eq!(detect(mention)[0].kind, RefKind::File, "{mention}");
        }
    }

    #[test]
    fn reads_the_numbered_preview_of_a_single_edit() {
        let edit = edit_preview(
            "● Update(file.rs)",
            [
                "  ⎿  Added 1 line, removed 1 line",
                "    48   +context",
                "    49 -old line",
                "    49 +// Selection",
                "● Update(other.rs)",
                "    3 +elsewhere",
            ],
        );
        assert_eq!(
            edit.counts,
            Some(EditCounts {
                additions: 1,
                deletions: 1
            })
        );
        assert_eq!(edit.lines.len(), 3);
        assert_eq!(edit.lines[0].kind, DiffKind::Ctx);
        assert_eq!(edit.lines[0].text, "+context");
        assert_eq!(edit.lines[1].old_num, Some(49));
        assert_eq!(edit.lines[1].new_num, None);
        assert_eq!(edit.lines[2].new_num, Some(49));
        assert_eq!(edit.lines[2].text, "// Selection");

        let summary = edit_preview("└ Updated file.rs +5 −2", ["", "    49 +elsewhere"]);
        assert!(summary.lines.is_empty());
        assert_eq!(
            summary.counts,
            Some(EditCounts {
                additions: 5,
                deletions: 2
            })
        );
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

    #[test]
    fn detects_cli_tui_formats_file_uris_images_and_tilde_paths() {
        // Bare image filenames, @mentions, TUI borders, markdown emphasis, and file:// URIs.
        let line = "│ see @app/sidebar.tsx:42, **screenshot.png**, file:///tmp/my%20pic.webp#L1, and https://example.com/a│";
        let refs = detect(line);
        assert_eq!(refs.len(), 4, "{refs:?}");
        assert_eq!(refs[0].kind, RefKind::File);
        assert_eq!(refs[0].target, "app/sidebar.tsx");
        assert_eq!(refs[0].line, Some(42));
        assert_eq!(refs[1].kind, RefKind::File);
        assert_eq!(refs[1].target, "screenshot.png");
        assert_eq!(refs[2].kind, RefKind::File);
        assert_eq!(refs[2].target, "/tmp/my pic.webp");
        assert_eq!(refs[2].line, Some(1));
        assert_eq!(refs[3].kind, RefKind::Url);
        assert_eq!(refs[3].target, "https://example.com/a");

        // OpenCode / Antigravity / Grok edit headers.
        for header in ["← Edit app/sidebar.tsx", "▸ Write src/lib.rs +4 -1"] {
            let items = detect(header);
            assert_eq!(items.len(), 1, "{header}");
            assert_eq!(items[0].kind, RefKind::Diff, "{header}");
        }

        // OSC 8 hyperlink URIs (file:// with #L line/col, Windows drive paths, and https://).
        let osc_file = parse_hyperlink_uri("file:///C:/Users/dev/main.rs#L18-L24", 0, 7).unwrap();
        assert_eq!(osc_file.kind, RefKind::File);
        assert_eq!(osc_file.target, "C:/Users/dev/main.rs");
        assert_eq!(osc_file.line, Some(18));

        // Tilde expansion in resolve().
        if let Some(home) = crate::platform::home_dir() {
            let tilde_ref = detect("open ~/demo/shot.png").pop().unwrap();
            let expected = home.join("demo/shot.png");
            let resolved = resolve(&tilde_ref, Path::new("/other"), |p| p == expected);
            assert_eq!(
                resolved,
                Some(ResolvedTarget::File {
                    path: expected,
                    line: None,
                    col: None,
                })
            );
        }
    }
}
