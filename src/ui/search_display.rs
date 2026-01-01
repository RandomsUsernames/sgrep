//! Beautiful search results display
//!
//! Aesthetic: Cyberpunk Terminal + Brutalist Typography
//! - Geometric frames with neon accents
//! - Score visualization bars
//! - Syntax-highlighted code previews
//! - Animated loading states

use colored::Colorize;
use std::path::Path;

use super::theme::{BoxChars, Theme};
use crate::core::search::SearchResult;

/// Display search results with beautiful formatting
pub fn display_results(query: &str, results: &[SearchResult], show_content: bool) {
    let term_width = terminal_width();

    // Header
    print_header(query, results.len(), term_width);

    if results.is_empty() {
        print_empty_state(term_width);
        return;
    }

    // Results
    for (i, result) in results.iter().enumerate() {
        print_result(i + 1, result, show_content, term_width);
    }

    // Footer
    print_footer(results.len(), term_width);
}

/// Print the search header with query
fn print_header(query: &str, count: usize, width: usize) {
    println!();

    // Top border with accent
    let border = BoxChars::H_LINE.repeat(width.saturating_sub(2));
    println!(
        "{}{}{}",
        BoxChars::TL_CORNER.color(Theme::BORDER_ACCENT),
        border.color(Theme::BORDER),
        BoxChars::TR_CORNER.color(Theme::BORDER_ACCENT)
    );

    // Title line
    let title = format!(" {} SEARCHGREP ", BoxChars::DIAMOND);
    let query_display = format!("\"{}\"", truncate_str(query, 40));
    let count_display = format!("{} matches", count);

    let padding = width.saturating_sub(title.len() + query_display.len() + count_display.len() + 6);

    println!(
        "{} {}{}{}{} {}",
        BoxChars::V_LINE.color(Theme::BORDER_ACCENT),
        title.color(Theme::NEON_CYAN).bold(),
        query_display.color(Theme::NEON_MAGENTA),
        " ".repeat(padding),
        count_display.color(Theme::SUBTLE),
        BoxChars::V_LINE.color(Theme::BORDER_ACCENT)
    );

    // Separator
    println!(
        "{}{}{}",
        BoxChars::T_RIGHT.color(Theme::BORDER_ACCENT),
        BoxChars::H_LINE
            .repeat(width.saturating_sub(2))
            .color(Theme::BORDER),
        BoxChars::T_LEFT.color(Theme::BORDER_ACCENT)
    );
}

/// Print a single search result
fn print_result(index: usize, result: &SearchResult, show_content: bool, width: usize) {
    let score = result.score;
    let score_pct = (score * 100.0) as u32;
    let file_path = &result.chunk.file_path;
    let start_line = result.chunk.start_line;
    let end_line = result.chunk.end_line;
    let lang = result.chunk.language.as_deref().unwrap_or("text");

    // Extract just the filename and parent dir for display
    let display_path = shorten_path(file_path, 50);

    // Index badge
    let index_badge = format!(" {:>2} ", index);

    // Score bar (8 chars)
    let score_bar = Theme::score_bar(score, 8);
    let score_text = format!("{:>3}%", score_pct);

    // Language badge
    let lang_badge = format!(" {} ", lang.to_uppercase());
    let lang_color = Theme::lang_color(lang);

    // Line range
    let line_range = format!("L{}-{}", start_line, end_line);

    // Main result line
    println!(
        "{} {}{}  {}  {}  {}  {}",
        BoxChars::V_LINE.color(Theme::BORDER),
        index_badge
            .on_color(Theme::BORDER_ACCENT)
            .color(colored::Color::Black)
            .bold(),
        score_bar,
        score_text.color(Theme::score_color(score)).bold(),
        display_path.color(Theme::NEON_CYAN),
        line_range.color(Theme::SUBTLE),
        lang_badge.on_color(lang_color).color(colored::Color::Black)
    );

    // Content preview if enabled
    if show_content {
        print_content_preview(&result.chunk.content, width, lang);
    }

    // Subtle separator between results
    if !show_content {
        println!(
            "{} {}",
            BoxChars::V_LINE.color(Theme::BORDER),
            BoxChars::L_H_LINE
                .repeat(width.saturating_sub(4))
                .color(Theme::DIM)
        );
    }
}

/// Print syntax-highlighted code preview
fn print_content_preview(content: &str, width: usize, lang: &str) {
    let preview_lines: Vec<&str> = content.lines().take(6).collect();
    let lang_color = Theme::lang_color(lang);

    // Preview box top
    println!(
        "{} {}{}{}",
        BoxChars::V_LINE.color(Theme::BORDER),
        BoxChars::L_TL.color(Theme::DIM),
        BoxChars::L_H_LINE
            .repeat(width.saturating_sub(6))
            .color(Theme::DIM),
        BoxChars::L_TR.color(Theme::DIM)
    );

    for line in &preview_lines {
        let trimmed = truncate_str(line, width.saturating_sub(8));
        // Simple syntax highlighting based on common patterns
        let highlighted = highlight_code(trimmed, lang);
        println!(
            "{} {} {} {}",
            BoxChars::V_LINE.color(Theme::BORDER),
            BoxChars::L_V_LINE.color(Theme::DIM),
            highlighted,
            BoxChars::L_V_LINE.color(Theme::DIM)
        );
    }

    if content.lines().count() > 6 {
        let more = format!("... +{} more lines", content.lines().count() - 6);
        println!(
            "{} {} {} {}",
            BoxChars::V_LINE.color(Theme::BORDER),
            BoxChars::L_V_LINE.color(Theme::DIM),
            more.color(Theme::SUBTLE).italic(),
            BoxChars::L_V_LINE.color(Theme::DIM)
        );
    }

    // Preview box bottom
    println!(
        "{} {}{}{}",
        BoxChars::V_LINE.color(Theme::BORDER),
        BoxChars::L_BL.color(Theme::DIM),
        BoxChars::L_H_LINE
            .repeat(width.saturating_sub(6))
            .color(Theme::DIM),
        BoxChars::L_BR.color(Theme::DIM)
    );
}

/// Simple syntax highlighting for code
fn highlight_code(line: &str, lang: &str) -> String {
    // Keywords to highlight based on language
    let keywords: &[&str] = match lang.to_lowercase().as_str() {
        "rust" | "rs" => &[
            "fn", "let", "mut", "pub", "struct", "impl", "use", "mod", "async", "await", "match",
            "if", "else", "for", "while", "return", "Self", "self",
        ],
        "typescript" | "javascript" | "ts" | "js" => &[
            "function",
            "const",
            "let",
            "var",
            "async",
            "await",
            "class",
            "interface",
            "type",
            "import",
            "export",
            "return",
            "if",
            "else",
            "for",
            "while",
        ],
        "python" | "py" => &[
            "def", "class", "import", "from", "return", "if", "else", "elif", "for", "while",
            "async", "await", "with", "as", "try", "except",
        ],
        "go" => &[
            "func",
            "var",
            "const",
            "type",
            "struct",
            "interface",
            "import",
            "package",
            "return",
            "if",
            "else",
            "for",
            "range",
            "go",
            "defer",
        ],
        _ => &[],
    };

    let mut result = line.to_string();

    // Highlight strings (simple approach)
    if result.contains('"') {
        // This is a simplified approach - real syntax highlighting would be more complex
        result = result.color(Theme::SUBTLE).to_string();
    }

    // For now, just return dimmed - full syntax highlighting would need a proper lexer
    line.color(Theme::SUBTLE).to_string()
}

/// Print empty state when no results
fn print_empty_state(width: usize) {
    println!(
        "{} {} {}",
        BoxChars::V_LINE.color(Theme::BORDER),
        "No matches found".color(Theme::SCORE_LOW).italic(),
        BoxChars::V_LINE.color(Theme::BORDER)
    );
}

/// Print the footer
fn print_footer(count: usize, width: usize) {
    // Bottom border
    let border = BoxChars::H_LINE.repeat(width.saturating_sub(2));
    println!(
        "{}{}{}",
        BoxChars::BL_CORNER.color(Theme::BORDER_ACCENT),
        border.color(Theme::BORDER),
        BoxChars::BR_CORNER.color(Theme::BORDER_ACCENT)
    );

    // Stats line
    let stats = format!(
        " {} Found {} results {} sgrep v0.1.0 ",
        BoxChars::CHECK,
        count,
        BoxChars::BULLET
    );
    println!("{}", stats.color(Theme::SUBTLE));
    println!();
}

/// Display loading animation
pub fn display_loading(message: &str) {
    use super::theme::Spinner;
    print!(
        "\r{} {} {}",
        Spinner::FRAMES[0].color(Theme::NEON_CYAN),
        message.color(Theme::SUBTLE),
        "   " // Clear any previous longer text
    );
}

/// Get terminal width, default to 80
fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
        .max(60)
}

/// Truncate string with ellipsis
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len.saturating_sub(3)]
    }
}

/// Shorten path for display
fn shorten_path(path: &str, max_len: usize) -> String {
    let p = Path::new(path);

    // Get filename and parent
    let filename = p.file_name().and_then(|f| f.to_str()).unwrap_or(path);

    let parent = p
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if parent.is_empty() {
        filename.to_string()
    } else {
        let short = format!("{}/{}", parent, filename);
        if short.len() > max_len {
            format!(".../{}", filename)
        } else {
            short
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_path() {
        let path = "/home/user/projects/myapp/src/main.rs";
        let short = shorten_path(path, 30);
        assert!(short.len() <= 30);
    }
}
