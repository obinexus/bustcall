// src/pid_watcher.rs
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use crate::dimensional_cache::{DimensionalCacheManager, ModelBinding, CacheBustSeverity};

#[derive(Debug, Deserialize, Serialize)]
pub struct BustCallConfig {
    pub global: GlobalConfig,
    pub target: HashMap<String, TargetConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobalConfig {
    pub self_healing: bool,
    pub supervisor_mode: bool,
    pub default_max_retries: u32,
    pub daemon_interval_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TargetConfig {
    pub path: String,
    pub runtime: String,
    pub pid_watch: bool,
    pub enabled: bool,
    pub language_priority: Option<f32>,
    pub dependency_impact: Option<f32>,
    pub build_cost: Option<f32>,
    pub critical_path: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWatcher {
    pub target_name: String,
    pub config: TargetConfig,
    pub current_pid: Option<u32>,
    pub last_file_hash: Option<String>,
}

pub struct BustCallDaemon {
    config: BustCallConfig,
    watchers: HashMap<String, RuntimeWatcher>,
    cache_manager: Arc<DimensionalCacheManager>,
    daemon_running: Arc<Mutex<bool>>,
}

impl BustCallDaemon {
    /// Initialize daemon with TOML configuration
    pub fn new(config_path: &str) -> Result<Self> {
        let config_content = fs::read_to_string(config_path)
            .context("Failed to read bustcall.config.toml")?;
        
        let config: BustCallConfig = toml::from_str(&config_content)
            .context("Failed to parse TOML configuration")?;
        
        let cache_manager = Arc::new(DimensionalCacheManager::new()?);
        let mut watchers = HashMap::new();
        
        // Initialize watchers for each enabled target
        for (target_name, target_config) in &config.target {
            if target_config.enabled {
                let watcher = RuntimeWatcher {
                    target_name: target_name.clone(),
                    config: target_config.clone(),
                    current_pid: None,
                    last_file_hash: None,
                };
                watchers.insert(target_name.clone(), watcher);
                
                // Register model binding with dimensional cache
                let binding = ModelBinding {
                    runtime: target_config.runtime.clone(),
                    pid: None,
                    path: target_config.path.clone(),
                    last_modified: 0,
                    cache_dependencies: Vec::new(),
                };
                
                cache_manager.bind_model(target_name, binding)?;
            }
        }
        
        Ok(BustCallDaemon {
            config,
            watchers,
            cache_manager,
            daemon_running: Arc::new(Mutex::new(false)),
        })
    }
    
    /// Start daemon in background mode
    pub fn start_daemon(&mut self) -> Result<()> {
        {
            let mut running = self.daemon_running.lock().unwrap();
            if *running {
                log::warn!("⚠️ Daemon already running");
                return Ok(());
            }
            *running = true;
        }
        
        log::info!("🚀 Starting bustcall daemon with {} targets", self.watchers.len());
        
        // Spawn threads for each target
        for (target_name, watcher) in &self.watchers {
            self.spawn_target_watcher(target_name.clone(), watcher.clone())?;
        }
        
        // Main daemon supervision loop
        self.supervision_loop()?;
        
        Ok(())
    }
    
    /// Spawn individual watcher thread for target
    fn spawn_target_watcher(&self, target_name: String, mut watcher: RuntimeWatcher) -> Result<()> {
        let cache_manager = Arc::clone(&self.cache_manager);
        let daemon_running = Arc::clone(&self.daemon_running);
        let interval = Duration::from_secs(self.config.global.daemon_interval_seconds);
        
        // File system watcher thread
        if Path::new(&watcher.config.path).exists() {
            let path_target_name = target_name.clone();
            let path_cache_manager = Arc::clone(&cache_manager);
            let watch_path = PathBuf::from(watcher.config.path.clone());
            
            thread::spawn(move || {
                if let Err(e) = Self::watch_filesystem(&path_target_name, watch_path, path_cache_manager) {
                    log::error!("📂 Filesystem watcher error for {}: {}", path_target_name, e);
                }
            });
        }
        
        // PID monitoring thread
        if watcher.config.pid_watch {
            let pid_target_name = target_name.clone();
            let pid_cache_manager = Arc::clone(&cache_manager);
            let runtime = watcher.config.runtime.clone();
            
            thread::spawn(move || {
                while *daemon_running.lock().unwrap() {
                    if let Err(e) = Self::monitor_pid(&pid_target_name, &runtime, &pid_cache_manager, &mut watcher) {
                        log::error!("🔍 PID monitor error for {}: {}", pid_target_name, e);
                    }
                    thread::sleep(interval);
                }
            });
        }
        
        log::info!("👀 Spawned watchers for target: {}", target_name);
        Ok(())
    }
    
    /// File system change monitoring
    fn watch_filesystem(target_name: &str, path: PathBuf, cache_manager: Arc<DimensionalCacheManager>) -> Result<()> {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(2))?;
        watcher.watch(&path, RecursiveMode::Recursive)?;
        
        log::info!("📂 Watching filesystem: {} at {:?}", target_name, path);
        
        loop {
            match rx.recv() {
                Ok(event) => match event {
                    DebouncedEvent::Write(ref path) | DebouncedEvent::Create(ref path) => {
                        log::info!("📝 File change detected: {:?} in target {}", path, target_name);
                        
                        // Calculate change severity based on file type
                        let severity = Self::assess_file_change_severity(path);
                        
                        if let Err(e) = cache_manager.bust_cache(target_name, severity) {
                            log::error!("💥 Cache bust failed for {}: {}", target_name, e);
                        }
                    }
                    DebouncedEvent::Remove(ref path) => {
                        log::warn!("🗑️ File deletion detected: {:?} in target {}", path, target_name);
                        cache_manager.bust_cache(target_name, CacheBustSeverity::High)?;
                    }
                    _ => {}
                },
                Err(e) => {
                    log::error!("📂 Filesystem watch error: {:?}", e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// PID monitoring with change detection
    fn monitor_pid(
        target_name: &str, 
        runtime: &str, 
        cache_manager: &Arc<DimensionalCacheManager>, 
        watcher: &mut RuntimeWatcher
    ) -> Result<()> {
        let current_pid = Self::get_runtime_pid(runtime);
        
        if watcher.current_pid != current_pid {
            log::info!("🔄 PID change detected for {}: {:?} -> {:?}", 
                      target_name, watcher.current_pid, current_pid);
            
            // Notify cache manager of PID change
            cache_manager.monitor_pid_changes(target_name, watcher.current_pid, current_pid)?;
            
            watcher.current_pid = current_pid;
            
            // PID death/restart triggers cache bust
            if current_pid.is_none() {
                cache_manager.bust_cache(target_name, CacheBustSeverity::High)?;
            } else if watcher.current_pid.is_some() {
                // PID restart - moderate bust for rebinding
                cache_manager.bust_cache(target_name, CacheBustSeverity::Medium)?;
            }
        }
        
        Ok(())
    }
    
    /// Get PID of running process by name
    fn get_runtime_pid(runtime: &str) -> Option<u32> {
        let output = Command::new("pgrep")
            .arg("-f")  // Full command line match
            .arg(runtime)
            .output()
            .ok()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .next()?
            .parse::<u32>()
            .ok()
    }
    
    /// Assess severity of file changes for cache busting prioritization
    fn assess_file_change_severity(path: &Path) -> CacheBustSeverity {
        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            match extension {
                // Source code changes
                "rs" | "c" | "cpp" | "h" | "hpp" | "py" | "js" | "ts" => CacheBustSeverity::High,
                
                // Configuration changes
                "toml" | "json" | "yaml" | "yml" | "ini" | "conf" => CacheBustSeverity::Medium,
                
                // Documentation/assets
                "md" | "txt" | "png" | "jpg" | "svg" => CacheBustSeverity::Low,
                
                // Package/dependency files
                "lock" | "sum" => CacheBustSeverity::Critical,
                
                _ => CacheBustSeverity::Low,
            }
        } else {
            CacheBustSeverity::Medium  // Unknown file type
        }
    }
    
    /// Main supervision loop for daemon health monitoring
    fn supervision_loop(&self) -> Result<()> {
        let interval = Duration::from_secs(60); // Health check every minute
        
        loop {
            {
                let running = self.daemon_running.lock().unwrap();
                if !*running {
                    log::info!("🛑 Daemon shutdown requested");
                    break;
                }
            }
            
            // Health checks and self-healing
            if self.config.global.self_healing {
                self.perform_health_checks()?;
            }
            
            thread::sleep(interval);
        }
        
        Ok(())
    }
    
    /// Self-healing health checks
    fn perform_health_checks(&self) -> Result<()> {
        // Check if critical processes are still running
        for (target_name, watcher) in &self.watchers {
            if watcher.config.pid_watch {
                let current_pid = Self::get_runtime_pid(&watcher.config.runtime);
                if current_pid.is_none() && watcher.config.critical_path.unwrap_or(false) {
                    log::warn!("🚨 Critical process {} is down - triggering recovery", target_name);
                    self.trigger_process_recovery(target_name)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Trigger recovery for failed critical processes
    fn trigger_process_recovery(&self, target_name: &str) -> Result<()> {
        log::info!("🔧 Attempting recovery for target: {}", target_name);
        
        // Trigger critical cache bust to force rebuild/restart
        self.cache_manager.bust_cache(target_name, CacheBustSeverity::Critical)?;
        
        // Additional recovery logic would go here (restart scripts, notifications, etc.)
        
        Ok(())
    }
    
    /// Graceful shutdown
    pub fn shutdown(&self) -> Result<()> {
        {
            let mut running = self.daemon_running.lock().unwrap();
            *running = false;
        }
        
        log::info!("🛑 bustcall daemon shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_config_parsing() {
        let config_content = r#"
[global]
self_healing = true
supervisor_mode = true
default_max_retries = 3
daemon_interval_seconds = 5

[target.node]
path = "./node_modules"
runtime = "node"
pid_watch = true
enabled = true
language_priority = 0.8
critical_path = true

[target.python]
path = "./venv/lib"
runtime = "python3"
pid_watch = true
enabled = true
language_priority = 0.7
"#;
        
        let config: BustCallConfig = toml::from_str(config_content).unwrap();
        assert_eq!(config.global.self_healing, true);
        assert_eq!(config.target.len(), 2);
        assert_eq!(config.target["node"].runtime, "node");
    }
}
