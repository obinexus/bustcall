// src/ffi/mod.rs - Recommended approach for OBINexus architecture
//! OBINexus FFI Module - Constitutional Multi-Language Bindings
//! Provides C and Python bindings for bustcall core functionality

pub mod c_bindings;
pub mod python_bindings;

// Re-export FFI functionality
pub use c_bindings::*;
pub use python_bindings::*;

// src/ffi/c_bindings.rs
//! C FFI bindings for OBINexus bustcall core

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::core::daemon::Daemon;
use crate::core::notify::{NotificationLevel, NotificationManager};

/// Opaque pointer type for C API
pub type BustcallDaemonHandle = *mut Daemon;

/// Create new daemon instance
#[no_mangle]
pub extern "C" fn bustcall_daemon_new() -> BustcallDaemonHandle {
    match Daemon::new() {
        Ok(daemon) => Box::into_raw(Box::new(daemon)),
        Err(_) => ptr::null_mut(),
    }
}

/// Start daemon
#[no_mangle]
pub extern "C" fn bustcall_daemon_start(handle: BustcallDaemonHandle) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let daemon = unsafe { &mut *handle };
    match daemon.start() {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Stop daemon
#[no_mangle]
pub extern "C" fn bustcall_daemon_stop(handle: BustcallDaemonHandle) -> c_int {
    if handle.is_null() {
        return -1;
    }
    
    let daemon = unsafe { &mut *handle };
    match daemon.stop() {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Free daemon resources
#[no_mangle]
pub extern "C" fn bustcall_daemon_free(handle: BustcallDaemonHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

/// Send notification (constitutional compliance)
#[no_mangle]
pub extern "C" fn bustcall_notify(level: c_int, message: *const c_char) -> c_int {
    if message.is_null() {
        return -1;
    }
    
    let c_str = unsafe { CStr::from_ptr(message) };
    let message_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    
    let notification_level = match level {
        0 => NotificationLevel::Info,
        1 => NotificationLevel::Warning,
        2 => NotificationLevel::Error,
        3 => NotificationLevel::Critical,
        _ => NotificationLevel::Info,
    };
    
    let notification_manager = NotificationManager::new();
    match notification_manager.send(notification_level, message_str) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Get version string
#[no_mangle]
pub extern "C" fn bustcall_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

// src/ffi/python_bindings.rs
//! Python FFI bindings for OBINexus bustcall core

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use crate::core::daemon::{Daemon, DaemonStatus};
use crate::core::notify::{NotificationLevel, NotificationManager};

#[pyclass]
pub struct PyDaemon {
    inner: Daemon,
}

#[pymethods]
impl PyDaemon {
    #[new]
    pub fn new() -> PyResult<Self> {
        match Daemon::new() {
            Ok(daemon) => Ok(PyDaemon { inner: daemon }),
            Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e))),
        }
    }
    
    pub fn start(&mut self) -> PyResult<()> {
        self.inner.start()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
    }
    
    pub fn stop(&mut self) -> PyResult<()> {
        self.inner.stop()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
    }
    
    pub fn status(&self) -> String {
        format!("{:?}", self.inner.status())
    }
    
    pub fn is_running(&self) -> bool {
        matches!(self.inner.status(), DaemonStatus::Running)
    }
}

#[pyclass]
pub struct PyNotificationManager {
    inner: NotificationManager,
}

#[pymethods]
impl PyNotificationManager {
    #[new]
    pub fn new() -> Self {
        Self {
            inner: NotificationManager::new(),
        }
    }
    
    pub fn send_info(&self, message: &str) -> PyResult<()> {
        self.inner.send(NotificationLevel::Info, message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        Ok(())
    }
    
    pub fn send_warning(&self, message: &str) -> PyResult<()> {
        self.inner.send(NotificationLevel::Warning, message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        Ok(())
    }
    
    pub fn send_error(&self, message: &str) -> PyResult<()> {
        self.inner.send(NotificationLevel::Error, message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        Ok(())
    }
    
    pub fn send_critical(&self, message: &str) -> PyResult<()> {
        self.inner.send(NotificationLevel::Critical, message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        Ok(())
    }
}

/// Test warning function (constitutional testing requirement)
#[pyfunction]
pub fn test_warn(message: String) -> PyResult<()> {
    let notification_manager = NotificationManager::new();
    notification_manager.send(NotificationLevel::Warning, &message)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
}

/// Test critical function (constitutional testing requirement)
#[pyfunction]
pub fn test_critical(message: String) -> PyResult<()> {
    let notification_manager = NotificationManager::new();
    notification_manager.send(NotificationLevel::Critical, &message)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
}

/// Python module definition
#[pymodule]
fn bustcall_core(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyDaemon>()?;
    m.add_class::<PyNotificationManager>()?;
    m.add_function(wrap_pyfunction!(test_warn, m)?)?;
    m.add_function(wrap_pyfunction!(test_critical, m)?)?;
    
    // Add version information
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__author__", "OBINexus Team")?;
    
    Ok(())
}