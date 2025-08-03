//! OBINexus Bustcall Core Library
//! 
//! This crate provides process monitoring, notification, and daemon management
//! capabilities for the OBINexus CI/CD pipeline.

pub mod core;
pub mod utils;

#[cfg(feature = "ffi")]
pub mod ffi;

// Re-export core functionality
pub use core::{
    daemon::{Daemon, DaemonConfig, DaemonStatus},
    notify::{NotificationLevel, NotificationManager, NotifyResult},
    process::{ProcessManager, ProcessInfo, ProcessFilter},
    config::{BustcallConfig, ConfigError},
};

pub use utils::{
    logger::{init_logger, LogLevel},
    error::{BustcallError, Result},
};

// FFI exports for Python bindings
#[cfg(feature = "python-bindings")]
pub mod python_api {
    use pyo3::prelude::*;
    use pyo3::wrap_pyfunction;
    
    use crate::core::daemon::{Daemon, DaemonStatus};
    use crate::core::notify::NotificationLevel;
    
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
    }
    
    #[pyfunction]
    pub fn test_warn(message: String) -> PyResult<()> {
        let notification_manager = crate::core::notify::NotificationManager::new();
        notification_manager.send(NotificationLevel::Warning, &message)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
    }
    
    #[pymodule]
    fn bustcall_core(_py: Python, m: &PyModule) -> PyResult<()> {
        m.add_class::<PyDaemon>()?;
        m.add_function(wrap_pyfunction!(test_warn, m)?)?;
        Ok(())
    }
}

// C FFI exports
#[cfg(feature = "ffi")]
pub mod c_api {
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    use crate::core::daemon::Daemon;
    
    #[no_mangle]
    pub extern "C" fn bustcall_daemon_new() -> *mut Daemon {
        match Daemon::new() {
            Ok(daemon) => Box::into_raw(Box::new(daemon)),
            Err(_) => std::ptr::null_mut(),
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bustcall_daemon_start(daemon: *mut Daemon) -> c_int {
        if daemon.is_null() {
            return -1;
        }
        
        let daemon = unsafe { &mut *daemon };
        match daemon.start() {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bustcall_daemon_stop(daemon: *mut Daemon) -> c_int {
        if daemon.is_null() {
            return -1;
        }
        
        let daemon = unsafe { &mut *daemon };
        match daemon.stop() {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bustcall_daemon_free(daemon: *mut Daemon) {
        if !daemon.is_null() {
            unsafe {
                let _ = Box::from_raw(daemon);
            }
        }
    }
    
    #[no_mangle]
    pub extern "C" fn bustcall_test_warn(message: *const c_char) -> c_int {
        if message.is_null() {
            return -1;
        }
        
        let c_str = unsafe { CStr::from_ptr(message) };
        let message_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };
        
        let notification_manager = crate::core::notify::NotificationManager::new();
        match notification_manager.send(crate::core::notify::NotificationLevel::Warning, message_str) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_daemon_lifecycle() {
        let mut daemon = Daemon::new().expect("Failed to create daemon");
        assert!(daemon.start().is_ok());
        assert!(daemon.stop().is_ok());
    }
    
    #[test]
    fn test_notification_system() {
        let notification_manager = core::notify::NotificationManager::new();
        assert!(notification_manager.send(
            core::notify::NotificationLevel::Info,
            "Test message"
        ).is_ok());
    }
}