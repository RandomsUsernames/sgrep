use regex::Regex;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub chunk_type: ChunkType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkType {
    Function,
    Class,
    Module,
    Import,
    Comment,
    Code,
}

impl ChunkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkType::Function => "function",
            ChunkType::Class => "class",
            ChunkType::Module => "module",
            ChunkType::Import => "import",
            ChunkType::Comment => "comment",
            ChunkType::Code => "code",
        }
    }
}

pub struct CodeChunker {
    max_chunk_size: usize,
    min_chunk_size: usize,
    overlap: usize,
}

impl Default for CodeChunker {
    fn default() -> Self {
        Self {
            max_chunk_size: 1500,
            min_chunk_size: 100,
            overlap: 100,
        }
    }
}

impl CodeChunker {
    pub fn new(max_chunk_size: usize, min_chunk_size: usize, overlap: usize) -> Self {
        Self {
            max_chunk_size,
            min_chunk_size,
            overlap,
        }
    }

    pub fn chunk(&self, content: &str, language: Option<&str>) -> Vec<Chunk> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return vec![];
        }

        // Try semantic chunking first
        let semantic_chunks = self.semantic_chunk(&lines, language);

        if !semantic_chunks.is_empty() {
            return semantic_chunks;
        }

        // Fall back to simple chunking
        self.simple_chunk(&lines)
    }

    fn semantic_chunk(&self, lines: &[&str], language: Option<&str>) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let patterns = get_language_patterns(language);

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Check for semantic boundaries
            if let Some((chunk_type, end_line)) = self.find_semantic_block(lines, i, &patterns) {
                let block_lines: Vec<&str> = lines[i..=end_line].to_vec();
                let content = block_lines.join("\n");

                // If block is too large, split it
                if content.len() > self.max_chunk_size {
                    let sub_chunks = self.split_large_block(&block_lines, i, chunk_type.clone());
                    chunks.extend(sub_chunks);
                } else if content.len() >= self.min_chunk_size {
                    chunks.push(Chunk {
                        content,
                        start_line: i + 1,
                        end_line: end_line + 1,
                        chunk_type,
                    });
                }

                i = end_line + 1;
                continue;
            }

            i += 1;
        }

        chunks
    }

    fn find_semantic_block(
        &self,
        lines: &[&str],
        start: usize,
        patterns: &LanguagePatterns,
    ) -> Option<(ChunkType, usize)> {
        let line = lines[start].trim();

        // Check for function/method
        if patterns.function_pattern.is_match(line) {
            let end = self.find_block_end(lines, start, patterns);
            return Some((ChunkType::Function, end));
        }

        // Check for class/struct
        if patterns.class_pattern.is_match(line) {
            let end = self.find_block_end(lines, start, patterns);
            return Some((ChunkType::Class, end));
        }

        // Check for imports
        if patterns.import_pattern.is_match(line) {
            let end = self.find_import_block_end(lines, start, patterns);
            return Some((ChunkType::Import, end));
        }

        // Check for doc comments
        if patterns.doc_comment_pattern.is_match(line) {
            let end = self.find_comment_end(lines, start, patterns);
            return Some((ChunkType::Comment, end));
        }

        None
    }

    fn find_block_end(&self, lines: &[&str], start: usize, patterns: &LanguagePatterns) -> usize {
        let mut brace_count = 0;
        let mut paren_count = 0;
        let mut found_open = false;

        for (i, line) in lines.iter().enumerate().skip(start) {
            for ch in line.chars() {
                match ch {
                    '{' => {
                        brace_count += 1;
                        found_open = true;
                    }
                    '}' => brace_count -= 1,
                    '(' => paren_count += 1,
                    ')' => paren_count -= 1,
                    _ => {}
                }
            }

            // For languages without braces (Python, etc.)
            if patterns.uses_indentation {
                if i > start && !lines[i].trim().is_empty() {
                    let current_indent = get_indent_level(lines[i]);
                    let start_indent = get_indent_level(lines[start]);
                    if current_indent <= start_indent && i > start + 1 {
                        return i - 1;
                    }
                }
            } else if found_open && brace_count == 0 {
                return i;
            }

            // Safety limit
            if i - start > 500 {
                return i;
            }
        }

        lines.len() - 1
    }

    fn find_import_block_end(
        &self,
        lines: &[&str],
        start: usize,
        patterns: &LanguagePatterns,
    ) -> usize {
        let mut end = start;

        for i in start..lines.len() {
            let trimmed = lines[i].trim();
            if patterns.import_pattern.is_match(trimmed) || trimmed.is_empty() {
                end = i;
            } else {
                break;
            }
        }

        end
    }

    fn find_comment_end(&self, lines: &[&str], start: usize, patterns: &LanguagePatterns) -> usize {
        let first_line = lines[start].trim();

        // Block comment
        if first_line.starts_with("/*") {
            for (i, line) in lines.iter().enumerate().skip(start) {
                if line.contains("*/") {
                    return i;
                }
            }
        }

        // Line comments
        let mut end = start;
        for i in start..lines.len() {
            let trimmed = lines[i].trim();
            if patterns.doc_comment_pattern.is_match(trimmed)
                || trimmed.starts_with("//")
                || trimmed.starts_with("#")
            {
                end = i;
            } else if !trimmed.is_empty() {
                break;
            }
        }

        end
    }

    fn split_large_block(
        &self,
        lines: &[&str],
        base_line: usize,
        chunk_type: ChunkType,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_start = 0;
        let mut current_size = 0;

        for (i, line) in lines.iter().enumerate() {
            current_size += line.len() + 1;

            if current_size >= self.max_chunk_size && i > current_start {
                let content: String = lines[current_start..i].join("\n");
                if content.len() >= self.min_chunk_size {
                    chunks.push(Chunk {
                        content,
                        start_line: base_line + current_start + 1,
                        end_line: base_line + i,
                        chunk_type: chunk_type.clone(),
                    });
                }

                // Overlap
                current_start = i.saturating_sub(self.overlap / 50);
                current_size = lines[current_start..=i].iter().map(|l| l.len() + 1).sum();
            }
        }

        // Last chunk
        if current_start < lines.len() {
            let content: String = lines[current_start..].join("\n");
            if content.len() >= self.min_chunk_size {
                chunks.push(Chunk {
                    content,
                    start_line: base_line + current_start + 1,
                    end_line: base_line + lines.len(),
                    chunk_type: chunk_type.clone(),
                });
            }
        }

        chunks
    }

    fn simple_chunk(&self, lines: &[&str]) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut current_start = 0;
        let mut current_size = 0;

        for (i, line) in lines.iter().enumerate() {
            current_size += line.len() + 1;

            if current_size >= self.max_chunk_size {
                let content: String = lines[current_start..=i].join("\n");
                chunks.push(Chunk {
                    content,
                    start_line: current_start + 1,
                    end_line: i + 1,
                    chunk_type: ChunkType::Code,
                });

                current_start = i.saturating_sub(2);
                current_size = 0;
            }
        }

        // Last chunk
        if current_start < lines.len() {
            let content: String = lines[current_start..].join("\n");
            if content.len() >= self.min_chunk_size || chunks.is_empty() {
                chunks.push(Chunk {
                    content,
                    start_line: current_start + 1,
                    end_line: lines.len(),
                    chunk_type: ChunkType::Code,
                });
            }
        }

        chunks
    }
}

struct LanguagePatterns {
    function_pattern: Regex,
    class_pattern: Regex,
    import_pattern: Regex,
    doc_comment_pattern: Regex,
    uses_indentation: bool,
}

fn get_language_patterns(language: Option<&str>) -> LanguagePatterns {
    match language {
        Some("python") => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*(async\s+)?def\s+\w+").unwrap(),
            class_pattern: Regex::new(r"^\s*class\s+\w+").unwrap(),
            import_pattern: Regex::new(r"^\s*(import|from)\s+").unwrap(),
            doc_comment_pattern: Regex::new(r#"^\s*("""|''')"#).unwrap(),
            uses_indentation: true,
        },
        Some("rust") => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*(pub\s+)?(async\s+)?fn\s+\w+").unwrap(),
            class_pattern: Regex::new(r"^\s*(pub\s+)?(struct|enum|trait|impl)\s+").unwrap(),
            import_pattern: Regex::new(r"^\s*use\s+").unwrap(),
            doc_comment_pattern: Regex::new(r"^\s*///").unwrap(),
            uses_indentation: false,
        },
        Some("go") => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*func\s+").unwrap(),
            class_pattern: Regex::new(r"^\s*type\s+\w+\s+(struct|interface)").unwrap(),
            import_pattern: Regex::new(r"^\s*import\s+").unwrap(),
            doc_comment_pattern: Regex::new(r"^\s*//").unwrap(),
            uses_indentation: false,
        },
        Some("typescript") | Some("javascript") => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*(export\s+)?(async\s+)?function\s+\w+|^\s*(const|let|var)\s+\w+\s*=\s*(async\s+)?\(").unwrap(),
            class_pattern: Regex::new(r"^\s*(export\s+)?class\s+\w+").unwrap(),
            import_pattern: Regex::new(r"^\s*import\s+").unwrap(),
            doc_comment_pattern: Regex::new(r"^\s*/\*\*").unwrap(),
            uses_indentation: false,
        },
        Some("java") | Some("kotlin") => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*(public|private|protected)?\s*(static\s+)?\w+\s+\w+\s*\(").unwrap(),
            class_pattern: Regex::new(r"^\s*(public\s+)?(abstract\s+)?class\s+\w+").unwrap(),
            import_pattern: Regex::new(r"^\s*import\s+").unwrap(),
            doc_comment_pattern: Regex::new(r"^\s*/\*\*").unwrap(),
            uses_indentation: false,
        },
        _ => LanguagePatterns {
            function_pattern: Regex::new(r"^\s*(pub\s+)?(async\s+)?(fn|function|def|func)\s+\w+").unwrap(),
            class_pattern: Regex::new(r"^\s*(pub\s+)?(class|struct|type|interface)\s+\w+").unwrap(),
            import_pattern: Regex::new(r"^\s*(import|use|from|require)\s+").unwrap(),
            doc_comment_pattern: Regex::new(r#"^\s*(///|/\*\*|#|""")"#).unwrap(),
            uses_indentation: false,
        },
    }
}

fn get_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}
