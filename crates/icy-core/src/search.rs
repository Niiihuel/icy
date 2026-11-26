//! Workspace file search for the side panel's "Search or enter a URL" field: a bounded directory
//! walk plus a subsequence fuzzy score that favours file-name matches and word starts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SKIP_DIRS: [&str; 6] = [".git", "target", "node_modules", ".next", "dist", ".cache"];
const MAX_WALK: usize = 30_000;

/// Fast in-memory index of workspace files keyed by lowercase basename, so bare filenames
/// (`main.rs`, `review_tests.rs`, `design.md`) and partial paths (`src/main.rs`) printed by CLIs
/// resolve in O(1) without blocking terminal rendering.
#[derive(Default)]
pub struct WorkspaceFileIndex {
    by_name: HashMap<String, Vec<String>>,
}

impl WorkspaceFileIndex {
    pub fn build(root: &Path) -> Self {
        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
        let mut stack = vec![root.to_path_buf()];
        let mut seen = 0usize;
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                seen += 1;
                if seen > MAX_WALK {
                    break;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if !SKIP_DIRS.contains(&name.as_str()) {
                        stack.push(path);
                    }
                    continue;
                }
                let Ok(rel) = path.strip_prefix(root) else {
                    continue;
                };
                let rel = rel.to_string_lossy().replace('\\', "/");
                by_name
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(rel);
            }
        }
        Self { by_name }
    }

    /// Resolves a bare filename (`review_tests.rs`, `design.md`) or partial relative path
    /// (`src/main.rs`) to the best matching file inside `root`.
    pub fn resolve(&self, root: &Path, target: &str) -> Option<PathBuf> {
        let mut norm = target.replace('\\', "/");
        while let Some(rest) = norm.strip_prefix("./") {
            norm = rest.to_string();
        }
        if let Some(rest) = norm.strip_prefix("a/").or_else(|| norm.strip_prefix("b/")) {
            norm = rest.to_string();
        }
        let norm = norm.trim_start_matches('/');
        if norm.is_empty() {
            return None;
        }
        let base = norm.rsplit('/').next()?;
        let candidates = self.by_name.get(&base.to_ascii_lowercase())?;
        let norm_lower = norm.to_ascii_lowercase();
        let has_slash = norm.contains('/');
        let suffix_exact = format!("/{norm}");
        let suffix_lower = format!("/{norm_lower}");

        let mut best: Option<(u8, usize, usize, &str)> = None;
        for rel in candidates {
            let rel_lower = rel.to_ascii_lowercase();
            let tier = if *rel == norm {
                0
            } else if rel_lower == norm_lower {
                1
            } else if has_slash && rel.ends_with(&suffix_exact) {
                2
            } else if has_slash && rel_lower.ends_with(&suffix_lower) {
                3
            } else if !has_slash && rel.rsplit('/').next() == Some(base) {
                4
            } else if !has_slash {
                5
            } else {
                continue;
            };
            let depth = rel.bytes().filter(|&b| b == b'/').count();
            let rank = (tier, depth, rel.len(), rel.as_str());
            if best.is_none_or(|best_rank| rank < best_rank) {
                best = Some(rank);
            }
        }
        best.map(|(_, _, _, rel)| root.join(rel))
            .filter(|p| p.is_file())
    }
}

/// Fuzzy score of `query` against a workspace-relative `path` (higher is better), or `None` when
/// the query is not a case-insensitive subsequence of the path.
pub fn fuzzy_score(query: &str, path: &str) -> Option<i32> {
    let query: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if query.is_empty() {
        return None;
    }
    let lower: Vec<char> = path.to_lowercase().chars().collect();
    let name_start = path
        .rfind(['/', '\\'])
        .map_or(0, |i| path[..=i].chars().count());
    let (mut qi, mut score, mut prev) = (0usize, 0i32, None::<usize>);
    for (i, &c) in lower.iter().enumerate() {
        if qi < query.len() && c == query[qi] {
            score += 1;
            if prev.is_some_and(|p| p + 1 == i) {
                score += 5; // consecutive run
            }
            let boundary = i == 0 || matches!(lower[i - 1], '/' | '\\' | '_' | '-' | '.' | ' ');
            if boundary {
                score += 4;
            }
            if i >= name_start {
                score += 3; // inside the file name
            }
            prev = Some(i);
            qi += 1;
        }
    }
    (qi == query.len()).then(|| score * 100 / (lower.len() as i32 + 10))
}

/// Walks `root` (skipping VCS/build folders) and returns the best `limit` file matches as
/// workspace-relative paths with `/` separators.
pub fn search_files(root: &Path, query: &str, limit: usize) -> Vec<String> {
    let mut stack = vec![root.to_path_buf()];
    let mut scored: Vec<(i32, String)> = Vec::new();
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_WALK {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if let Some(score) = fuzzy_score(query, &rel) {
                scored.push((score, rel));
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.len().cmp(&b.1.len())));
    scored.into_iter().take(limit).map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_subsequences_and_prefers_file_names() {
        assert!(fuzzy_score("xyz", "app/sidebar.tsx").is_none());
        let name = fuzzy_score("sidebar", "app/sidebar.tsx").unwrap();
        let scattered = fuzzy_score("sidebar", "src/some/idea/database/bar.rs").unwrap_or(0);
        assert!(name > scattered);
        let exact = fuzzy_score("tiling", "crates/icy-core/src/tiling.rs").unwrap();
        let deep = fuzzy_score("tiling", "docs/time/limits/interesting.md").unwrap_or(0);
        assert!(exact > deep);
        assert!(fuzzy_score("APP SIDE", "app/sidebar.tsx").is_some());
    }

    #[test]
    fn walks_and_ranks_workspace_files() {
        let dir = std::env::temp_dir().join(format!("icy-search-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::write(dir.join("app/panel.tsx"), "").unwrap();
        std::fs::write(dir.join("app/paint.ts"), "").unwrap();
        std::fs::write(dir.join("target/debug/panel.tsx"), "").unwrap();
        let hits = search_files(&dir, "panel", 10);
        assert_eq!(hits, vec!["app/panel.tsx".to_string()]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
