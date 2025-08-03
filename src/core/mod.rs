// src/core/mod.rs
//! OBINexus Core Module Implementation
//! Constitutional compliance framework for bustcall daemon architecture

pub mod daemon;
pub mod notify;
pub mod process;
pub mod config;

// Re-export core types for library interface
pub use daemon::{Daemon, DaemonConfig, DaemonStatus};
pub use notify::{NotificationLevel, NotificationManager, NotifyResult};
pub use process::{ProcessManager, ProcessInfo, ProcessFilter};
pub use config::{BustcallConfig, ConfigError};

// src/core/daemon.rs
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub interval_seconds: u64,
    pub max_retries: u32,
    pub supervisor_mode: bool,
    pub self_healing: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 5,
            max_retries: 3,
            supervisor_mode: true,
            self_healing: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DaemonStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

pub struct Daemon {
    config: DaemonConfig,
    status: Arc<Mutex<DaemonStatus>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Daemon {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: DaemonConfig::default(),
            status: Arc::new(Mutex::new(DaemonStatus::Stopped)),
            shutdown_tx: None,
        })
    }

    pub fn with_config(config: DaemonConfig) -> Result<Self> {
        Ok(Self {
            config,
            status: Arc::new(Mutex::new(DaemonStatus::Stopped)),
            shutdown_tx: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        if *status != DaemonStatus::Stopped {
            return Err(BustcallError::DaemonError("Daemon already running".to_string()));
        }
        *status = DaemonStatus::Starting;
        drop(status);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let status_clone = self.status.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            *status_clone.lock().unwrap() = DaemonStatus::Running;
            
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(config.interval_seconds)) => {
                        // Perform periodic cache bust operations
                        if let Err(e) = Self::perform_cache_operations(&config).await {
                            log::warn!("Cache operation failed: {}", e);
                        }
                    }
                }
            }
            
            *status_clone.lock().unwrap() = DaemonStatus::Stopped;
        });

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.try_send(());
        }
        
        *self.status.lock().unwrap() = DaemonStatus::Stopping;
        Ok(())
    }

    pub fn status(&self) -> DaemonStatus {
        self.status.lock().unwrap().clone()
    }

    async fn perform_cache_operations(_config: &DaemonConfig) -> Result<()> {
        // Implementation for cache operations
        // This will integrate with dimensional_cache module
        Ok(())
    }
}

// src/core/notify.rs
use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct NotifyResult {
    pub level: NotificationLevel,
    pub message: String,
    pub timestamp: std::time::SystemTime,
}

pub struct NotificationManager {
    // Internal notification state
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn send(&self, level: NotificationLevel, message: &str) -> Result<NotifyResult> {
        let result = NotifyResult {
            level: level.clone(),
            message: message.to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        // Log notification based on level
        match level {
            NotificationLevel::Info => log::info!("{}", message),
            NotificationLevel::Warning => log::warn!("{}", message),
            NotificationLevel::Error => log::error!("{}", message),
            NotificationLevel::Critical => {
                log::error!("CRITICAL: {}", message);
                // Could trigger additional alerting mechanisms
            }
        }

        Ok(result)
    }
}

// src/core/process.rs
use std::collections::HashMap;
use sysinfo::{ProcessExt, System, SystemExt};
use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ProcessFilter {
    pub name_pattern: Option<String>,
    pub min_cpu_usage: Option<f32>,
    pub min_memory_usage: Option<u64>,
}

pub struct ProcessManager {
    system: System,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
        }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }

    pub fn get_processes(&self, filter: Option<ProcessFilter>) -> Vec<ProcessInfo> {
        let mut processes = Vec::new();

        for (pid, process) in self.system.processes() {
            let process_info = ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string(),
                cpu_usage: process.cpu_usage(),
                memory_usage: process.memory(),
                status: format!("{:?}", process.status()),
            };

            if let Some(ref filter) = filter {
                if !self.matches_filter(&process_info, filter) {
                    continue;
                }
            }

            processes.push(process_info);
        }

        processes
    }

    fn matches_filter(&self, process: &ProcessInfo, filter: &ProcessFilter) -> bool {
        if let Some(ref pattern) = filter.name_pattern {
            if !process.name.contains(pattern) {
                return false;
            }
        }

        if let Some(min_cpu) = filter.min_cpu_usage {
            if process.cpu_usage < min_cpu {
                return false;
            }
        }

        if let Some(min_memory) = filter.min_memory_usage {
            if process.memory_usage < min_memory {
                return false;
            }
        }

        true
    }
}

// src/core/config.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BustcallConfig {
    pub global: GlobalConfig,
    pub target: HashMap<String, TargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub self_healing: bool,
    pub supervisor_mode: bool,
    pub default_max_retries: u32,
    pub daemon_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub path: String,
    pub runtime: String,
    pub pid_watch: bool,
    pub enabled: bool,
    pub language_priority: f64,
    pub dependency_impact: f64,
    pub build_cost: f64,
    pub critical_path: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(String),
    #[error("Configuration parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    TomlError(#[from] toml::de::Error),
}

impl BustcallConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(BustcallError::ConfigError(ConfigError::NotFound(
                path.display().to_string(),
            )));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| BustcallError::ConfigError(ConfigError::IoError(e)))?;

        let config: BustcallConfig = toml::from_str(&content)
            .map_err(|e| BustcallError::ConfigError(ConfigError::TomlError(e)))?;

        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| BustcallError::ConfigError(ConfigError::ParseError(e.to_string())))?;

        fs::write(path, content)
            .map_err(|e| BustcallError::ConfigError(ConfigError::IoError(e)))?;

        Ok(())
    }

    pub fn default() -> Self {
        let mut targets = HashMap::new();
        
        targets.insert("node".to_string(), TargetConfig {
            path: "./node_modules".to_string(),
            runtime: "node".to_string(),
            pid_watch: true,
            enabled: true,
            language_priority: 0.8,
            dependency_impact: 0.9,
            build_cost: 0.7,
            critical_path: true,
        });

        targets.insert("python".to_string(), TargetConfig {
            path: "./venv/lib".to_string(),
            runtime: "python3".to_string(),
            pid_watch: true,
            enabled: true,
            language_priority: 0.7,
            dependency_impact: 0.8,
            build_cost: 0.6,
            critical_path: false,
        });

        Self {
            global: GlobalConfig {
                self_healing: true,
                supervisor_mode: true,
                default_max_retries: 3,
                daemon_interval_seconds: 5,
            },
            target: targets,
        }
    }
}