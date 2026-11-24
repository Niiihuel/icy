//! Working-tree changes and unified diffs for the Review / Files panel.
//! Parsing is pure; only `changes` and `file_diff` shell out to `git`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// `M` modified, `A` added (staged), `U` untracked, `D` deleted, `R` renamed.
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    pub files: Vec<FileChange>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffKind {
    Hunk,
    Ctx,
    Add,
    Del,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffKind,
    pub old_num: Option<u32>,
    pub new_num: Option<u32>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditCounts {
    pub additions: u32,
    pub deletions: u32,
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    // `git diff --no-index` exits 1 when files differ; that is still a valid diff.
    if out.status.success() || out.status.code() == Some(1) {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub author: String,
    pub time: i64,
    pub summary: String,
    pub uncommitted: bool,
}

/// Parses one entry of `git blame --porcelain` (the header block before the content line).
pub fn parse_blame_porcelain(raw: &str) -> Option<BlameLine> {
    let mut lines = raw.lines();
    let sha = lines.next()?.split(' ').next()?;
    if sha.len() < 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (mut author, mut time, mut summary) = (None, None, String::new());
    for line in lines.take_while(|line| !line.starts_with('\t')) {
        if let Some(value) = line.strip_prefix("author ") {
            author = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("author-time ") {
            time = value.trim().parse().ok();
        } else if let Some(value) = line.strip_prefix("summary ") {
            summary = value.to_string();
        }
    }
    Some(BlameLine {
        author: author?,
        time: time?,
        summary,
        uncommitted: sha.bytes().all(|b| b == b'0'),
    })
}

/// Blame for one 1-based line of a file; `None` when untracked, outside a repo, or git fails.
pub fn blame_line(file: &Path, line: u32) -> Option<BlameLine> {
    let name = file.file_name()?.to_str()?;
    let range = format!("{line},{line}");
    let raw = git(
        file.parent()?,
        &["blame", "--porcelain", "-L", &range, "--", name],
    )?;
    parse_blame_porcelain(&raw)
}

/// Current branch of the repository containing `dir`.
pub fn branch(dir: &Path) -> Option<String> {
    let name = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Branch checked out when `dir` is a repository root, read from `.git/HEAD` without spawning
/// git so a folder listing can tag every repo. A detached HEAD yields its short hash.
pub fn head_branch(dir: &Path) -> Option<String> {
    let dot = dir.join(".git");
    let git_dir = if dot.is_file() {
        // Worktrees and submodules: `.git` is a file holding `gitdir: <path>`.
        dir.join(
            fs::read_to_string(&dot)
                .ok()?
                .strip_prefix("gitdir:")?
                .trim(),
        )
    } else {
        dot
    };
    parse_head(&fs::read_to_string(git_dir.join("HEAD")).ok()?)
}

fn parse_head(head: &str) -> Option<String> {
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(name) => {
            let name = name.trim();
            Some(name.strip_prefix("refs/heads/").unwrap_or(name).to_string())
        }
        None => head.get(..7).map(str::to_string),
    }
}

/// Parses `git status --porcelain=v1 -z` into `(path, status)` pairs.
pub fn parse_porcelain(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut parts = raw.split('\0').filter(|p| !p.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let (xy, path) = entry.split_at(3);
        let xy = xy.trim_end();
        let code = if xy.contains('?') {
            "U"
        } else if xy.contains('D') {
            "D"
        } else if xy.contains('R') {
            // Renames carry the original path as the next NUL-separated field.
            parts.next();
            "R"
        } else if xy.starts_with('A') {
            "A"
        } else {
            "M"
        };
        out.push((path.to_string(), code.to_string()));
    }
    out
}

/// Parses `git diff --numstat` lines (`adds\tdels\tpath`, `-` for binary files).
pub fn parse_numstat(raw: &str) -> HashMap<String, (u32, u32)> {
    raw.lines()
        .filter_map(|line| {
            let mut cols = line.splitn(3, '\t');
            let adds = cols.next()?.parse().unwrap_or(0);
            let dels = cols.next()?.parse().unwrap_or(0);
            let path = cols.next()?;
            // Renames are reported as `old => new`; keep the new path.
            let path = path
                .rsplit(" => ")
                .next()
                .unwrap_or(path)
                .trim_end_matches('}');
            Some((path.to_string(), (adds, dels)))
        })
        .collect()
}

/// Lists working-tree changes against `HEAD` with per-file line counts.
pub fn changes(root: &Path) -> ChangeSet {
    let Some(prefix) = git(root, &["rev-parse", "--show-prefix"]) else {
        return ChangeSet::default();
    };
    let prefix = prefix.trim_end_matches(['\r', '\n']);
    let Some(status) = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    ) else {
        return ChangeSet::default();
    };
    let numstat = git(
        root,
        &[
            "diff",
            "HEAD",
            "--no-ext-diff",
            "--no-textconv",
            "--numstat",
            "--relative",
            "--",
            ".",
        ],
    )
    .map(|raw| parse_numstat(&raw))
    .unwrap_or_default();

    let mut set = ChangeSet::default();
    for (repo_path, code) in parse_porcelain(&status) {
        let Some(path) = repo_path.strip_prefix(prefix) else {
            continue;
        };
        let (additions, deletions) = match numstat.get(path) {
            Some(&counts) => counts,
            None if code == "U" => (count_lines(&root.join(path)), 0),
            None => (0, 0),
        };
        set.additions += additions;
        set.deletions += deletions;
        set.files.push(FileChange {
            path: path.to_owned(),
            status: code,
            additions,
            deletions,
        });
    }
    set
}

fn count_lines(path: &Path) -> u32 {
    match fs::metadata(path) {
        // shortcut: skip line counting for files over 2 MiB, they show as +0.
        Ok(meta) if meta.is_file() && meta.len() <= 2 * 1024 * 1024 => fs::read(path)
            .map(|bytes| {
                // Binary files (NUL in the first 8 KiB, git's heuristic) count as +0 like numstat `-`.
                if bytes.iter().take(8192).any(|&b| b == 0) {
                    0
                } else {
                    bytes.iter().filter(|&&b| b == b'\n').count() as u32
                        + u32::from(bytes.last().is_some_and(|&b| b != b'\n'))
                }
            })
            .unwrap_or(0),
        _ => 0,
    }
}

/// Parses a unified diff into numbered lines; file headers before the first hunk are skipped.
pub fn parse_unified_diff(raw: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let (mut old, mut new) = (0u32, 0u32);
    let mut in_hunk = false;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("@@") {
            let mut nums = rest.split_whitespace();
            old = hunk_start(nums.next(), '-');
            new = hunk_start(nums.next(), '+');
            in_hunk = true;
            out.push(DiffLine {
                kind: DiffKind::Hunk,
                old_num: None,
                new_num: None,
                text: line.to_string(),
            });
            continue;
        }
        if !in_hunk {
            continue;
        }
        let (kind, text) = match line.chars().next() {
            Some('+') => (DiffKind::Add, &line[1..]),
            Some('-') => (DiffKind::Del, &line[1..]),
            Some(' ') => (DiffKind::Ctx, &line[1..]),
            Some('\\') => continue,
            None => (DiffKind::Ctx, ""),
            _ => {
                in_hunk = false;
                continue;
            }
        };
        let (old_num, new_num) = match kind {
            DiffKind::Add => {
                new += 1;
                (None, Some(new - 1))
            }
            DiffKind::Del => {
                old += 1;
                (Some(old - 1), None)
            }
            _ => {
                old += 1;
                new += 1;
                (Some(old - 1), Some(new - 1))
            }
        };
        out.push(DiffLine {
            kind,
            old_num,
            new_num,
            text: text.to_string(),
        });
    }
    out
}

fn hunk_start(field: Option<&str>, sign: char) -> u32 {
    field
        .and_then(|f| f.strip_prefix(sign))
        .and_then(|f| f.split(',').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

/// Unified diff of `rel_path` against `HEAD` with the whole file as context (the editor folds
/// unchanged runs itself, Antigravity-style); untracked files diff against an empty file.
pub fn file_diff(root: &Path, path: &str) -> Option<Vec<DiffLine>> {
    match git(
        root,
        &[
            "--literal-pathspecs",
            "diff",
            "HEAD",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "-U1000000",
            "--",
            path,
        ],
    ) {
        // Check the patch before the index so staged deletions still have a diff.
        Some(raw) if !raw.is_empty() => return Some(parse_unified_diff(&raw)),
        Some(_) => {
            if git(root, &["--literal-pathspecs", "ls-files", "--", path])
                .is_some_and(|out| !out.is_empty())
            {
                return Some(Vec::new());
            }
        }
        None => {
            let in_work_tree = git(root, &["rev-parse", "--is-inside-work-tree"])
                .is_some_and(|out| out.trim() == "true");
            if !in_work_tree || git(root, &["rev-parse", "--verify", "HEAD"]).is_some() {
                return None;
            }
        }
    }

    // New files have an empty base, including files in a repository without a first commit.
    let bytes = fs::read(root.join(path)).ok()?;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return Some(Vec::new());
    }
    Some(added_lines(&String::from_utf8_lossy(&bytes)))
}

/// Finds the changed row nearest the requested side and line, or the first change.
/// A change present only on the opposite side is located at the next line on this side.
pub fn diff_focus(lines: &[DiffLine], target: Option<u32>, old_side: bool) -> Option<usize> {
    if target.is_none() {
        return lines
            .iter()
            .position(|line| matches!(line.kind, DiffKind::Add | DiffKind::Del));
    }
    let target = target?;
    let mut next = 1;
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let num = if old_side { line.old_num } else { line.new_num };
            let position = num.unwrap_or(next);
            if num.is_some() {
                next = position.saturating_add(1);
            }
            matches!(line.kind, DiffKind::Add | DiffKind::Del).then_some((
                position.abs_diff(target),
                num.is_none(),
                index,
            ))
        })
        .min()
        .map(|(_, _, index)| index)
}

/// Resolves a summary without a printed patch only when a single change block matches its
/// reported size and any line hint. Keep adjacent context up to the next change.
pub fn edit_fragment(
    lines: &[DiffLine],
    counts: EditCounts,
    target: Option<u32>,
    old_side: bool,
) -> Option<&[DiffLine]> {
    const CONTEXT: usize = 3;
    let is_change = |line: &DiffLine| matches!(line.kind, DiffKind::Add | DiffKind::Del);
    let mut result = None;
    let mut end = 0;
    while let Some(start) = lines[end..].iter().position(is_change).map(|i| end + i) {
        end = start
            + lines[start..]
                .iter()
                .take_while(|line| is_change(line))
                .count();
        let block = &lines[start..end];
        let additions = block
            .iter()
            .filter(|line| line.kind == DiffKind::Add)
            .count();
        let deletions = block.len() - additions;
        if additions != counts.additions as usize
            || deletions != counts.deletions as usize
            || target.is_some_and(|number| {
                !block.iter().any(|line| {
                    (if old_side { line.old_num } else { line.new_num }) == Some(number)
                })
            })
        {
            continue;
        }
        if result.is_some() {
            return None;
        }
        let before = lines[..start]
            .iter()
            .rev()
            .take(CONTEXT)
            .take_while(|line| line.kind == DiffKind::Ctx)
            .count();
        let after = lines[end..]
            .iter()
            .take(CONTEXT)
            .take_while(|line| line.kind == DiffKind::Ctx)
            .count();
        result = Some(&lines[start - before..end + after]);
    }
    result
}

fn added_lines(text: &str) -> Vec<DiffLine> {
    text.lines()
        .enumerate()
        .map(|(index, text)| DiffLine {
            kind: DiffKind::Add,
            old_num: None,
            new_num: Some(index as u32 + 1),
            text: text.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_head_refs_and_detached_hashes() {
        assert_eq!(
            parse_head("ref: refs/heads/feat/tree\n").as_deref(),
            Some("feat/tree")
        );
        assert_eq!(parse_head("0123456789abcdef\n").as_deref(), Some("0123456"));
        assert_eq!(parse_head(""), None);
    }

    #[test]
    fn parses_blame_porcelain_entries() {
        let committed = "0123456789abcdef0123456789abcdef01234567 2 2 1\nauthor Ada\nauthor-mail <a@x>\nauthor-time 1700000000\nauthor-tz +0000\nsummary Fix it\nfilename f.rs\n\tlet x = 1;\n";
        let line = parse_blame_porcelain(committed).unwrap();
        assert_eq!(
            (line.author.as_str(), line.time, line.summary.as_str()),
            ("Ada", 1_700_000_000, "Fix it")
        );
        assert!(!line.uncommitted);
        let pending = "0000000000000000000000000000000000000000 1 1 1\nauthor Not Committed Yet\nauthor-time 1700000001\nsummary Version of f.rs\n\tx\n";
        assert!(parse_blame_porcelain(pending).unwrap().uncommitted);
        assert_eq!(parse_blame_porcelain("fatal: no such path"), None);
        assert_eq!(parse_blame_porcelain(""), None);
    }

    #[test]
    fn numbers_new_file_lines_without_a_null_device() {
        assert!(added_lines("").is_empty());
        for text in ["first\nsecond\n", "first\nsecond", "first\r\nsecond\r\n"] {
            let lines = added_lines(text);
            assert_eq!(lines.len(), 2);
            assert!(
                lines
                    .iter()
                    .all(|line| line.kind == DiffKind::Add && line.old_num.is_none())
            );
            assert_eq!(lines[0].new_num, Some(1));
            assert_eq!(lines[1].new_num, Some(2));
            assert_eq!(lines[1].text, "second");
        }
    }

    #[test]
    fn parses_porcelain_codes_and_renames() {
        let raw =
            " M app/ui.tsx\0?? new.rs\0A  staged.rs\0 D gone.rs\0R  new_name.rs\0old_name.rs\0";
        let parsed = parse_porcelain(raw);
        assert_eq!(
            parsed,
            vec![
                ("app/ui.tsx".into(), "M".into()),
                ("new.rs".into(), "U".into()),
                ("staged.rs".into(), "A".into()),
                ("gone.rs".into(), "D".into()),
                ("new_name.rs".into(), "R".into()),
            ]
        );
    }

    #[test]
    fn parses_numstat_with_binary_files() {
        let map = parse_numstat("12\t3\tsrc/a.rs\n-\t-\tlogo.png\n");
        assert_eq!(map["src/a.rs"], (12, 3));
        assert_eq!(map["logo.png"], (0, 0));
    }

    #[test]
    fn numbers_unified_diff_lines() {
        let raw = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -3,3 +3,4 @@ fn x\n ctx\n-old\n+new\n+more\n ctx2\n\\ No newline at end of file\n";
        let lines = parse_unified_diff(raw);
        let summary: Vec<_> = lines
            .iter()
            .map(|l| (l.kind, l.old_num, l.new_num, l.text.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (DiffKind::Hunk, None, None, "@@ -3,3 +3,4 @@ fn x"),
                (DiffKind::Ctx, Some(3), Some(3), "ctx"),
                (DiffKind::Del, Some(4), None, "old"),
                (DiffKind::Add, None, Some(4), "new"),
                (DiffKind::Add, None, Some(5), "more"),
                (DiffKind::Ctx, Some(5), Some(6), "ctx2"),
            ]
        );
        assert_eq!(diff_focus(&lines, None, false), Some(2));
        assert_eq!(diff_focus(&lines, Some(4), true), Some(2));
        assert_eq!(diff_focus(&lines, Some(4), false), Some(3));
        assert_eq!(diff_focus(&lines, Some(6), false), Some(4));
    }

    #[test]
    fn isolates_only_an_unambiguous_change_block() {
        let diff = parse_unified_diff(
            "@@ -1,8 +1,9 @@\n start\n-old\n+new\n between\n-old again\n+new again\n separator\n+unique\n end",
        );
        let replacement = EditCounts {
            additions: 1,
            deletions: 1,
        };
        assert!(edit_fragment(&diff, replacement, None, false).is_none());
        let anchored = edit_fragment(&diff, replacement, Some(4), true).unwrap();
        assert_eq!(
            anchored
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["between", "old again", "new again", "separator"]
        );
        let added = edit_fragment(
            &diff,
            EditCounts {
                additions: 1,
                deletions: 0,
            },
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            added
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["separator", "unique", "end"]
        );
        assert!(edit_fragment(&diff, replacement, Some(20), false).is_none());
    }
}
