//! Animated progress indicators
//!
//! Beautiful loading states with:
//! - Spinning indicators
//! - Progress bars
//! - Streaming text effects

use colored::Colorize;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::theme::{BoxChars, Spinner, Theme};

/// Progress bar for indexing operations
pub struct ProgressBar {
    total: usize,
    current: usize,
    width: usize,
    label: String,
}

impl ProgressBar {
    pub fn new(total: usize, label: &str) -> Self {
        Self {
            total,
            current: 0,
            width: 30,
            label: label.to_string(),
        }
    }

    pub fn set(&mut self, current: usize) {
        self.current = current.min(self.total);
        self.render();
    }

    pub fn inc(&mut self) {
        self.set(self.current + 1);
    }

    fn render(&self) {
        let pct = if self.total > 0 {
            (self.current as f32 / self.total as f32).min(1.0)
        } else {
            0.0
        };

        let filled = (pct * self.width as f32) as usize;
        let empty = self.width - filled;

        let bar = format!(
            "{}{}",
            "█".repeat(filled).color(Theme::NEON_CYAN),
            "░".repeat(empty).color(Theme::DIM)
        );

        let percent = format!("{:>3}%", (pct * 100.0) as u32);
        let count = format!("{}/{}", self.current, self.total);

        print!(
            "\r{} {} {} {} {} {}",
            BoxChars::ARROW_RIGHT.color(Theme::NEON_MAGENTA),
            self.label.color(Theme::SUBTLE),
            bar,
            percent.color(Theme::score_color(pct)).bold(),
            count.color(Theme::SUBTLE),
            " ".repeat(10) // Clear trailing chars
        );
        io::stdout().flush().ok();
    }

    pub fn finish(&self) {
        println!(
            "\r{} {} {}",
            BoxChars::CHECK.color(Theme::NEON_GREEN),
            self.label.color(Theme::SUBTLE),
            format!("Done ({} items)", self.total).color(Theme::SUBTLE)
        );
    }
}

/// Animated spinner for indeterminate operations
pub struct AnimatedSpinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl AnimatedSpinner {
    pub fn new(message: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = thread::spawn(move || {
            let mut frame = 0;
            while running_clone.load(Ordering::Relaxed) {
                let spinner = Spinner::FRAMES[frame % Spinner::FRAMES.len()];
                print!(
                    "\r{} {} {}",
                    spinner.color(Theme::NEON_CYAN),
                    message.color(Theme::SUBTLE),
                    " ".repeat(10)
                );
                io::stdout().flush().ok();
                frame += 1;
                thread::sleep(Duration::from_millis(80));
            }
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    pub fn finish(mut self, success_message: &str) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
        println!(
            "\r{} {}{}",
            BoxChars::CHECK.color(Theme::NEON_GREEN),
            success_message.color(Theme::SUBTLE),
            " ".repeat(20)
        );
    }

    pub fn fail(mut self, error_message: &str) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
        println!(
            "\r{} {}{}",
            BoxChars::CROSS_MARK.color(Theme::SCORE_LOW),
            error_message.color(Theme::SCORE_LOW),
            " ".repeat(20)
        );
    }
}

impl Drop for AnimatedSpinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Streaming search animation
pub struct SearchAnimation {
    query: String,
}

impl SearchAnimation {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
        }
    }

    pub fn start(&self) {
        // Print search header
        println!();
        println!(
            "{} {} {}",
            BoxChars::DIAMOND.color(Theme::NEON_CYAN),
            "Searching".color(Theme::SUBTLE),
            format!("\"{}\"", self.query).color(Theme::NEON_MAGENTA)
        );
    }

    pub fn update_stage(&self, stage: &str) {
        print!(
            "\r  {} {}{}",
            BoxChars::ARROW_RIGHT.color(Theme::BORDER_ACCENT),
            stage.color(Theme::SUBTLE),
            " ".repeat(40)
        );
        io::stdout().flush().ok();
    }

    pub fn finish(&self, result_count: usize, duration_ms: u128) {
        println!(
            "\r  {} Found {} matches in {}ms{}",
            BoxChars::CHECK.color(Theme::NEON_GREEN),
            result_count.to_string().color(Theme::NEON_CYAN).bold(),
            duration_ms.to_string().color(Theme::SUBTLE),
            " ".repeat(20)
        );
    }
}

/// Hybrid model status display
pub struct HybridModelStatus;

impl HybridModelStatus {
    pub fn show_loading() {
        println!(
            "\n{} {} {}",
            BoxChars::TL_CORNER.color(Theme::NEON_MAGENTA),
            BoxChars::H_LINE.repeat(40).color(Theme::BORDER),
            BoxChars::TR_CORNER.color(Theme::NEON_MAGENTA)
        );
        println!(
            "{} {} HYBRID FUSION MODEL {} {}",
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA),
            BoxChars::STAR.color(Theme::NEON_CYAN),
            BoxChars::STAR.color(Theme::NEON_CYAN),
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA)
        );
    }

    pub fn show_model_loading(model_name: &str, index: usize) {
        let bullet = if index == 0 { "┌" } else { "├" };
        print!(
            "\r{} {} Loading {}...{}",
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA),
            bullet.color(Theme::DIM),
            model_name.color(Theme::NEON_CYAN),
            " ".repeat(20)
        );
        io::stdout().flush().ok();
    }

    pub fn show_model_ready(model_name: &str, index: usize, is_last: bool) {
        let bullet = if is_last { "└" } else { "├" };
        println!(
            "\r{} {} {} {}{}",
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA),
            bullet.color(Theme::DIM),
            BoxChars::CHECK.color(Theme::NEON_GREEN),
            model_name.color(Theme::SUBTLE),
            " ".repeat(20)
        );
    }

    pub fn show_fusion_ready() {
        println!(
            "{} {} FUSION READY {} {}",
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA),
            BoxChars::ARROW_RIGHT.color(Theme::NEON_GREEN),
            BoxChars::ARROW_RIGHT.color(Theme::NEON_GREEN),
            BoxChars::V_LINE.color(Theme::NEON_MAGENTA)
        );
        println!(
            "{} {} {}",
            BoxChars::BL_CORNER.color(Theme::NEON_MAGENTA),
            BoxChars::H_LINE.repeat(40).color(Theme::BORDER),
            BoxChars::BR_CORNER.color(Theme::NEON_MAGENTA)
        );
    }
}
