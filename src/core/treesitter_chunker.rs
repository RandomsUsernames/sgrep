use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser, Tree};

#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub chunk_type: ChunkType,
    pub name: Option<String>,
    pub parent_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Module,
    Import,
    Comment,
    Code,
}

impl ChunkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkType::Function => "function",
            ChunkType::Method => "method",
            ChunkType::Class => "class",
            ChunkType::Struct => "struct",
            ChunkType::Enum => "enum",
            ChunkType::Trait => "trait",
            ChunkType::Interface => "interface",
            ChunkType::Module => "module",
            ChunkType::Import => "import",
            ChunkType::Comment => "comment",
            ChunkType::Code => "code",
        }
    }
}

/// Language configuration for tree-sitter parsing
struct LanguageConfig {
    language: Language,
    /// Node types that represent functions/methods
    function_types: Vec<&'static str>,
    /// Node types that represent classes/structs/types
    class_types: Vec<&'static str>,
    /// Node types that represent imports
    import_types: Vec<&'static str>,
    /// Node types that represent comments
    comment_types: Vec<&'static str>,
    /// Field name for identifier extraction
    name_field: &'static str,
}

pub struct TreeSitterChunker {
    max_chunk_size: usize,
    min_chunk_size: usize,
    parsers: HashMap<String, Parser>,
}

impl Default for TreeSitterChunker {
    fn default() -> Self {
        Self {
            max_chunk_size: 1500,
            min_chunk_size: 50,
            parsers: HashMap::new(),
        }
    }
}

impl TreeSitterChunker {
    pub fn new(max_chunk_size: usize, min_chunk_size: usize) -> Self {
        Self {
            max_chunk_size,
            min_chunk_size,
            parsers: HashMap::new(),
        }
    }

    /// Get or create a parser for the given language
    fn get_parser(&mut self, language: &str) -> Option<&mut Parser> {
        if !self.parsers.contains_key(language) {
            if let Some(config) = Self::get_language_config(language) {
                let mut parser = Parser::new();
                if parser.set_language(&config.language).is_ok() {
                    self.parsers.insert(language.to_string(), parser);
                }
            }
        }
        self.parsers.get_mut(language)
    }

    /// Get language configuration for tree-sitter
    fn get_language_config(language: &str) -> Option<LanguageConfig> {
        match language {
            "rust" => Some(LanguageConfig {
                language: tree_sitter_rust::LANGUAGE.into(),
                function_types: vec!["function_item", "impl_item"],
                class_types: vec!["struct_item", "enum_item", "trait_item", "type_item"],
                import_types: vec!["use_declaration"],
                comment_types: vec!["line_comment", "block_comment"],
                name_field: "name",
            }),
            "python" => Some(LanguageConfig {
                language: tree_sitter_python::LANGUAGE.into(),
                function_types: vec!["function_definition", "async_function_definition"],
                class_types: vec!["class_definition"],
                import_types: vec!["import_statement", "import_from_statement"],
                comment_types: vec!["comment", "string"], // docstrings are strings
                name_field: "name",
            }),
            "javascript" | "js" => Some(LanguageConfig {
                language: tree_sitter_javascript::LANGUAGE.into(),
                function_types: vec![
                    "function_declaration",
                    "arrow_function",
                    "method_definition",
                    "function_expression",
                ],
                class_types: vec!["class_declaration", "class_expression"],
                import_types: vec!["import_statement"],
                comment_types: vec!["comment"],
                name_field: "name",
            }),
            "typescript" | "ts" => Some(LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                function_types: vec![
                    "function_declaration",
                    "arrow_function",
                    "method_definition",
                    "function_expression",
                ],
                class_types: vec![
                    "class_declaration",
                    "interface_declaration",
                    "type_alias_declaration",
                ],
                import_types: vec!["import_statement"],
                comment_types: vec!["comment"],
                name_field: "name",
            }),
            "tsx" => Some(LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TSX.into(),
                function_types: vec![
                    "function_declaration",
                    "arrow_function",
                    "method_definition",
                    "function_expression",
                ],
                class_types: vec![
                    "class_declaration",
                    "interface_declaration",
                    "type_alias_declaration",
                ],
                import_types: vec!["import_statement"],
                comment_types: vec!["comment"],
                name_field: "name",
            }),
            "go" => Some(LanguageConfig {
                language: tree_sitter_go::LANGUAGE.into(),
                function_types: vec!["function_declaration", "method_declaration"],
                class_types: vec!["type_declaration"],
                import_types: vec!["import_declaration"],
                comment_types: vec!["comment"],
                name_field: "name",
            }),
            "java" => Some(LanguageConfig {
                language: tree_sitter_java::LANGUAGE.into(),
                function_types: vec!["method_declaration", "constructor_declaration"],
                class_types: vec![
                    "class_declaration",
                    "interface_declaration",
                    "enum_declaration",
                ],
                import_types: vec!["import_declaration"],
                comment_types: vec!["line_comment", "block_comment"],
                name_field: "name",
            }),
            "c" => Some(LanguageConfig {
                language: tree_sitter_c::LANGUAGE.into(),
                function_types: vec!["function_definition"],
                class_types: vec!["struct_specifier", "enum_specifier", "union_specifier"],
                import_types: vec!["preproc_include"],
                comment_types: vec!["comment"],
                name_field: "declarator",
            }),
            "cpp" | "c++" => Some(LanguageConfig {
                language: tree_sitter_cpp::LANGUAGE.into(),
                function_types: vec!["function_definition", "template_declaration"],
                class_types: vec!["class_specifier", "struct_specifier", "enum_specifier"],
                import_types: vec!["preproc_include"],
                comment_types: vec!["comment"],
                name_field: "declarator",
            }),
            _ => None,
        }
    }

    /// Main chunking function
    pub fn chunk(&mut self, content: &str, language: Option<&str>) -> Vec<Chunk> {
        let lang = language.unwrap_or("unknown");

        // Try tree-sitter parsing
        if let Some(chunks) = self.treesitter_chunk(content, lang) {
            if !chunks.is_empty() {
                return chunks;
            }
        }

        // Fallback to simple line-based chunking
        self.simple_chunk(content)
    }

    /// Parse and chunk using tree-sitter
    fn treesitter_chunk(&mut self, content: &str, language: &str) -> Option<Vec<Chunk>> {
        let config = Self::get_language_config(language)?;

        let parser = self.get_parser(language)?;
        let tree = parser.parse(content, None)?;

        let mut chunks = Vec::new();
        let source_bytes = content.as_bytes();

        self.extract_chunks_recursive(
            tree.root_node(),
            source_bytes,
            content,
            &config,
            None,
            &mut chunks,
        );

        // Sort chunks by start position
        chunks.sort_by_key(|c| c.start_byte);

        // Remove overlapping chunks (keep larger semantic units)
        let chunks = self.deduplicate_chunks(chunks);

        // Split oversized chunks
        let chunks = self.split_oversized_chunks(chunks, content);

        Some(chunks)
    }

    /// Recursively extract chunks from AST
    fn extract_chunks_recursive(
        &self,
        node: Node,
        source: &[u8],
        content: &str,
        config: &LanguageConfig,
        parent_name: Option<&str>,
        chunks: &mut Vec<Chunk>,
    ) {
        let node_type = node.kind();

        // Check if this node is a semantic boundary
        let chunk_type = self.get_chunk_type(node_type, config);

        if let Some(ctype) = chunk_type {
            let name = self.extract_name(&node, source, config);
            let chunk_content = &content[node.start_byte()..node.end_byte()];

            chunks.push(Chunk {
                content: chunk_content.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                chunk_type: ctype,
                name: name.clone(),
                parent_name: parent_name.map(String::from),
            });

            // Recurse into children with this node as parent
            let current_name = name.as_deref().or(parent_name);
            for child in node.children(&mut node.walk()) {
                self.extract_chunks_recursive(child, source, content, config, current_name, chunks);
            }
        } else {
            // Continue recursing
            for child in node.children(&mut node.walk()) {
                self.extract_chunks_recursive(child, source, content, config, parent_name, chunks);
            }
        }
    }

    /// Determine chunk type from node type
    fn get_chunk_type(&self, node_type: &str, config: &LanguageConfig) -> Option<ChunkType> {
        if config.function_types.contains(&node_type) {
            // Check if it's a method (inside a class) vs standalone function
            if node_type == "method_definition" || node_type == "method_declaration" {
                return Some(ChunkType::Method);
            }
            return Some(ChunkType::Function);
        }

        if config.class_types.contains(&node_type) {
            if node_type.contains("struct") {
                return Some(ChunkType::Struct);
            }
            if node_type.contains("enum") {
                return Some(ChunkType::Enum);
            }
            if node_type.contains("trait") {
                return Some(ChunkType::Trait);
            }
            if node_type.contains("interface") {
                return Some(ChunkType::Interface);
            }
            return Some(ChunkType::Class);
        }

        if config.import_types.contains(&node_type) {
            return Some(ChunkType::Import);
        }

        if config.comment_types.contains(&node_type) {
            return Some(ChunkType::Comment);
        }

        None
    }

    /// Extract the name/identifier from a node
    fn extract_name(&self, node: &Node, source: &[u8], config: &LanguageConfig) -> Option<String> {
        // Try to find the name field
        if let Some(name_node) = node.child_by_field_name(config.name_field) {
            let name = &source[name_node.start_byte()..name_node.end_byte()];
            return String::from_utf8(name.to_vec()).ok();
        }

        // For some languages, we need to traverse to find the identifier
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                let name = &source[child.start_byte()..child.end_byte()];
                return String::from_utf8(name.to_vec()).ok();
            }
        }

        None
    }

    /// Remove overlapping chunks, preferring larger semantic units
    fn deduplicate_chunks(&self, chunks: Vec<Chunk>) -> Vec<Chunk> {
        if chunks.is_empty() {
            return chunks;
        }

        let mut result = Vec::new();
        let mut covered_ranges: Vec<(usize, usize)> = Vec::new();

        // Sort by size (largest first) to prefer larger semantic units
        let mut sorted_chunks = chunks;
        sorted_chunks.sort_by(|a, b| {
            let a_size = a.end_byte - a.start_byte;
            let b_size = b.end_byte - b.start_byte;
            b_size.cmp(&a_size)
        });

        for chunk in sorted_chunks {
            // Check if this chunk is already covered by a larger chunk
            let is_covered = covered_ranges
                .iter()
                .any(|(start, end)| chunk.start_byte >= *start && chunk.end_byte <= *end);

            if !is_covered {
                covered_ranges.push((chunk.start_byte, chunk.end_byte));
                result.push(chunk);
            }
        }

        result
    }

    /// Split chunks that exceed max size
    fn split_oversized_chunks(&self, chunks: Vec<Chunk>, content: &str) -> Vec<Chunk> {
        let mut result = Vec::new();

        for chunk in chunks {
            if chunk.content.len() <= self.max_chunk_size {
                if chunk.content.len() >= self.min_chunk_size {
                    result.push(chunk);
                }
            } else {
                // Split large chunk by lines
                let lines: Vec<&str> = chunk.content.lines().collect();
                let mut current_start = 0;
                let mut current_content = String::new();
                let mut current_line_start = chunk.start_line;

                for (i, line) in lines.iter().enumerate() {
                    let new_len = current_content.len() + line.len() + 1;

                    if new_len > self.max_chunk_size && !current_content.is_empty() {
                        // Save current chunk
                        result.push(Chunk {
                            content: current_content.clone(),
                            start_line: current_line_start,
                            end_line: chunk.start_line + i - 1,
                            start_byte: chunk.start_byte + current_start,
                            end_byte: chunk.start_byte + current_start + current_content.len(),
                            chunk_type: chunk.chunk_type.clone(),
                            name: chunk.name.clone(),
                            parent_name: chunk.parent_name.clone(),
                        });

                        current_start += current_content.len() + 1;
                        current_content = String::new();
                        current_line_start = chunk.start_line + i;
                    }

                    if !current_content.is_empty() {
                        current_content.push('\n');
                    }
                    current_content.push_str(line);
                }

                // Save remaining content
                if current_content.len() >= self.min_chunk_size {
                    result.push(Chunk {
                        content: current_content.clone(),
                        start_line: current_line_start,
                        end_line: chunk.end_line,
                        start_byte: chunk.start_byte + current_start,
                        end_byte: chunk.end_byte,
                        chunk_type: chunk.chunk_type.clone(),
                        name: chunk.name.clone(),
                        parent_name: chunk.parent_name.clone(),
                    });
                }
            }
        }

        result
    }

    /// Simple line-based chunking fallback
    fn simple_chunk(&self, content: &str) -> Vec<Chunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();
        let mut current_start = 0;
        let mut current_size = 0;
        let mut current_byte_start = 0;

        for (i, line) in lines.iter().enumerate() {
            current_size += line.len() + 1;

            if current_size >= self.max_chunk_size {
                let chunk_content: String = lines[current_start..=i].join("\n");
                let chunk_bytes = chunk_content.len();

                chunks.push(Chunk {
                    content: chunk_content,
                    start_line: current_start + 1,
                    end_line: i + 1,
                    start_byte: current_byte_start,
                    end_byte: current_byte_start + chunk_bytes,
                    chunk_type: ChunkType::Code,
                    name: None,
                    parent_name: None,
                });

                current_byte_start += chunk_bytes + 1;
                current_start = i.saturating_sub(2);
                current_size = 0;
            }
        }

        // Last chunk
        if current_start < lines.len() {
            let chunk_content: String = lines[current_start..].join("\n");
            if chunk_content.len() >= self.min_chunk_size || chunks.is_empty() {
                chunks.push(Chunk {
                    content: chunk_content.clone(),
                    start_line: current_start + 1,
                    end_line: lines.len(),
                    start_byte: current_byte_start,
                    end_byte: current_byte_start + chunk_content.len(),
                    chunk_type: ChunkType::Code,
                    name: None,
                    parent_name: None,
                });
            }
        }

        chunks
    }
}

/// Detect language from file extension
pub fn detect_language(file_path: &str) -> Option<&'static str> {
    let extension = file_path.rsplit('.').next()?;

    match extension.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" | "pyw" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "jsx" => Some("javascript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some("cpp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_chunking() {
        let mut chunker = TreeSitterChunker::default();
        let code = r#"
fn main() {
    println!("Hello, world!");
}

pub struct Config {
    name: String,
    value: i32,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self { name, value: 0 }
    }
}
"#;
        let chunks = chunker.chunk(code, Some("rust"));
        assert!(!chunks.is_empty());

        // Should find function and struct
        let types: Vec<_> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(types.contains(&&ChunkType::Function));
    }

    #[test]
    fn test_python_chunking() {
        let mut chunker = TreeSitterChunker::default();
        let code = r#"
def hello():
    print("Hello, world!")

class MyClass:
    def __init__(self):
        self.value = 0

    def get_value(self):
        return self.value
"#;
        let chunks = chunker.chunk(code, Some("python"));
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust"));
        assert_eq!(detect_language("app.py"), Some("python"));
        assert_eq!(detect_language("index.ts"), Some("typescript"));
        assert_eq!(detect_language("README.md"), None);
    }
}
