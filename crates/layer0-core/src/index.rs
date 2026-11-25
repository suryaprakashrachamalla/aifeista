use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

const MAX_FILE_BYTES: u64 = 512 * 1024;
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    ".wrangler",
    ".next",
    "build",
    ".venv",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMatch {
    pub path: String,
    pub line_no: usize,
    pub snippet: String,
    pub score: u32,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    rel_path: String,
    lines: Vec<String>,
}

/// FlashGrep-inspired resident workspace index kept in memory for sub-millisecond
/// file and symbol lookups without spawning external `grep`/`find` subprocesses.
#[derive(Clone)]
pub struct WorkspaceIndex {
    roots: Vec<PathBuf>,
    files: Arc<RwLock<Vec<IndexedFile>>>,
}

impl WorkspaceIndex {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            files: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Scans all configured roots and refreshes the resident index. Returns indexed file count.
    pub fn rebuild(&self) -> usize {
        let mut collected = Vec::new();
        for root in &self.roots {
            if !root.exists() {
                continue;
            }
            for entry in WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !is_ignored(e))
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                if meta.len() > MAX_FILE_BYTES {
                    continue;
                }
                let Ok(bytes) = fs::read(entry.path()) else {
                    continue;
                };
                if bytes.contains(&0) {
                    continue; // Skip binary files
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                let rel = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .to_string();
                let lines = text.lines().map(|l| l.to_string()).collect();
                collected.push(IndexedFile {
                    rel_path: rel,
                    lines,
                });
            }
        }
        let count = collected.len();
        if let Ok(mut guard) = self.files.write() {
            *guard = collected;
        }
        count
    }

    pub fn file_count(&self) -> usize {
        self.files.read().map(|f| f.len()).unwrap_or(0)
    }

    /// Searches indexed paths and file contents in memory.
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchMatch> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() || max_results == 0 {
            return Vec::new();
        }
        let Ok(files) = self.files.read() else {
            return Vec::new();
        };

        let mut matches = Vec::new();
        for file in files.iter() {
            let path_lower = file.rel_path.to_lowercase();
            if path_lower.contains(&needle) {
                let score = if path_lower.ends_with(&needle) { 100 } else { 70 };
                matches.push(SearchMatch {
                    path: file.rel_path.clone(),
                    line_no: 1,
                    snippet: format!("[path match] {}", file.rel_path),
                    score,
                });
            }

            for (idx, line) in file.lines.iter().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    let trimmed = line.trim();
                    let snippet = if trimmed.len() > 160 {
                        format!("{}...", &trimmed[..160])
                    } else {
                        trimmed.to_string()
                    };
                    matches.push(SearchMatch {
                        path: file.rel_path.clone(),
                        line_no: idx + 1,
                        snippet,
                        score: 50,
                    });
                    if matches.len() >= max_results * 4 {
                        break;
                    }
                }
            }
        }

        matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
        matches.truncate(max_results);
        matches
    }
}

fn is_ignored(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    IGNORED_DIRS.iter().any(|ignored| *ignored == name)
}

pub fn default_workspace_roots(repo_root: &Path) -> Vec<PathBuf> {
    vec![
        repo_root.join("crates"),
        repo_root.join("desktop"),
        repo_root.join("companion/leanring-buddy"),
        repo_root.join("runtime/packages/core"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resident_index_finds_path_and_content_and_skips_ignored() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let git_dir = dir.path().join(".git");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&git_dir).unwrap();

        fs::write(
            src.join("CompanionManager.swift"),
            "final class CompanionManager {\n  let marker = \"hello_carnival\"\n}\n",
        )
        .unwrap();
        fs::write(git_dir.join("config"), "hello_carnival_ignored").unwrap();

        let index = WorkspaceIndex::new(vec![dir.path().to_path_buf()]);
        let count = index.rebuild();
        assert_eq!(count, 1);

        let hits = index.search("hello_carnival", 10);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.contains("CompanionManager.swift"));
        assert_eq!(hits[0].line_no, 2);
    }
}
