use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BustcallConfig {
    pub daemon: crate::core::daemon::DaemonConfig,
    pub notifications: NotificationConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub interval_seconds: u64,
    pub processes: Vec<String>,
}

impl Default for BustcallConfig {
    fn default() -> Self {
        Self {
            daemon: crate::core::daemon::DaemonConfig::default(),
            notifications: NotificationConfig {
                enabled: true,
                channels: vec!["console".to_string()],
            },
            monitoring: MonitoringConfig {
                interval_seconds: 5,
                processes: vec![],
            },
        }
    }
}
