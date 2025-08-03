//! OBINexus bustcall - Core cache management and system orchestration
//! 
//! Provides polyglot-ready API for cache invalidation, process monitoring,
//! and self-healing capabilities within CI/CD pipeline orchestration.

pub mod core;
pub mod ffi;

// Re-export core functionality
pub use core::{
    dimensional_cache::{CacheManager, InvalidationStrategy},
    self_healing::{HealthMonitor, RecoveryProtocol},
    pid_watcher::{ProcessWatcher, SystemState},
};

// C-compatible API for polyglot bindings
#[no_mangle]
pub extern "C" fn bustcall_init() -> *mut core::BustcallContext {
    // Implementation for C FFI
}

#[no_mangle]
pub extern "C" fn bustcall_cache_invalidate(
    ctx: *mut core::BustcallContext,
    target: *const std::os::raw::c_char,
    severity: std::os::raw::c_int,
) -> std::os::raw::c_int {
    // Implementation for cache invalidation via C ABI
}
