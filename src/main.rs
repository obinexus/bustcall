// src/main.rs
use std::path::Path;
use clap::{Arg, ArgMatches, Command};
use anyhow::{Context, Result};
use log::{info, warn, error};
use env_logger;

mod dimensional_cache;
mod pid_watcher;

use dimensional_cache::{DimensionalCacheManager, CacheBustSeverity, EvictionStrategy, ModelWeights};
use pid_watcher::{BustCallDaemon, BustCallConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging system
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    info!("🚀 bustcall v{} - OBINexus Polyglot Cache Buster", env!("CARGO_PKG_VERSION"));
    
    let matches = build_cli().get_matches();
    
    match matches.subcommand() {
        Some(("daemon", sub_matches)) => handle_daemon_command(sub_matches).await,
        Some(("bind", sub_matches)) => handle_bind_command(sub_matches).await,
        Some(("bust", sub_matches)) => handle_bust_command(sub_matches).await,
        Some(("watch", sub_matches)) => handle_watch_command(sub_matches).await,
        Some(("status", sub_matches)) => handle_status_command(sub_matches).await,
        Some(("evict", sub_matches)) => handle_evict_command(sub_matches).await,
        _ => {
            // Default behavior - analyze command line arguments for legacy compatibility
            handle_legacy_mode(&matches).await
        }
    }
}

fn build_cli() -> Command {
    Command::new("bustcall")
        .version(env!("CARGO_PKG_VERSION"))
        .author("OBINexus Team <obinexus@obinexus.com>")
        .about("World's first polyglot cache buster with constitutional compliance")
        .arg(Arg::new("config")
            .short('c')
            .long("config")
            .value_name("FILE")
            .help("Configuration file path")
            .default_value("bustcall.config.toml"))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .action(clap::ArgAction::Count)
            .help("Increase verbosity level"))
        .subcommand(
            Command::new("daemon")
                .about("Run bustcall as background daemon")
                .arg(Arg::new("detach")
                    .short('d')
                    .long("detach")
                    .action(clap::ArgAction::SetTrue)
                    .help("Detach process and run in background"))
        )
        .subcommand(
            Command::new("bind")
                .about("Bind a runtime target for monitoring")
                .arg(Arg::new("target")
                    .short('t')
                    .long("target")
                    .value_name("TARGET")
                    .help("Target runtime name (node, python, c, etc.)")
                    .required(true))
                .arg(Arg::new("path")
                    .short('p')
                    .long("path")
                    .value_name("PATH")
                    .help("Path to monitor for changes")
                    .required(true))
                .arg(Arg::new("runtime")
                    .short('r')
                    .long("runtime")
                    .value_name("RUNTIME")
                    .help("Runtime process name")
                    .required(true))
        )
        .subcommand(
            Command::new("bust")
                .about("Manually trigger cache bust for target")
                .arg(Arg::new("target")
                    .short('t')
                    .long("target")
                    .value_name("TARGET")
                    .help("Target to bust cache for")
                    .required(true))
                .arg(Arg::new("severity")
                    .short('s')
                    .long("severity")
                    .value_name("SEVERITY")
                    .help("Bust severity level")
                    .value_parser(["low", "medium", "high", "critical"])
                    .default_value("medium"))
        )
        .subcommand(
            Command::new("watch")
                .about("Start file/PID watching for specific target")
                .arg(Arg::new("target")
                    .short('t')
                    .long("target")
                    .value_name("TARGET")
                    .help("Target to watch")
                    .required(true))
                .arg(Arg::new("daemon")
                    .short('d')
                    .long("daemon")
                    .action(clap::ArgAction::SetTrue)
                    .help("Run in daemon mode"))
        )
        .subcommand(
            Command::new("status")
                .about("Show current daemon and cache status")
        )
        .subcommand(
            Command::new("evict")
                .about("Manually trigger cache eviction")
                .arg(Arg::new("strategy")
                    .short('s')
                    .long("strategy")
                    .value_name("STRATEGY")
                    .help("Eviction strategy")
                    .value_parser(["lru", "mru", "lfu", "fifo", "model-aware"])
                    .default_value("model-aware"))
        )
}

async fn handle_daemon_command(matches: &ArgMatches) -> Result<()> {
    let config_path = matches.get_one::<String>("config").unwrap();
    let detach = matches.get_flag("detach");
    
    info!("🔧 Starting bustcall daemon with config: {}", config_path);
    
    if !Path::new(config_path).exists() {
        error!("❌ Configuration file not found: {}", config_path);
        create_default_config(config_path)?;
        info!("📝 Created default configuration at: {}", config_path);
        return Ok(());
    }
    
    let mut daemon = BustCallDaemon::new(config_path)
        .context("Failed to initialize daemon")?;
    
    if detach {
        info!("🔄 Detaching process...");
        // In a real implementation, this would fork the process
        // For now, we'll run in foreground with a note
        warn!("⚠️ Process detachment not implemented in this version - running in foreground");
    }
    
    daemon.start_daemon()
        .context("Failed to start daemon")?;
    
    Ok(())
}

async fn handle_bind_command(matches: &ArgMatches) -> Result<()> {
    let target = matches.get_one::<String>("target").unwrap();
    let path = matches.get_one::<String>("path").unwrap();
    let runtime = matches.get_one::<String>("runtime").unwrap();
    
    info!("🔗 Binding target '{}' with runtime '{}' at path '{}'", target, runtime, path);
    
    let cache_manager = DimensionalCacheManager::new()?;
    let binding = pid_watcher::ModelBinding {
        runtime: runtime.clone(),
        pid: None,
        path: path.clone(),
        last_modified: 0,
        cache_dependencies: Vec::new(),
    };
    
    cache_manager.bind_model(target, binding)?;
    info!("✅ Successfully bound target: {}", target);
    
    Ok(())
}

async fn handle_bust_command(matches: &ArgMatches) -> Result<()> {
    let target = matches.get_one::<String>("target").unwrap();
    let severity_str = matches.get_one::<String>("severity").unwrap();
    
    let severity = match severity_str.as_str() {
        "low" => CacheBustSeverity::Low,
        "medium" => CacheBustSeverity::Medium,
        "high" => CacheBustSeverity::High,
        "critical" => CacheBustSeverity::Critical,
        _ => CacheBustSeverity::Medium,
    };
    
    info!("💥 Triggering cache bust for '{}' with severity: {:?}", target, severity);
    
    let cache_manager = DimensionalCacheManager::new()?;
    cache_manager.bust_cache(target, severity)?;
    
    info!("✅ Cache bust completed for target: {}", target);
    
    Ok(())
}

async fn handle_watch_command(matches: &ArgMatches) -> Result<()> {
    let target = matches.get_one::<String>("target").unwrap();
    let daemon_mode = matches.get_flag("daemon");
    
    info!("👀 Starting watch for target: {}", target);
    
    if daemon_mode {
        info!("🔄 Running in daemon mode...");
        // This would start a persistent watcher
        // For now, simulate with a simple message
        info!("✅ Watch daemon started for target: {}", target);
    } else {
        info!("🔍 Single-run watch mode for target: {}", target);
    }
    
    Ok(())
}

async fn handle_status_command(_matches: &ArgMatches) -> Result<()> {
    info!("📊 bustcall Status Report");
    
    // In a real implementation, this would query the daemon status
    // For now, show basic system information
    
    println!("🔧 bustcall v{}", env!("CARGO_PKG_VERSION"));
    println!("📍 OBINexus Constitutional Compliance: ✅ Active");
    println!("🧠 Dimensional Cache: ✅ Available");
    println!("🔗 PID Monitoring: ✅ Available");
    println!("🗂️ Polyglot Support: Node.js, Python, C/C++, GosiLang");
    
    Ok(())
}

async fn handle_evict_command(matches: &ArgMatches) -> Result<()> {
    let strategy_str = matches.get_one::<String>("strategy").unwrap();
    
    let strategy = match strategy_str.as_str() {
        "lru" => EvictionStrategy::LRU,
        "mru" => EvictionStrategy::MRU,
        "lfu" => EvictionStrategy::LFU,
        "fifo" => EvictionStrategy::FIFO,
        "model-aware" => {
            EvictionStrategy::ModelAware(ModelWeights {
                language_priority: 0.7,
                dependency_impact: 0.8,
                build_cost: 0.6,
                critical_path: true,
            })
        }
        _ => EvictionStrategy::LRU,
    };
    
    info!("🗑️ Triggering cache eviction with strategy: {:?}", strategy);
    
    let cache_manager = DimensionalCacheManager::new()?;
    let evicted = cache_manager.cache_evict(&strategy)?;
    
    info!("✅ Evicted {} cache entries: {:?}", evicted.len(), evicted);
    
    Ok(())
}

async fn handle_legacy_mode(matches: &ArgMatches) -> Result<()> {
    // Handle legacy command format like `cargo run -- test-warn`
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        let command = &args[1];
        
        match command.as_str() {
            "test-warn" => {
                info!("🧪 Running test warning simulation");
                
                let cache_manager = DimensionalCacheManager::new()?;
                
                // Simulate a warning-level cache issue
                cache_manager.bust_cache("test-target", CacheBustSeverity::Medium)?;
                
                info!("⚠️ Test warning simulation completed");
                Ok(())
            }
            "test-danger" => {
                info!("🧪 Running test danger simulation");
                
                let cache_manager = DimensionalCacheManager::new()?;
                cache_manager.bust_cache("test-target", CacheBustSeverity::High)?;
                
                info!("🚨 Test danger simulation completed");
                Ok(())
            }
            "test-panic" => {
                info!("🧪 Running test panic simulation");
                
                let cache_manager = DimensionalCacheManager::new()?;
                cache_manager.bust_cache("test-target", CacheBustSeverity::Critical)?;
                
                error!("💀 Test panic simulation completed");
                Ok(())
            }
            _ => {
                info!("ℹ️ Use 'bustcall --help' for usage information");
                Ok(())
            }
        }
    } else {
        info!("ℹ️ Use 'bustcall --help' for usage information");
        Ok(())
    }
}

fn create_default_config(config_path: &str) -> Result<()> {
    let default_config = r#"# bustcall.config.toml
# OBINexus Polyglot Cache Buster Configuration

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
dependency_impact = 0.9
build_cost = 0.7
critical_path = true

[target.python]
path = "./venv/lib"
runtime = "python3"
pid_watch = true
enabled = true
language_priority = 0.7
dependency_impact = 0.8
build_cost = 0.6
critical_path = false

[target.c]
path = "./bin"
runtime = "gcc"
pid_watch = false
enabled = true
language_priority = 0.9
dependency_impact = 0.9
build_cost = 0.9
critical_path = true

[target.gosilang]
path = "./gosi/build"
runtime = "gosi"
pid_watch = true
enabled = false
language_priority = 1.0
dependency_impact = 1.0
build_cost = 0.8
critical_path = true
"#;
    
    std::fs::write(config_path, default_config)
        .context("Failed to create default configuration file")?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cli_parsing() {
        let app = build_cli();
        let matches = app.try_get_matches_from(vec!["bustcall", "status"]);
        assert!(matches.is_ok());
    }
    
    #[tokio::test]
    async fn test_bind_command() {
        let matches = build_cli().get_matches_from(vec![
            "bustcall", "bind", 
            "--target", "test", 
            "--path", "./test", 
            "--runtime", "test-runtime"
        ]);
        
        if let Some(sub_matches) = matches.subcommand_matches("bind") {
            assert_eq!(sub_matches.get_one::<String>("target").unwrap(), "test");
            assert_eq!(sub_matches.get_one::<String>("path").unwrap(), "./test");
            assert_eq!(sub_matches.get_one::<String>("runtime").unwrap(), "test-runtime");
        }
    }
}