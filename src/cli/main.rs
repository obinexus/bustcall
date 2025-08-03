//! OBINexus Bustcall CLI Interface
//! 
//! Command-line interface for daemon management and process monitoring

use clap::{Parser, Subcommand};
use anyhow::Result;
use bustcall_core::{
    Daemon, DaemonConfig, DaemonStatus,
    NotificationLevel, NotificationManager,
    ProcessManager, ProcessFilter,
    init_logger, LogLevel, BustcallError
};

#[derive(Parser)]
#[command(name = "bustcall")]
#[command(about = "OBINexus Process Monitor and Notification Daemon")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, global = true)]
    verbose: bool,
    
    #[arg(short, long, global = true, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the bustcall daemon
    Daemon {
        #[arg(short, long)]
        config: Option<String>,
        
        #[arg(short, long)]
        detach: bool,
    },
    
    /// Check daemon status
    Status,
    
    /// Stop the daemon
    Stop,
    
    /// Send a test warning notification
    TestWarn {
        #[arg(short, long, default_value = "Test warning from bustcall")]
        message: String,
    },
    
    /// List monitored processes
    List {
        #[arg(short, long)]
        filter: Option<String>,
    },
    
    /// Monitor a specific process
    Monitor {
        /// Process ID or name pattern
        target: String,
        
        #[arg(short, long)]
        continuous: bool,
    },
    
    /// Show configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigActions>,
    },
}

#[derive(Subcommand)]
enum ConfigActions {
    /// Show current configuration
    Show,
    
    /// Validate configuration file
    Validate {
        path: String,
    },
    
    /// Generate default configuration
    Init {
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = match cli.log_level.to_lowercase().as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        _ => LogLevel::Info,
    };
    
    init_logger(log_level)?;
    
    match cli.command {
        Commands::Daemon { config, detach } => {
            handle_daemon_command(config, detach)
        },
        
        Commands::Status => {
            handle_status_command()
        },
        
        Commands::Stop => {
            handle_stop_command()
        },
        
        Commands::TestWarn { message } => {
            handle_test_warn_command(&message)
        },
        
        Commands::List { filter } => {
            handle_list_command(filter.as_deref())
        },
        
        Commands::Monitor { target, continuous } => {
            handle_monitor_command(&target, continuous)
        },
        
        Commands::Config { action } => {
            handle_config_command(action)
        },
    }
}

fn handle_daemon_command(config_path: Option<String>, detach: bool) -> Result<()> {
    println!("Starting OBINexus bustcall daemon...");
    
    let config = match config_path {
        Some(path) => DaemonConfig::from_file(&path)?,
        None => DaemonConfig::default(),
    };
    
    let mut daemon = Daemon::with_config(config)?;
    
    if detach {
        daemon.start_detached()?;
        println!("Daemon started in background");
    } else {
        daemon.start()?;
        println!("Daemon started in foreground");
        
        // Handle Ctrl+C gracefully
        let daemon_handle = daemon.clone();
        ctrlc::set_handler(move || {
            println!("\nReceived interrupt signal, stopping daemon...");
            if let Err(e) = daemon_handle.stop() {
                eprintln!("Error stopping daemon: {}", e);
            }
            std::process::exit(0);
        })?;
        
        // Keep the main thread alive
        daemon.wait_for_shutdown()?;
    }
    
    Ok(())
}

fn handle_status_command() -> Result<()> {
    let daemon = Daemon::connect()?;
    let status = daemon.status();
    
    match status {
        DaemonStatus::Running { pid, uptime } => {
            println!("Daemon Status: Running");
            println!("Process ID: {}", pid);
            println!("Uptime: {} seconds", uptime);
        },
        DaemonStatus::Stopped => {
            println!("Daemon Status: Stopped");
        },
        DaemonStatus::Error(msg) => {
            println!("Daemon Status: Error - {}", msg);
        },
    }
    
    Ok(())
}

fn handle_stop_command() -> Result<()> {
    println!("Stopping bustcall daemon...");
    
    let mut daemon = Daemon::connect()?;
    daemon.stop()?;
    
    println!("Daemon stopped successfully");
    Ok(())
}

fn handle_test_warn_command(message: &str) -> Result<()> {
    println!("Sending test warning: {}", message);
    
    let notification_manager = NotificationManager::new();
    notification_manager.send(NotificationLevel::Warning, message)?;
    
    println!("Warning notification sent successfully");
    Ok(())
}

fn handle_list_command(filter: Option<&str>) -> Result<()> {
    let process_manager = ProcessManager::new();
    
    let process_filter = match filter {
        Some(pattern) => ProcessFilter::NamePattern(pattern.to_string()),
        None => ProcessFilter::All,
    };
    
    let processes = process_manager.list_processes(process_filter)?;
    
    if processes.is_empty() {
        println!("No processes found matching criteria");
        return Ok(());
    }
    
    println!("{:<8} {:<20} {:<10} {:<15}", "PID", "NAME", "STATUS", "CPU%");
    println!("{}", "-".repeat(60));
    
    for process in processes {
        println!("{:<8} {:<20} {:<10} {:<15}",
            process.pid,
            process.name,
            process.status,
            format!("{:.1}%", process.cpu_usage)
        );
    }
    
    Ok(())
}

fn handle_monitor_command(target: &str, continuous: bool) -> Result<()> {
    let process_manager = ProcessManager::new();
    
    // Try to parse as PID first, then as name pattern
    let filter = if let Ok(pid) = target.parse::<u32>() {
        ProcessFilter::Pid(pid)
    } else {
        ProcessFilter::NamePattern(target.to_string())
    };
    
    if continuous {
        println!("Monitoring {} continuously (Ctrl+C to stop)...", target);
        
        loop {
            let processes = process_manager.list_processes(filter.clone())?;
            
            if processes.is_empty() {
                println!("Process {} not found", target);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
            
            for process in processes {
                println!("[{}] PID: {}, CPU: {:.1}%, Memory: {:.1}MB",
                    chrono::Utc::now().format("%H:%M:%S"),
                    process.pid,
                    process.cpu_usage,
                    process.memory_usage as f64 / 1024.0 / 1024.0
                );
            }
            
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    } else {
        let processes = process_manager.list_processes(filter)?;
        
        if processes.is_empty() {
            println!("Process {} not found", target);
            return Ok(());
        }
        
        for process in processes {
            println!("Process Information:");
            println!("  PID: {}", process.pid);
            println!("  Name: {}", process.name);
            println!("  Status: {}", process.status);
            println!("  CPU Usage: {:.1}%", process.cpu_usage);
            println!("  Memory Usage: {:.1}MB", process.memory_usage as f64 / 1024.0 / 1024.0);
        }
    }
    
    Ok(())
}

fn handle_config_command(action: Option<ConfigActions>) -> Result<()> {
    match action {
        Some(ConfigActions::Show) => {
            let config = DaemonConfig::load_default()?;
            println!("{}", toml::to_string_pretty(&config)?);
        },
        
        Some(ConfigActions::Validate { path }) => {
            match DaemonConfig::from_file(&path) {
                Ok(_) => println!("Configuration file '{}' is valid", path),
                Err(e) => {
                    eprintln!("Configuration file '{}' is invalid: {}", path, e);
                    std::process::exit(1);
                }
            }
        },
        
        Some(ConfigActions::Init { output }) => {
            let config = DaemonConfig::default();
            let config_str = toml::to_string_pretty(&config)?;
            
            match output {
                Some(path) => {
                    std::fs::write(&path, config_str)?;
                    println!("Default configuration written to '{}'", path);
                },
                None => {
                    println!("{}", config_str);
                }
            }
        },
        
        None => {
            println!("Available config actions: show, validate, init");
            println!("Use 'bustcall config --help' for more information");
        }
    }
    
    Ok(())
}

// Additional utility functions for CLI operations
fn check_daemon_running() -> bool {
    match Daemon::connect() {
        Ok(daemon) => matches!(daemon.status(), DaemonStatus::Running { .. }),
        Err(_) => false,
    }
}

fn format_uptime(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}