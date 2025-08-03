// src/servers/server.rs - Unified API Server for OBINexus Bustcall
//! Constitutional REST API server implementing FaultTorrent execution model

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use warp::{Filter, Reply};

use crate::core::daemon::Daemon;
use crate::ffi::{BustcallDaemonHandle, bustcall_daemon_new, bustcall_daemon_start};

/// FaultTorrent execution stages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaultStage {
    Panic = 0,    // 0-3: HALT, rollback
    Exception = 3, // 3-6: TDD coverage required
    Warning = 6,   // 6-9: QA override or fix
    Silent = 9,    // 9-12: Log + scheduled fix
}

/// Binding metadata for capability advertisement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingMetadata {
    pub binding: String,
    pub capabilities: Vec<String>,
    pub semverx: String,
    pub stage: u8,
    pub p2p_enabled: bool,
}

/// Cache bust request structure
#[derive(Debug, Deserialize)]
pub struct BustRequest {
    pub target: String,
    pub strategy: Option<String>,
    pub binding: Option<String>,
    pub fault_tolerance: Option<u8>,
}

/// Cache bust response structure
#[derive(Debug, Serialize)]
pub struct BustResponse {
    pub status: String,
    pub cache_key: String,
    pub delegate: String,
    pub fault_stage: u8,
    pub execution_time_ms: u64,
}

/// Daemon status response
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub daemon_pid: u32,
    pub bindings: HashMap<String, BindingStatus>,
    pub cache_size: String,
    pub fault_history: Vec<FaultEvent>,
}

#[derive(Debug, Serialize)]
pub struct BindingStatus {
    pub status: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FaultEvent {
    pub timestamp: String,
    pub binding: String,
    pub fault_stage: u8,
    pub message: String,
}

/// OBINexus Bustcall API Server
pub struct BustcallServer {
    daemon_handle: Option<BustcallDaemonHandle>,
    bindings: Arc<RwLock<HashMap<String, BindingMetadata>>>,
    fault_history: Arc<RwLock<Vec<FaultEvent>>>,
}

impl BustcallServer {
    pub fn new() -> Self {
        let mut bindings = HashMap::new();
        
        // Register available bindings with capabilities
        bindings.insert("pybustcall".to_string(), BindingMetadata {
            binding: "pybustcall".to_string(),
            capabilities: vec!["daemon".to_string(), "cache.bust".to_string(), "watch.fs".to_string()],
            semverx: "v0.1.3".to_string(),
            stage: 3,
            p2p_enabled: true,
        });
        
        bindings.insert("napi-bustcall".to_string(), BindingMetadata {
            binding: "napi-bustcall".to_string(),
            capabilities: vec!["daemon".to_string(), "cache.bust".to_string()],
            semverx: "v0.1.3".to_string(),
            stage: 1,
            p2p_enabled: true,
        });

        Self {
            daemon_handle: None,
            bindings: Arc::new(RwLock::new(bindings)),
            fault_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize daemon
        self.daemon_handle = Some(unsafe { bustcall_daemon_new() });
        
        if let Some(handle) = self.daemon_handle {
            unsafe {
                bustcall_daemon_start(handle);
            }
        }

        // Start web server
        let bindings = self.bindings.clone();
        let fault_history = self.fault_history.clone();

        // API Routes
        let bust_route = warp::path!("api" / "v1" / "bust")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_state(bindings.clone()))
            .and(with_state(fault_history.clone()))
            .and_then(handle_bust);

        let status_route = warp::path!("api" / "v1" / "status")
            .and(warp::get())
            .and(with_state(bindings.clone()))
            .and(with_state(fault_history.clone()))
            .and_then(handle_status);

        let capabilities_route = warp::path!("api" / "v1" / "bindings" / "capabilities")
            .and(warp::get())
            .and(with_state(bindings.clone()))
            .and_then(handle_capabilities);

        let routes = bust_route
            .or(status_route)
            .or(capabilities_route)
            .with(warp::cors().allow_any_origin());

        println!("🌀 OBINexus Bustcall API Server starting on port 8989");
        println!("Constitutional compliance: FaultTorrent enabled");
        
        warp::serve(routes)
            .run(([127, 0, 0, 1], 8989))
            .await;

        Ok(())
    }
}

// Helper function to pass state to handlers
fn with_state<T: Clone + Send>(
    state: T,
) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Handle cache bust requests
async fn handle_bust(
    request: BustRequest,
    bindings: Arc<RwLock<HashMap<String, BindingMetadata>>>,
    fault_history: Arc<RwLock<Vec<FaultEvent>>>,
) -> Result<impl Reply, warp::Rejection> {
    let start_time = std::time::Instant::now();
    
    // Select binding (auto or specified)
    let selected_binding = match request.binding {
        Some(binding) => binding,
        None => "pybustcall".to_string(), // Default to Python binding
    };

    // Simulate cache bust operation
    let cache_key = format!("sha256:{}", hex::encode(sha2::Sha256::digest(request.target.as_bytes())));
    
    // Check fault tolerance threshold
    let fault_stage = request.fault_tolerance.unwrap_or(6);
    
    // Log fault event if necessary
    if fault_stage <= 6 {
        let mut history = fault_history.write().await;
        history.push(FaultEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            binding: selected_binding.clone(),
            fault_stage,
            message: format!("Cache bust executed for target: {}", request.target),
        });
    }

    let execution_time = start_time.elapsed().as_millis() as u64;

    let response = BustResponse {
        status: "success".to_string(),
        cache_key,
        delegate: selected_binding,
        fault_stage,
        execution_time_ms: execution_time,
    };

    Ok(warp::reply::json(&response))
}

/// Handle status requests
async fn handle_status(
    bindings: Arc<RwLock<HashMap<String, BindingMetadata>>>,
    fault_history: Arc<RwLock<Vec<FaultEvent>>>,
) -> Result<impl Reply, warp::Rejection> {
    let bindings_map = bindings.read().await;
    let history = fault_history.read().await;
    
    let mut binding_statuses = HashMap::new();
    for (name, metadata) in bindings_map.iter() {
        binding_statuses.insert(name.clone(), BindingStatus {
            status: "active".to_string(),
            version: Some(metadata.semverx.clone()),
        });
    }

    let response = StatusResponse {
        daemon_pid: std::process::id(),
        bindings: binding_statuses,
        cache_size: "1.2MB".to_string(),
        fault_history: history.clone(),
    };

    Ok(warp::reply::json(&response))
}

/// Handle capabilities requests
async fn handle_capabilities(
    bindings: Arc<RwLock<HashMap<String, BindingMetadata>>>,
) -> Result<impl Reply, warp::Rejection> {
    let bindings_map = bindings.read().await;
    Ok(warp::reply::json(&*bindings_map))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let mut server = BustcallServer::new();
    server.start().await?;
    
    Ok(())
}
