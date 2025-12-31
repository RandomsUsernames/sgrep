use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::core::graph::{make_repo_id, CommitNode, RepoMetadata};

/// Git repository integration
pub struct GitRepo {
    root_path: String,
    repo_id: String,
}

impl GitRepo {
    /// Open a git repository at the given path
    pub fn open(path: &str) -> Result<Self> {
        let root = Self::find_git_root(path)?;
        let repo_id = make_repo_id(&root);

        Ok(Self {
            root_path: root,
            repo_id,
        })
    }

    /// Find the git root directory
    fn find_git_root(path: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .context("Failed to run git")?;

        if !output.status.success() {
            anyhow::bail!("Not a git repository: {}", path);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get repository metadata
    pub fn metadata(&self) -> Result<RepoMetadata> {
        let origin_url = self.get_remote_url("origin").ok();
        let branch = self.current_branch()?;
        let file_count = self.count_tracked_files()?;

        Ok(RepoMetadata {
            id: self.repo_id.clone(),
            root_path: self.root_path.clone(),
            origin_url,
            branch,
            last_indexed: Some(chrono::Utc::now().to_rfc3339()),
            file_count,
        })
    }

    /// Get the repo ID
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    /// Get the root path
    pub fn root_path(&self) -> &str {
        &self.root_path
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.root_path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get remote URL
    pub fn get_remote_url(&self, remote: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["remote", "get-url", remote])
            .current_dir(&self.root_path)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Remote not found: {}", remote);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Count tracked files
    fn count_tracked_files(&self) -> Result<usize> {
        let output = Command::new("git")
            .args(["ls-files"])
            .current_dir(&self.root_path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).lines().count())
    }

    /// Get recent commits (default: last 100)
    pub fn get_commits(&self, limit: usize) -> Result<Vec<CommitNode>> {
        let output = Command::new("git")
            .args([
                "log",
                &format!("-{}", limit),
                "--pretty=format:%H|%an|%at|%s|%P",
                "--name-only",
            ])
            .current_dir(&self.root_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_commits(&stdout)
    }

    /// Get commits that modified a specific file
    pub fn get_file_commits(&self, file_path: &str, limit: usize) -> Result<Vec<CommitNode>> {
        let output = Command::new("git")
            .args([
                "log",
                &format!("-{}", limit),
                "--pretty=format:%H|%an|%at|%s|%P",
                "--name-only",
                "--follow",
                "--",
                file_path,
            ])
            .current_dir(&self.root_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_commits(&stdout)
    }

    /// Parse git log output into CommitNodes
    fn parse_commits(&self, output: &str) -> Result<Vec<CommitNode>> {
        let mut commits = Vec::new();
        let mut current_commit: Option<CommitNode> = None;

        for line in output.lines() {
            // Check if this is a commit line (starts with 40 hex chars followed by |)
            let is_commit_line = line.len() >= 42
                && line.chars().take(40).all(|c| c.is_ascii_hexdigit())
                && line.chars().nth(40) == Some('|');

            if is_commit_line {
                // This is a commit line
                if let Some(commit) = current_commit.take() {
                    commits.push(commit);
                }

                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    let hash = parts[0].to_string();
                    let author = parts[1].to_string();
                    let timestamp = parts[2].parse().unwrap_or(0);
                    let message = parts[3].to_string();
                    let parents: Vec<String> = if parts.len() > 4 {
                        parts[4].split_whitespace().map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    };

                    current_commit = Some(CommitNode {
                        hash,
                        repo_id: self.repo_id.clone(),
                        message,
                        author,
                        timestamp,
                        files_modified: vec![],
                        parent_hashes: parents,
                    });
                }
            } else if !line.is_empty() {
                // This is a file path
                if let Some(ref mut commit) = current_commit {
                    let file_id = format!("{}:{}", self.repo_id, line);
                    commit.files_modified.push(file_id);
                }
            }
        }

        if let Some(commit) = current_commit {
            commits.push(commit);
        }

        Ok(commits)
    }

    /// Get files changed between two commits
    pub fn diff_files(&self, from: &str, to: &str) -> Result<Vec<DiffEntry>> {
        let output = Command::new("git")
            .args(["diff", "--name-status", from, to])
            .current_dir(&self.root_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let status = match parts[0].chars().next() {
                    Some('A') => DiffStatus::Added,
                    Some('M') => DiffStatus::Modified,
                    Some('D') => DiffStatus::Deleted,
                    Some('R') => DiffStatus::Renamed,
                    _ => DiffStatus::Modified,
                };

                entries.push(DiffEntry {
                    path: parts[1].to_string(),
                    status,
                    old_path: if parts.len() > 2 {
                        Some(parts[2].to_string())
                    } else {
                        None
                    },
                });
            }
        }

        Ok(entries)
    }

    /// Get files changed since last index (using a ref or timestamp)
    pub fn changed_since(&self, since_ref: &str) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", since_ref, "HEAD"])
            .current_dir(&self.root_path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect())
    }

    /// Get the current HEAD commit hash
    pub fn head_commit(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.root_path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Check if a path is tracked by git
    pub fn is_tracked(&self, path: &str) -> bool {
        Command::new("git")
            .args(["ls-files", "--error-unmatch", path])
            .current_dir(&self.root_path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get blame information for a file
    pub fn blame(&self, file_path: &str) -> Result<Vec<BlameLine>> {
        let output = Command::new("git")
            .args(["blame", "--porcelain", file_path])
            .current_dir(&self.root_path)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("git blame failed for {}", file_path);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_blame(&stdout)
    }

    fn parse_blame(&self, output: &str) -> Result<Vec<BlameLine>> {
        let mut lines = Vec::new();
        let mut current_hash = String::new();
        let mut current_author = String::new();
        let mut current_time: i64 = 0;
        let mut line_num = 0;

        for line in output.lines() {
            if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                current_hash = parts[0].to_string();
                if parts.len() > 2 {
                    line_num = parts[2].parse().unwrap_or(0);
                }
            } else if line.starts_with("author ") {
                current_author = line[7..].to_string();
            } else if line.starts_with("author-time ") {
                current_time = line[12..].parse().unwrap_or(0);
            } else if line.starts_with('\t') {
                lines.push(BlameLine {
                    line_number: line_num,
                    commit_hash: current_hash.clone(),
                    author: current_author.clone(),
                    timestamp: current_time,
                    content: line[1..].to_string(),
                });
            }
        }

        Ok(lines)
    }
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub path: String,
    pub status: DiffStatus,
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_hash: String,
    pub author: String,
    pub timestamp: i64,
    pub content: String,
}

/// Check if a directory is a git repository
pub fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
        || Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_repo() {
        // Current directory should be a git repo (searchgrep-rs)
        assert!(is_git_repo("."));
    }
}
