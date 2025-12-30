use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct FileScanner {
    root: String,
    extensions: HashSet<String>,
}

impl FileScanner {
    pub fn new(root: &str) -> Self {
        let mut extensions = HashSet::new();
        // Default supported extensions
        for ext in &[
            "rs",
            "ts",
            "tsx",
            "js",
            "jsx",
            "py",
            "go",
            "java",
            "c",
            "cpp",
            "h",
            "hpp",
            "cs",
            "rb",
            "php",
            "swift",
            "kt",
            "scala",
            "clj",
            "ex",
            "exs",
            "erl",
            "hs",
            "ml",
            "fs",
            "vue",
            "svelte",
            "html",
            "css",
            "scss",
            "sass",
            "less",
            "json",
            "yaml",
            "yml",
            "toml",
            "xml",
            "md",
            "txt",
            "sh",
            "bash",
            "zsh",
            "fish",
            "ps1",
            "sql",
            "graphql",
            "proto",
            "dockerfile",
            "makefile",
            "cmake",
            "gradle",
            "cargo",
        ] {
            extensions.insert(ext.to_string());
        }

        Self {
            root: root.to_string(),
            extensions,
        }
    }

    pub fn with_extensions(mut self, exts: &[String]) -> Self {
        self.extensions = exts.iter().map(|s| s.to_lowercase()).collect();
        self
    }

    pub fn scan(&self) -> Result<Vec<ScannedFile>> {
        let mut files = Vec::new();

        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .ignore(true)
            .parents(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Check extension
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if !self.extensions.contains(&ext_str) {
                    continue;
                }
            } else {
                // Check for extensionless files by name
                if let Some(name) = path.file_name() {
                    let name_lower = name.to_string_lossy().to_lowercase();
                    if !["dockerfile", "makefile", "cargo"].contains(&name_lower.as_str()) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            // Read file content
            match fs::read_to_string(path) {
                Ok(content) => {
                    // Skip binary or very large files
                    if content.len() > 1_000_000 || content.contains('\0') {
                        continue;
                    }

                    files.push(ScannedFile {
                        path: path.to_string_lossy().to_string(),
                        content,
                        language: detect_language(path),
                    });
                }
                Err(_) => continue,
            }
        }

        Ok(files)
    }

    pub fn scan_single(&self, path: &Path) -> Result<Option<ScannedFile>> {
        if !path.is_file() {
            return Ok(None);
        }

        // Check extension
        let has_valid_ext = if let Some(ext) = path.extension() {
            self.extensions
                .contains(&ext.to_string_lossy().to_lowercase())
        } else if let Some(name) = path.file_name() {
            let name_lower = name.to_string_lossy().to_lowercase();
            ["dockerfile", "makefile", "cargo"].contains(&name_lower.as_str())
        } else {
            false
        };

        if !has_valid_ext {
            return Ok(None);
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                if content.len() > 1_000_000 || content.contains('\0') {
                    return Ok(None);
                }

                Ok(Some(ScannedFile {
                    path: path.to_string_lossy().to_string(),
                    content,
                    language: detect_language(path),
                }))
            }
            Err(_) => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: String,
    pub content: String,
    pub language: Option<String>,
}

fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();

    let lang = match ext.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        "scala" => "scala",
        "clj" => "clojure",
        "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" => "haskell",
        "ml" => "ocaml",
        "fs" => "fsharp",
        "vue" => "vue",
        "svelte" => "svelte",
        "html" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "md" => "markdown",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "graphql" => "graphql",
        "proto" => "protobuf",
        _ => return None,
    };

    Some(lang.to_string())
}

pub fn get_file_type(path: &str) -> Option<String> {
    let path = Path::new(path);
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}
