//! OBINexus bustcall CLI - Command interface for cache management
//! 
//! Provides terminal-based access to core functionality with daemon mode,
//! binding management, and system status monitoring.

use clap::{Parser, Subcommand};
use bustcall_core::{CacheManager, HealthMonitor, ProcessWatcher};

#[derive(Parser)]
#[command(name = "bustcall")]
#[command(about = "OBINexus cache invalidation and system orchestration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start daemon mode for continuous monitoring
    Daemon,
    /// Bind runtime targets for cache management
    Bind {
        #[arg(long)]
        target: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        runtime: String,
    },
    /// Execute cache invalidation with specified severity
    Bust {
        #[arg(long)]
        target: String,
        #[arg(long)]
        severity: String,
    },
    /// Display system status and health metrics
    Status,
    /// Test warning protocols
    TestWarn,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Daemon => daemon_mode(),
        Commands::Bind { target, path, runtime } => bind_target(target, path, runtime),
        Commands::Bust { target, severity } => execute_bust(target, severity),
        Commands::Status => display_status(),
        Commands::TestWarn => test_warning_protocols(),
    }
}
