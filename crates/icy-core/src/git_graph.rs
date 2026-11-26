//! Commit graph for the Git panel: `git log` parsing and lane layout.
//!
//! The lane layout is ported from herdr-max's `mods/git-graph/graph.js`
//! (https://github.com/maxjkfc/herdr-max, Apache-2.0).

use std::path::Path;

use serde::Serialize;

/// How many commits the graph loads (newest first).
pub const LOG_LIMIT: usize = 500;

/// `git log` fields split by unit separators; each record ends with a record separator.
const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%at%x1f%D%x1f%s%x1e";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitRefKind {
    Head,
    Branch,
    Remote,
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GitRef {
    pub kind: GitRefKind,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    /// Author time, unix seconds.
    pub time: i64,
    pub refs: Vec<GitRef>,
    pub subject: String,
}

/// A line piece inside one row's cell. `x` is in lanes; `y` is in half rows:
/// 0 = top edge, 1 = the commit dot, 2 = bottom edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seg {
    pub x1: usize,
    pub y1: u8,
    pub x2: usize,
    pub y2: u8,
    /// Lane identity; the renderer maps it onto the theme's lane palette.
    pub color: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub commit: Commit,
    pub node: usize,
    pub color: usize,
    pub segs: Vec<Seg>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub rows: Vec<Row>,
    /// Lanes the widest row needs.
    pub width: usize,
}

#[derive(Clone, Copy)]
struct Lane<'a> {
    hash: &'a str,
    color: usize,
}

/// Lays out commits given in topological order (every child before its parents, as
/// `git log --date-order` prints them). A lane is a column waiting for one commit hash.
pub fn layout(commits: Vec<Commit>) -> Graph {
    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color = 0;
    let mut width = 1;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in &commits {
        let before = &lanes;
        let (node, node_color) = match before
            .iter()
            .position(|l| l.is_some_and(|l| l.hash == commit.hash))
        {
            Some(i) => (i, before[i].unwrap().color),
            None => {
                let free = before
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(before.len());
                next_color += 1;
                (free, next_color - 1)
            }
        };

        let mut segs = Vec::new();
        // Above the dot: lanes passing through, and lanes ending at this commit.
        for (i, lane) in before.iter().enumerate() {
            let Some(lane) = lane else { continue };
            let (x2, y2) = if lane.hash == commit.hash {
                (node, 1)
            } else {
                (i, 2)
            };
            segs.push(Seg {
                x1: i,
                y1: 0,
                x2,
                y2,
                color: lane.color,
            });
        }

        let mut after: Vec<Option<Lane>> = before
            .iter()
            .map(|l| l.filter(|l| l.hash != commit.hash))
            .collect();
        // Below the dot: one line to each parent's lane.
        for (k, parent) in commit.parents.iter().enumerate() {
            if let Some(existing) = after
                .iter()
                .position(|l| l.is_some_and(|l| l.hash == parent.as_str()))
            {
                let color = after[existing].unwrap().color;
                segs.push(Seg {
                    x1: node,
                    y1: 1,
                    x2: existing,
                    y2: 2,
                    color,
                });
                continue;
            }
            let lane = if k == 0 {
                node
            } else {
                after
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(after.len())
            };
            let color = if k == 0 {
                node_color
            } else {
                next_color += 1;
                next_color - 1
            };
            if lane >= after.len() {
                after.resize(lane + 1, None);
            }
            after[lane] = Some(Lane {
                hash: parent,
                color,
            });
            segs.push(Seg {
                x1: node,
                y1: 1,
                x2: lane,
                y2: 2,
                color,
            });
        }
        while after.last().is_some_and(Option::is_none) {
            after.pop();
        }

        width = width.max(node + 1).max(before.len()).max(after.len());
        segs.sort_unstable();
        segs.dedup();
        rows.push((node, node_color, segs));
        lanes = after;
    }

    Graph {
        rows: commits
            .into_iter()
            .zip(rows)
            .map(|(commit, (node, color, segs))| Row {
                commit,
                node,
                color,
                segs,
            })
            .collect(),
        width,
    }
}

pub fn parse_log(raw: &str) -> Vec<Commit> {
    raw.split('\x1e')
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            let mut f = record.split('\x1f');
            let hash = f.next().filter(|h| !h.is_empty())?.to_string();
            let parents = f
                .next()
                .unwrap_or("")
                .split_whitespace()
                .map(String::from)
                .collect();
            let author = f.next().unwrap_or("").to_string();
            let time = f.next().and_then(|t| t.parse().ok()).unwrap_or(0);
            let refs = parse_refs(f.next().unwrap_or(""));
            let subject = f.next().unwrap_or("").to_string();
            Some(Commit {
                hash,
                parents,
                author,
                time,
                refs,
                subject,
            })
        })
        .collect()
}

/// `HEAD -> main, origin/main, tag: v1` → typed refs (`origin/HEAD` is dropped).
pub fn parse_refs(decoration: &str) -> Vec<GitRef> {
    decoration
        .split(", ")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.ends_with("/HEAD"))
        .map(|p| {
            let (kind, name) = if p == "HEAD" {
                (GitRefKind::Head, p)
            } else if let Some(name) = p.strip_prefix("HEAD -> ") {
                (GitRefKind::Head, name)
            } else if let Some(name) = p.strip_prefix("tag: ") {
                (GitRefKind::Tag, name)
            } else if p.contains('/') {
                // shortcut: a local branch named `feat/x` reads as remote; fine for badges.
                (GitRefKind::Remote, p)
            } else {
                (GitRefKind::Branch, p)
            };
            GitRef {
                kind,
                name: name.to_string(),
            }
        })
        .collect()
}

/// The newest commits of every branch, or `None` outside a git repository.
pub fn log(root: &Path, limit: usize) -> Option<Vec<Commit>> {
    let limit = limit.to_string();
    let format = format!("--format={LOG_FORMAT}");
    let raw = crate::git::git(
        root,
        &[
            "--no-optional-locks",
            "log",
            "--all",
            "--date-order",
            "-n",
            &limit,
            "--decorate=short",
            &format,
        ],
    )?;
    Some(parse_log(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, parents: &[&str]) -> Commit {
        Commit {
            hash: hash.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author: String::new(),
            time: 0,
            refs: Vec::new(),
            subject: String::new(),
        }
    }

    #[test]
    fn linear_history_stays_in_one_lane() {
        let g = layout(vec![
            commit("c", &["b"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        assert_eq!(g.width, 1);
        assert!(g.rows.iter().all(|r| r.node == 0 && r.color == 0));
        assert_eq!(
            g.rows[1].segs,
            vec![
                Seg {
                    x1: 0,
                    y1: 0,
                    x2: 0,
                    y2: 1,
                    color: 0
                },
                Seg {
                    x1: 0,
                    y1: 1,
                    x2: 0,
                    y2: 2,
                    color: 0
                },
            ]
        );
        assert!(
            g.rows[2].segs.iter().all(|s| s.y2 <= 1),
            "root has no line below"
        );
    }

    #[test]
    fn merge_opens_a_lane_and_closes_it_at_the_fork() {
        // m merges f into main; f and b both come from a.
        let g = layout(vec![
            commit("m", &["b", "f"]),
            commit("f", &["a"]),
            commit("b", &["a"]),
            commit("a", &[]),
        ]);
        assert_eq!(g.width, 2);
        // `a` keeps the lane `f` opened; `b` bends into it below its dot.
        assert_eq!((g.rows[1].node, g.rows[2].node, g.rows[3].node), (1, 0, 1));
        assert_ne!(g.rows[1].color, g.rows[0].color);
        assert!(g.rows[2].segs.contains(&Seg {
            x1: 0,
            y1: 1,
            x2: 1,
            y2: 2,
            color: g.rows[1].color
        }));
        assert_eq!(
            g.rows[3].segs,
            vec![Seg {
                x1: 1,
                y1: 0,
                x2: 1,
                y2: 1,
                color: g.rows[1].color
            }]
        );
    }

    #[test]
    fn parses_log_records_and_refs() {
        let raw = "abc\x1fp1 p2\x1fAda\x1f1700000000\x1fHEAD -> main, origin/main, origin/HEAD, tag: v1\x1fMerge it\x1e\n\
                   p1\x1f\x1fAda\x1f1699999999\x1f\x1fRoot\x1e\n";
        let commits = parse_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, vec!["p1", "p2"]);
        assert_eq!(commits[0].time, 1_700_000_000);
        assert_eq!(commits[0].subject, "Merge it");
        let kinds: Vec<_> = commits[0]
            .refs
            .iter()
            .map(|r| (r.kind, r.name.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (GitRefKind::Head, "main"),
                (GitRefKind::Remote, "origin/main"),
                (GitRefKind::Tag, "v1"),
            ]
        );
        assert!(commits[1].parents.is_empty() && commits[1].refs.is_empty());
    }
}
