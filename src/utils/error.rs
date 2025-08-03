#[derive(Debug, thiserror::Error)]
pub enum BustcallError {
    #[error("Daemon error: {0}")]
    DaemonError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Process error: {0}")]
    ProcessError(String),
    
    #[error("Notification error: {0}")]
    NotificationError(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, BustcallError>;
