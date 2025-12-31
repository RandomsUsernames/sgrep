use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Unique identifier for a repository
pub type RepoId = String;
/// Unique identifier for a file (repo:path)
pub type FileId = String;
/// Unique identifier for a chunk
pub type ChunkId = String;
/// Git commit hash
pub type CommitHash = String;

/// Node identifier in the graph
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeId {
    Repo(RepoId),
    File(FileId),
    Chunk(ChunkId),
    Commit(CommitHash),
    Symbol(String), // function/class name
}

/// Types of relationships between nodes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// File imports another file
    Imports,
    /// File is imported by another file
    ImportedBy,
    /// Symbol references another symbol
    References,
    /// Symbol is referenced by another symbol
    ReferencedBy,
    /// File was modified by commit
    ModifiedBy,
    /// Commit modified a file
    Modifies,
    /// File belongs to repo
    BelongsTo,
    /// Repo contains file
    Contains,
    /// File contains chunk
    ContainsChunk,
    /// Chunk belongs to file
    ChunkOf,
    /// Symbol defined in file
    DefinedIn,
    /// File defines symbol
    Defines,
    /// File depends on another (transitive imports)
    DependsOn,
    /// File is depended on by another
    DependedBy,
}

/// An edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    /// Optional metadata (e.g., line number, commit message)
    pub metadata: Option<HashMap<String, String>>,
}

/// Metadata about a repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMetadata {
    pub id: RepoId,
    pub root_path: String,
    pub origin_url: Option<String>,
    pub branch: String,
    pub last_indexed: Option<String>,
    pub file_count: usize,
}

/// A file node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub repo_id: RepoId,
    pub language: Option<String>,
    pub hash: String,
    pub chunk_ids: Vec<ChunkId>,
    /// Symbols defined in this file
    pub symbols: Vec<String>,
    /// Files this file imports
    pub imports: Vec<FileId>,
    /// Files that import this file
    pub imported_by: Vec<FileId>,
}

/// A commit node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitNode {
    pub hash: CommitHash,
    pub repo_id: RepoId,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    /// Files modified in this commit
    pub files_modified: Vec<FileId>,
    pub parent_hashes: Vec<CommitHash>,
}

/// A symbol (function, class, etc.) in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolNode {
    pub name: String,
    pub kind: SymbolKind,
    pub file_id: FileId,
    pub start_line: usize,
    pub end_line: usize,
    /// Symbols this symbol references
    pub references: Vec<String>,
    /// Symbols that reference this symbol
    pub referenced_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Constant,
    Variable,
    Module,
    Type,
}

/// The knowledge graph
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    /// All repositories
    pub repos: HashMap<RepoId, RepoMetadata>,
    /// All files indexed by FileId
    pub files: HashMap<FileId, FileNode>,
    /// All commits indexed by hash
    pub commits: HashMap<CommitHash, CommitNode>,
    /// All symbols indexed by qualified name
    pub symbols: HashMap<String, SymbolNode>,
    /// Edges in the graph (adjacency list by source node)
    edges_out: HashMap<NodeId, Vec<Edge>>,
    /// Reverse edges (adjacency list by target node)
    edges_in: HashMap<NodeId, Vec<Edge>>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a repository to the graph
    pub fn add_repo(&mut self, repo: RepoMetadata) {
        self.repos.insert(repo.id.clone(), repo);
    }

    /// Add a file to the graph
    pub fn add_file(&mut self, file: FileNode) {
        let file_id = file.id.clone();
        let repo_id = file.repo_id.clone();

        // Add file -> repo edge
        self.add_edge(Edge {
            from: NodeId::File(file_id.clone()),
            to: NodeId::Repo(repo_id.clone()),
            kind: EdgeKind::BelongsTo,
            metadata: None,
        });

        // Add repo -> file edge
        self.add_edge(Edge {
            from: NodeId::Repo(repo_id),
            to: NodeId::File(file_id.clone()),
            kind: EdgeKind::Contains,
            metadata: None,
        });

        self.files.insert(file_id, file);
    }

    /// Add a commit to the graph
    pub fn add_commit(&mut self, commit: CommitNode) {
        let hash = commit.hash.clone();

        // Add edges for modified files
        for file_id in &commit.files_modified {
            self.add_edge(Edge {
                from: NodeId::Commit(hash.clone()),
                to: NodeId::File(file_id.clone()),
                kind: EdgeKind::Modifies,
                metadata: None,
            });

            self.add_edge(Edge {
                from: NodeId::File(file_id.clone()),
                to: NodeId::Commit(hash.clone()),
                kind: EdgeKind::ModifiedBy,
                metadata: None,
            });
        }

        self.commits.insert(hash, commit);
    }

    /// Add a symbol to the graph
    pub fn add_symbol(&mut self, symbol: SymbolNode) {
        let name = symbol.name.clone();
        let file_id = symbol.file_id.clone();

        // Add symbol <-> file edges
        self.add_edge(Edge {
            from: NodeId::Symbol(name.clone()),
            to: NodeId::File(file_id.clone()),
            kind: EdgeKind::DefinedIn,
            metadata: None,
        });

        self.add_edge(Edge {
            from: NodeId::File(file_id),
            to: NodeId::Symbol(name.clone()),
            kind: EdgeKind::Defines,
            metadata: None,
        });

        self.symbols.insert(name, symbol);
    }

    /// Add an import relationship between files
    pub fn add_import(&mut self, from_file: &FileId, to_file: &FileId) {
        self.add_edge(Edge {
            from: NodeId::File(from_file.clone()),
            to: NodeId::File(to_file.clone()),
            kind: EdgeKind::Imports,
            metadata: None,
        });

        self.add_edge(Edge {
            from: NodeId::File(to_file.clone()),
            to: NodeId::File(from_file.clone()),
            kind: EdgeKind::ImportedBy,
            metadata: None,
        });

        // Update file nodes
        if let Some(file) = self.files.get_mut(from_file) {
            if !file.imports.contains(to_file) {
                file.imports.push(to_file.clone());
            }
        }
        if let Some(file) = self.files.get_mut(to_file) {
            if !file.imported_by.contains(from_file) {
                file.imported_by.push(from_file.clone());
            }
        }
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges_out
            .entry(edge.from.clone())
            .or_default()
            .push(edge.clone());

        self.edges_in.entry(edge.to.clone()).or_default().push(edge);
    }

    /// Get outgoing edges from a node
    pub fn edges_from(&self, node: &NodeId) -> &[Edge] {
        self.edges_out
            .get(node)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get incoming edges to a node
    pub fn edges_to(&self, node: &NodeId) -> &[Edge] {
        self.edges_in.get(node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all files that a file imports (direct)
    pub fn get_imports(&self, file_id: &FileId) -> Vec<&FileNode> {
        self.files
            .get(file_id)
            .map(|f| {
                f.imports
                    .iter()
                    .filter_map(|id| self.files.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all files that import a file (direct)
    pub fn get_importers(&self, file_id: &FileId) -> Vec<&FileNode> {
        self.files
            .get(file_id)
            .map(|f| {
                f.imported_by
                    .iter()
                    .filter_map(|id| self.files.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all related files (imports + importers + same directory)
    pub fn get_related_files(&self, file_id: &FileId, depth: usize) -> Vec<&FileNode> {
        let mut visited: HashSet<&FileId> = HashSet::new();
        let mut result: Vec<&FileNode> = Vec::new();
        let mut queue: Vec<(&FileId, usize)> = vec![(file_id, 0)];

        while let Some((current_id, current_depth)) = queue.pop() {
            if current_depth > depth || visited.contains(current_id) {
                continue;
            }
            visited.insert(current_id);

            if let Some(file) = self.files.get(current_id) {
                if current_id != file_id {
                    result.push(file);
                }

                if current_depth < depth {
                    // Add imports and importers to queue
                    for import_id in &file.imports {
                        if !visited.contains(import_id) {
                            queue.push((import_id, current_depth + 1));
                        }
                    }
                    for importer_id in &file.imported_by {
                        if !visited.contains(importer_id) {
                            queue.push((importer_id, current_depth + 1));
                        }
                    }
                }
            }
        }

        result
    }

    /// Get commits that modified a file
    pub fn get_file_history(&self, file_id: &FileId) -> Vec<&CommitNode> {
        let node = NodeId::File(file_id.clone());
        self.edges_from(&node)
            .iter()
            .filter(|e| e.kind == EdgeKind::ModifiedBy)
            .filter_map(|e| {
                if let NodeId::Commit(hash) = &e.to {
                    self.commits.get(hash)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get files modified in a commit
    pub fn get_commit_files(&self, commit_hash: &CommitHash) -> Vec<&FileNode> {
        self.commits
            .get(commit_hash)
            .map(|c| {
                c.files_modified
                    .iter()
                    .filter_map(|id| self.files.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get statistics about the graph
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            repos: self.repos.len(),
            files: self.files.len(),
            commits: self.commits.len(),
            symbols: self.symbols.len(),
            edges: self.edges_out.values().map(|v| v.len()).sum(),
        }
    }

    /// Clear the graph
    pub fn clear(&mut self) {
        self.repos.clear();
        self.files.clear();
        self.commits.clear();
        self.symbols.clear();
        self.edges_out.clear();
        self.edges_in.clear();
    }
}

#[derive(Debug, Clone)]
pub struct GraphStats {
    pub repos: usize,
    pub files: usize,
    pub commits: usize,
    pub symbols: usize,
    pub edges: usize,
}

/// Generate a FileId from repo and path
pub fn make_file_id(repo_id: &str, path: &str) -> FileId {
    format!("{}:{}", repo_id, path)
}

/// Generate a RepoId from path
pub fn make_repo_id(root_path: &str) -> RepoId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(root_path.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file_creates_edges() {
        let mut graph = KnowledgeGraph::new();

        graph.add_repo(RepoMetadata {
            id: "repo1".to_string(),
            root_path: "/test".to_string(),
            origin_url: None,
            branch: "main".to_string(),
            last_indexed: None,
            file_count: 0,
        });

        graph.add_file(FileNode {
            id: "repo1:src/main.rs".to_string(),
            path: "src/main.rs".to_string(),
            repo_id: "repo1".to_string(),
            language: Some("rust".to_string()),
            hash: "abc123".to_string(),
            chunk_ids: vec![],
            symbols: vec![],
            imports: vec![],
            imported_by: vec![],
        });

        let node = NodeId::File("repo1:src/main.rs".to_string());
        let edges = graph.edges_from(&node);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].kind, EdgeKind::BelongsTo);
    }

    #[test]
    fn test_imports() {
        let mut graph = KnowledgeGraph::new();

        graph.add_file(FileNode {
            id: "a.rs".to_string(),
            path: "a.rs".to_string(),
            repo_id: "repo".to_string(),
            language: Some("rust".to_string()),
            hash: "".to_string(),
            chunk_ids: vec![],
            symbols: vec![],
            imports: vec![],
            imported_by: vec![],
        });

        graph.add_file(FileNode {
            id: "b.rs".to_string(),
            path: "b.rs".to_string(),
            repo_id: "repo".to_string(),
            language: Some("rust".to_string()),
            hash: "".to_string(),
            chunk_ids: vec![],
            symbols: vec![],
            imports: vec![],
            imported_by: vec![],
        });

        graph.add_import(&"a.rs".to_string(), &"b.rs".to_string());

        let imports = graph.get_imports(&"a.rs".to_string());
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].id, "b.rs");

        let importers = graph.get_importers(&"b.rs".to_string());
        assert_eq!(importers.len(), 1);
        assert_eq!(importers[0].id, "a.rs");
    }
}
