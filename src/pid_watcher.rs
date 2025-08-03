// src/pid_watcher.rs
//! OBINexus PID Watcher Implementation
//! Updated for notify 6.1 API compatibility

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{
    Config, Event, EventKind, PollWatcher, RecursiveMode, Result as NotifyResult, Watcher,
};
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::dimensional_cache::{CacheBustSeverity, DimensionalCacheManager};
use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone)]
pub struct BustCallConfig {
    pub watch_paths: Vec<PathBuf>,
    pub poll_interval: Duration,
    pub debounce_duration: Duration,
    pub max_events_per_second: u32,
    pub auto_restart: bool,
    pub cache_bust_threshold: f64,
}

impl Default for BustCallConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![],
            poll_interval: Duration::from_millis(500),
            debounce_duration: Duration::from_millis(200),
            max_events_per_second: 100,
            auto_restart: bool,
            cache_bust_threshold: 0.7,
        }
    }
}

pub struct BustCallDaemon {
    config: BustCallConfig,
    watcher: Option<PollWatcher>,
    event_tx: Option<mpsc::Sender<Event>>,
    is_running: Arc<Mutex<bool>>,
    cache_manager: DimensionalCacheManager,
    event_history: Arc<Mutex<Vec<(Instant, EventKind)>>>,
}

impl BustCallDaemon {
    pub fn new(config: BustCallConfig) -> Result<Self> {
        let cache_manager = DimensionalCacheManager::new()
            .map_err(|e| BustcallError::PidWatcherError(format!("Cache manager init failed: {}", e)))?;

        Ok(Self {
            config,
            watcher: None,
            event_tx: None,
            is_running: Arc::new(Mutex::new(false)),
            cache_manager,
            event_history: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        if *self.is_running.lock().unwrap() {
            return Err(BustcallError::PidWatcherError(
                "Daemon already running".to_string(),
            ));
        }

        let (event_tx, mut event_rx) = mpsc::channel::<Event>(1000);
        self.event_tx = Some(event_tx.clone());

        // Create watcher with updated notify API
        let mut watcher = PollWatcher::new(
            move |result: NotifyResult<Event>| {
                if let Ok(event) = result {
                    let _ = event_tx.try_send(event);
                } else if let Err(e) = result {
                    log::error!("File watcher error: {:?}", e);
                }
            },
            Config::default().with_poll_interval(self.config.poll_interval),
        )
        .map_err(|e| BustcallError::PidWatcherError(format!("Watcher creation failed: {}", e)))?;

        // Register watch paths
        for path in &self.config.watch_paths {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| {
                    BustcallError::PidWatcherError(format!(
                        "Failed to watch path {}: {}",
                        path.display(),
                        e
                    ))
                })?;
        }

        self.watcher = Some(watcher);
        *self.is_running.lock().unwrap() = true;

        // Spawn event processing task
        let is_running = self.is_running.clone();
        let cache_manager = self.cache_manager.clone();
        let event_history = self.event_history.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut debounce_buffer: HashMap<PathBuf, (Instant, EventKind)> = HashMap::new();
            let mut last_cleanup = Instant::now();

            while *is_running.lock().unwrap() {
                tokio::select! {
                    Some(event) = event_rx.recv() => {
                        if let Err(e) = Self::process_event(
                            event,
                            &mut debounce_buffer,
                            &cache_manager,
                            &event_history,
                            &config,
                        ).await {
                            log::error!("Event processing failed: {}", e);
                        }
                    }
                    _ = sleep(Duration::from_secs(1)) => {
                        // Periodic cleanup and debounce processing
                        if last_cleanup.elapsed() > Duration::from_secs(5) {
                            Self::cleanup_debounce_buffer(&mut debounce_buffer, &config);
                            Self::cleanup_event_history(&event_history);
                            last_cleanup = Instant::now();
                        }
                    }
                }
            }
        });

        log::info!("🚀 BustCall daemon started, watching {} paths", self.config.watch_paths.len());
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        *self.is_running.lock().unwrap() = false;
        self.watcher = None;
        self.event_tx = None;
        log::info!("⏹️ BustCall daemon stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    async fn process_event(
        event: Event,
        debounce_buffer: &mut HashMap<PathBuf, (Instant, EventKind)>,
        cache_manager: &DimensionalCacheManager,
        event_history: &Arc<Mutex<Vec<(Instant, EventKind)>>>,
        config: &BustCallConfig,
    ) -> Result<()> {
        let now = Instant::now();
        
        // Update event history for rate limiting
        {
            let mut history = event_history.lock().unwrap();
            history.push((now, event.kind.clone()));
        }

        // Check rate limiting
        if Self::should_rate_limit(event_history, config) {
            log::warn!("⚠️ Rate limiting file events - too many events per second");
            return Ok(());
        }

        // Process each path in the event
        for path in event.paths {
            // Debounce logic
            if let Some((last_time, _)) = debounce_buffer.get(&path) {
                if now.duration_since(*last_time) < config.debounce_duration {
                    continue; // Skip debounced events
                }
            }

            debounce_buffer.insert(path.clone(), (now, event.kind.clone()));

            // Determine cache bust severity based on file type and event
            let severity = Self::determine_cache_severity(&path, &event.kind, config);
            
            if let Some(severity) = severity {
                let target_name = Self::extract_target_name(&path);
                
                log::info!("📁 Cache bust triggered: {} ({:?}) -> {:?}", 
                    path.display(), event.kind, severity);
                
                cache_manager
                    .bust_cache(&target_name, severity)
                    .map_err(|e| BustcallError::PidWatcherError(format!("Cache bust failed: {}", e)))?;
            }
        }

        Ok(())
    }

    fn should_rate_limit(
        event_history: &Arc<Mutex<Vec<(Instant, EventKind)>>>,
        config: &BustCallConfig,
    ) -> bool {
        let history = event_history.lock().unwrap();
        let one_second_ago = Instant::now() - Duration::from_secs(1);
        
        let recent_events = history
            .iter()
            .filter(|(time, _)| *time > one_second_ago)
            .count();
        
        recent_events > config.max_events_per_second as usize
    }

    fn determine_cache_severity(
        path: &PathBuf,
        event_kind: &EventKind,
        config: &BustCallConfig,
    ) -> Option<CacheBustSeverity> {
        let extension = path.extension()?.to_str()?;
        let is_critical_file = matches!(
            extension,
            "rs" | "go" | "c" | "cpp" | "h" | "py" | "js" | "ts" | "toml" | "yaml" | "json"
        );

        match event_kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                if is_critical_file {
                    Some(CacheBustSeverity::Medium)
                } else {
                    Some(CacheBustSeverity::Low)
                }
            }
            EventKind::Remove(_) => {
                if is_critical_file {
                    Some(CacheBustSeverity::High)
                } else {
                    Some(CacheBustSeverity::Medium)
                }
            }
            _ => None,
        }
    }

    fn extract_target_name(path: &PathBuf) -> String {
        // Extract target name from path components
        if let Some(parent) = path.parent() {
            if let Some(dir_name) = parent.file_name() {
                if let Some(name) = dir_name.to_str() {
                    // Map common directory names to target names
                    return match name {
                        "node_modules" => "node".to_string(),
                        "venv" | "__pycache__" => "python".to_string(),
                        "target" => "rust".to_string(),
                        "bin" | "build" => "c".to_string(),
                        "gosi" => "gosilang".to_string(),
                        _ => name.to_string(),
                    };
                }
            }
        }
        
        "generic".to_string()
    }

    fn cleanup_debounce_buffer(
        buffer: &mut HashMap<PathBuf, (Instant, EventKind)>,
        config: &BustCallConfig,
    ) {
        let cutoff = Instant::now() - config.debounce_duration * 10;
        buffer.retain(|_, (time, _)| *time > cutoff);
    }

    fn cleanup_event_history(event_history: &Arc<Mutex<Vec<(Instant, EventKind)>>>) {
        let mut history = event_history.lock().unwrap();
        let cutoff = Instant::now() - Duration::from_secs(60);
        history.retain(|(time, _)| *time > cutoff);
    }

    pub fn add_watch_path(&mut self, path: PathBuf) -> Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|e| {
                    BustcallError::PidWatcherError(format!(
                        "Failed to add watch path {}: {}",
                        path.display(),
                        e
                    ))
                })?;
        }
        
        self.config.watch_paths.push(path);
        Ok(())
    }

    pub fn remove_watch_path(&mut self, path: &PathBuf) -> Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher.unwatch(path).map_err(|e| {
                BustcallError::PidWatcherError(format!(
                    "Failed to remove watch path {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }
        
        self.config.watch_paths.retain(|p| p != path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_daemon_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = BustCallConfig {
            watch_paths: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        };

        let mut daemon = BustCallDaemon::new(config).unwrap();
        assert!(!daemon.is_running());

        daemon.start().await.unwrap();
        assert!(daemon.is_running());

        daemon.stop().unwrap();
        assert!(!daemon.is_running());
    }

    #[test]
    fn test_target_name_extraction() {
        let path = PathBuf::from("/project/node_modules/package/index.js");
        let target = BustCallDaemon::extract_target_name(&path);
        assert_eq!(target, "node");

        let path = PathBuf::from("/project/venv/lib/python3.9/site-packages/module.py");
        let target = BustCallDaemon::extract_target_name(&path);
        assert_eq!(target, "python");
    }

    #[test]
    fn test_cache_severity_determination() {
        let config = BustCallConfig::default();
        
        let rs_file = PathBuf::from("src/main.rs");
        let severity = BustCallDaemon::determine_cache_severity(
            &rs_file,
            &EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content
            )),
            &config,
        );
        assert_eq!(severity, Some(CacheBustSeverity::Medium));
        
        let txt_file = PathBuf::from("README.txt");
        let severity = BustCallDaemon::determine_cache_severity(
            &txt_file,
            &EventKind::Create(notify::event::CreateKind::File),
            &config,
        );
        assert_eq!(severity, Some(CacheBustSeverity::Low));
    }
}