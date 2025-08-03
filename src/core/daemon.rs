use crate::utils::error::{BustcallError, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub bind_address: String,
    pub port: u16,
    pub log_level: String,
    pub pid_file: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8080,
            log_level: "info".to_string(),
            pid_file: "/tmp/bustcall.pid".to_string(),
        }
    }
}

impl DaemonConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| BustcallError::ConfigError(format!("Failed to read config: {}", e)))?;
        
        toml::from_str(&content)
            .map_err(|e| BustcallError::ConfigError(format!("Failed to parse config: {}", e)))
    }
    
    pub fn load_default() -> Result<Self> {
        Ok(Self::default())
    }
}

#[derive(Debug, Clone)]
pub enum DaemonStatus {
    Running { pid: u32, uptime: u64 },
    Stopped,
    Error(String),
}

#[derive(Debug)]
pub struct Daemon {
    config: DaemonConfig,
    status: Arc<Mutex<DaemonStatus>>,
}

impl Daemon {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: DaemonConfig::default(),
            status: Arc::new(Mutex::new(DaemonStatus::Stopped)),
        })
    }
    
    pub fn with_config(config: DaemonConfig) -> Result<Self> {
        Ok(Self {
            config,
            status: Arc::new(Mutex::new(DaemonStatus::Stopped)),
        })
    }
    
    pub fn connect() -> Result<Self> {
        // Implementation for connecting to existing daemon
        Self::new()
    }
    
    pub fn start(&mut self) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        *status = DaemonStatus::Running { 
            pid: std::process::id(), 
            uptime: 0 
        };
        Ok(())
    }
    
    pub fn start_detached(&mut self) -> Result<()> {
        self.start()
    }
    
    pub fn stop(&mut self) -> Result<()> {
        let mut status = self.status.lock().unwrap();
        *status = DaemonStatus::Stopped;
        Ok(())
    }
    
    pub fn status(&self) -> DaemonStatus {
        self.status.lock().unwrap().clone()
    }
    
    pub fn wait_for_shutdown(&self) -> Result<()> {
        // Implementation for graceful shutdown
        Ok(())
    }
}

impl Clone for Daemon {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            status: Arc::clone(&self.status),
        }
    }
}
