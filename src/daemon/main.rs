//! OBINexus Bustcall Daemon Binary
//! 
//! Standalone daemon process for system monitoring

use bustcall_core::{Daemon, DaemonConfig, init_logger, LogLevel};
use anyhow::Result;

fn main() -> Result<()> {
    init_logger(LogLevel::Info)?;
    
    let config = DaemonConfig::default();
    let mut daemon = Daemon::with_config(config)?;
    
    println!("Starting OBINexus bustcall daemon...");
    daemon.start()?;
    
    // Handle shutdown signals
    daemon.wait_for_shutdown()?;
    
    Ok(())
}
