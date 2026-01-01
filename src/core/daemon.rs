//! Background daemon with file watching and debouncing
//!
//! Inspired by Pommel's architecture:
//! - Debounced file watching to batch rapid changes
//! - Background indexing that doesn't block the user
//! - Graceful shutdown handling

use anyhow::Result;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::core::fast_indexer::{FastIndexConfig, FastIndexer, IndexTier};

/// Debounce configuration
#[derive(Debug, Clone)]
pub struct DebounceConfig {
    /// Minimum time to wait after last change before processing
    pub debounce_delay: Duration,
    /// Maximum time to wait before forcing a flush
    pub max_delay: Duration,
    /// Batch size limit (force flush when reached)
    pub max_batch_size: usize,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            debounce_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            max_batch_size: 100,
        }
    }
}

/// Daemon configuration
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Path to watch
    pub watch_path: PathBuf,
    /// Store name for the index
    pub store_name: Option<String>,
    /// Debounce settings
    pub debounce: DebounceConfig,
    /// Indexing tier
    pub index_tier: IndexTier,
}

impl DaemonConfig {
    pub fn new(watch_path: impl Into<PathBuf>) -> Self {
        Self {
            watch_path: watch_path.into(),
            store_name: None,
            debounce: DebounceConfig::default(),
            index_tier: IndexTier::Balanced,
        }
    }

    pub fn with_store(mut self, name: impl Into<String>) -> Self {
        self.store_name = Some(name.into());
        self
    }

    pub fn with_tier(mut self, tier: IndexTier) -> Self {
        self.index_tier = tier;
        self
    }

    pub fn with_debounce(mut self, debounce: DebounceConfig) -> Self {
        self.debounce = debounce;
        self
    }
}

/// Debounced file change accumulator
struct ChangeAccumulator {
    /// Set of changed file paths
    changed_paths: HashSet<PathBuf>,
    /// Time of first change in current batch
    first_change: Option<Instant>,
    /// Time of last change
    last_change: Option<Instant>,
}

impl ChangeAccumulator {
    fn new() -> Self {
        Self {
            changed_paths: HashSet::new(),
            first_change: None,
            last_change: None,
        }
    }

    fn add(&mut self, path: PathBuf) {
        let now = Instant::now();
        if self.first_change.is_none() {
            self.first_change = Some(now);
        }
        self.last_change = Some(now);
        self.changed_paths.insert(path);
    }

    fn should_flush(&self, config: &DebounceConfig) -> bool {
        let now = Instant::now();

        // Check if we've exceeded max batch size
        if self.changed_paths.len() >= config.max_batch_size {
            return true;
        }

        // Check if we've exceeded max delay since first change
        if let Some(first) = self.first_change {
            if now.duration_since(first) >= config.max_delay {
                return true;
            }
        }

        // Check if debounce delay has passed since last change
        if let Some(last) = self.last_change {
            if now.duration_since(last) >= config.debounce_delay {
                return true;
            }
        }

        false
    }

    fn take(&mut self) -> Vec<PathBuf> {
        self.first_change = None;
        self.last_change = None;
        self.changed_paths.drain().collect()
    }

    fn is_empty(&self) -> bool {
        self.changed_paths.is_empty()
    }
}

/// Background indexing daemon
pub struct Daemon {
    config: DaemonConfig,
    running: Arc<AtomicBool>,
}

impl Daemon {
    pub fn new(config: DaemonConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if daemon is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Stop the daemon
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Run the daemon (blocking)
    pub async fn run(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        let watch_path = std::fs::canonicalize(&self.config.watch_path)?;
        let path_str = watch_path.to_string_lossy().to_string();

        // Initial index
        eprintln!("[daemon] Initial indexing: {}", path_str);
        self.do_index(&path_str).await?;
        eprintln!("[daemon] Initial indexing complete");

        // Set up file watcher
        let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
        let running = self.running.clone();

        let _watcher = {
            let tx = tx.clone();
            let mut watcher = RecommendedWatcher::new(
                move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(event) = res {
                        use notify::EventKind;
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                for path in event.paths {
                                    let _ = tx.send(path);
                                }
                            }
                            _ => {}
                        }
                    }
                },
                NotifyConfig::default().with_poll_interval(Duration::from_secs(1)),
            )?;

            watcher.watch(Path::new(&path_str), RecursiveMode::Recursive)?;
            watcher
        };

        eprintln!("[daemon] Watching for changes...");

        // Process changes with debouncing
        let accumulator = Arc::new(Mutex::new(ChangeAccumulator::new()));
        let debounce_config = self.config.debounce.clone();

        while running.load(Ordering::SeqCst) {
            // Check for new changes (non-blocking with timeout)
            tokio::select! {
                Some(path) = rx.recv() => {
                    // Skip hidden files and directories
                    let path_str = path.to_string_lossy();
                    if path_str.contains("/.") || path_str.contains("\\.") {
                        continue;
                    }

                    let mut acc = accumulator.lock().unwrap();
                    acc.add(path);
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Check if we should flush
                }
            }

            // Check if we should flush accumulated changes
            let should_flush = {
                let acc = accumulator.lock().unwrap();
                !acc.is_empty() && acc.should_flush(&debounce_config)
            };

            if should_flush {
                let paths = {
                    let mut acc = accumulator.lock().unwrap();
                    acc.take()
                };

                eprintln!("[daemon] Processing {} changed files", paths.len());

                // Re-index the directory
                if let Err(e) = self.do_index(&path_str).await {
                    eprintln!("[daemon] Indexing error: {}", e);
                } else {
                    eprintln!("[daemon] Indexing complete");
                }
            }
        }

        eprintln!("[daemon] Shutting down");
        Ok(())
    }

    /// Perform indexing
    async fn do_index(&self, path: &str) -> Result<()> {
        let config = FastIndexConfig {
            tier: self.config.index_tier,
            incremental: true,
            ..Default::default()
        };

        let indexer = FastIndexer::new(config)?;
        let result = indexer
            .index(path, self.config.store_name.as_deref())
            .await?;

        eprintln!(
            "[daemon] Indexed {} files ({} chunks) in {}ms",
            result.indexed_files, result.total_chunks, result.duration_ms
        );

        Ok(())
    }
}

/// Start daemon in background thread
pub fn spawn_daemon(config: DaemonConfig) -> Result<DaemonHandle> {
    let daemon = Arc::new(Daemon::new(config));
    let daemon_clone = daemon.clone();

    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(async {
            if let Err(e) = daemon_clone.run().await {
                eprintln!("[daemon] Error: {}", e);
            }
        });
    });

    Ok(DaemonHandle {
        daemon,
        thread: Some(handle),
    })
}

/// Handle to a running daemon
pub struct DaemonHandle {
    daemon: Arc<Daemon>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DaemonHandle {
    /// Check if daemon is running
    pub fn is_running(&self) -> bool {
        self.daemon.is_running()
    }

    /// Stop the daemon and wait for it to finish
    pub fn stop(mut self) -> Result<()> {
        self.daemon.stop();
        if let Some(handle) = self.thread.take() {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("Daemon thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        self.daemon.stop();
        // Don't wait for thread in drop to avoid blocking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debounce_config_default() {
        let config = DebounceConfig::default();
        assert_eq!(config.debounce_delay, Duration::from_millis(500));
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(config.max_batch_size, 100);
    }

    #[test]
    fn test_change_accumulator() {
        let mut acc = ChangeAccumulator::new();
        assert!(acc.is_empty());

        acc.add(PathBuf::from("/test/file1.rs"));
        acc.add(PathBuf::from("/test/file2.rs"));
        assert!(!acc.is_empty());
        assert_eq!(acc.changed_paths.len(), 2);

        let paths = acc.take();
        assert_eq!(paths.len(), 2);
        assert!(acc.is_empty());
    }

    #[test]
    fn test_daemon_config_builder() {
        let config = DaemonConfig::new("/test/path")
            .with_store("my-store")
            .with_tier(IndexTier::Fast);

        assert_eq!(config.watch_path, PathBuf::from("/test/path"));
        assert_eq!(config.store_name, Some("my-store".to_string()));
        assert_eq!(config.index_tier, IndexTier::Fast);
    }
}
