use crate::utils::error::{BustcallError, Result};

#[derive(Debug, Clone, Copy)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Critical,
}

pub type NotifyResult = Result<()>;

#[derive(Debug)]
pub struct NotificationManager {
    // Implementation details
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn send(&self, level: NotificationLevel, message: &str) -> NotifyResult {
        println!("[{:?}] {}", level, message);
        Ok(())
    }
}
