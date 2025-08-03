use crate::utils::error::Result;

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub fn init_logger(level: LogLevel) -> Result<()> {
    let env_level = match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug", 
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    };
    
    std::env::set_var("RUST_LOG", env_level);
    env_logger::init();
    Ok(())
}
