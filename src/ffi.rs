// src/ffi.rs
// OBINexus FFI Bindings - Polyglot Cache Buster Integration
// Architecture: Rust Core → FFI → Node/Python/C/GosiLang Ecosystems

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};
use serde_json;
use crate::{BustCall, BustCallConfig, BustCallError, SeverityLevel};

// =============================================================================
// C FFI Interface for Native Language Integration
// =============================================================================

#[repr(C)]
pub struct CBustResult {
    pub success: c_int,
    pub severity: c_uint,
    pub message: *mut c_char,
    pub component: *mut c_char,
    pub recovery_action: *mut c_char,
}

#[repr(C)]
pub struct CBustConfig {
    pub enable_self_healing: c_int,
    pub enable_panic_restart: c_int,
    pub max_retries: c_uint,
    pub constitutional_compliance: c_int,
}

/// Initialize bustcall instance for C/C++ integration
#[no_mangle]
pub extern "C" fn bustcall_init(config: *const CBustConfig) -> *mut BustCall {
    let config = if config.is_null() {
        BustCallConfig::default()
    } else {
        unsafe {
            BustCallConfig {
                enable_self_healing: (*config).enable_self_healing != 0,
                enable_panic_restart: (*config).enable_panic_restart != 0,
                max_retries: (*config).max_retries as u8,
                constitutional_compliance: (*config).constitutional_compliance != 0,
            }
        }
    };

    Box::into_raw(Box::new(BustCall::new(config)))
}

/// Execute cache bust operation via C FFI
#[no_mangle]
pub extern "C" fn bustcall_execute(
    instance: *mut BustCall,
    package: *const c_char,
    language: *const c_char,
) -> CBustResult {
    if instance.is_null() || package.is_null() || language.is_null() {
        return CBustResult {
            success: 0,
            severity: 12, // Panic level for invalid input
            message: CString::new("Invalid input parameters").unwrap().into_raw(),
            component: CString::new("ffi_interface").unwrap().into_raw(),
            recovery_action: CString::new("Check input parameters").unwrap().into_raw(),
        };
    }

    let instance = unsafe { &mut *instance };
    let package_str = unsafe { CStr::from_ptr(package).to_string_lossy() };
    let language_str = unsafe { CStr::from_ptr(language).to_string_lossy() };

    match instance.execute_bust(&package_str, &language_str) {
        Ok(_) => CBustResult {
            success: 1,
            severity: 0,
            message: CString::new("Cache bust completed successfully").unwrap().into_raw(),
            component: CString::new("cache_buster").unwrap().into_raw(),
            recovery_action: std::ptr::null_mut(),
        },
        Err(error) => CBustResult {
            success: 0,
            severity: error.severity as c_uint,
            message: CString::new(error.message).unwrap().into_raw(),
            component: CString::new(error.component).unwrap().into_raw(),
            recovery_action: error.recovery_action
                .map(|action| CString::new(action).unwrap().into_raw())
                .unwrap_or(std::ptr::null_mut()),
        },
    }
}

/// Free bustcall instance
#[no_mangle]
pub extern "C" fn bustcall_free(instance: *mut BustCall) {
    if !instance.is_null() {
        unsafe {
            Box::from_raw(instance);
        }
    }
}

/// Free CBustResult memory
#[no_mangle]
pub extern "C" fn bustcall_free_result(result: *mut CBustResult) {
    if !result.is_null() {
        unsafe {
            let result = &*result;
            if !result.message.is_null() {
                CString::from_raw(result.message);
            }
            if !result.component.is_null() {
                CString::from_raw(result.component);
            }
            if !result.recovery_action.is_null() {
                CString::from_raw(result.recovery_action);
            }
        }
    }
}

// =============================================================================
// Node.js FFI Integration via napi
// =============================================================================

#[cfg(feature = "node-bindings")]
mod node_bindings {
    use napi::bindgen_prelude::*;
    use napi_derive::napi;
    use serde::{Deserialize, Serialize};
    use crate::{BustCall, BustCallConfig};

    #[derive(Serialize, Deserialize)]
    #[napi(object)]
    pub struct NodeBustConfig {
        pub enable_self_healing: Option<bool>,
        pub enable_panic_restart: Option<bool>,
        pub max_retries: Option<u32>,
        pub constitutional_compliance: Option<bool>,
    }

    #[derive(Serialize, Deserialize)]
    #[napi(object)]
    pub struct NodeBustResult {
        pub success: bool,
        pub severity: u32,
        pub message: String,
        pub component: String,
        pub recovery_action: Option<String>,
    }

    #[napi]
    pub struct NodeBustCall {
        inner: BustCall,
    }

    #[napi]
    impl NodeBustCall {
        #[napi(constructor)]
        pub fn new(config: Option<NodeBustConfig>) -> Self {
            let config = config.map(|c| BustCallConfig {
                enable_self_healing: c.enable_self_healing.unwrap_or(true),
                enable_panic_restart: c.enable_panic_restart.unwrap_or(true),
                max_retries: c.max_retries.unwrap_or(3) as u8,
                constitutional_compliance: c.constitutional_compliance.unwrap_or(true),
            }).unwrap_or_default();

            Self {
                inner: BustCall::new(config),
            }
        }

        #[napi]
        pub async fn bust_cache(&mut self, package: String, language: String) -> Result<NodeBustResult> {
            match self.inner.execute_bust(&package, &language) {
                Ok(_) => Ok(NodeBustResult {
                    success: true,
                    severity: 0,
                    message: "Cache bust completed successfully".to_string(),
                    component: "cache_buster".to_string(),
                    recovery_action: None,
                }),
                Err(error) => Ok(NodeBustResult {
                    success: false,
                    severity: error.severity as u32,
                    message: error.message,
                    component: error.component,
                    recovery_action: error.recovery_action,
                }),
            }
        }

        /// Batch cache busting for multiple packages
        #[napi]
        pub async fn bust_multiple(&mut self, packages: Vec<String>, language: String) -> Result<Vec<NodeBustResult>> {
            let mut results = Vec::new();
            
            for package in packages {
                let result = match self.inner.execute_bust(&package, &language) {
                    Ok(_) => NodeBustResult {
                        success: true,
                        severity: 0,
                        message: format!("Cache bust completed for {}", package),
                        component: "cache_buster".to_string(),
                        recovery_action: None,
                    },
                    Err(error) => NodeBustResult {
                        success: false,
                        severity: error.severity as u32,
                        message: error.message,
                        component: error.component,
                        recovery_action: error.recovery_action,
                    },
                };
                results.push(result);
            }
            
            Ok(results)
        }

        /// Get system health metrics
        #[napi]
        pub fn get_health_metrics(&self) -> Result<String> {
            let metrics = serde_json::json!({
                "system_status": "operational",
                "supported_languages": ["node", "python", "c", "cpp", "gosilang"],
                "constitutional_compliance": true,
                "polycore_version": "v2"
            });
            
            Ok(metrics.to_string())
        }
    }

    /// Static utility functions for Node.js
    #[napi]
    pub fn bustcall_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[napi]
    pub fn bustcall_support_languages() -> Vec<String> {
        vec![
            "node".to_string(),
            "python".to_string(),
            "c".to_string(),
            "cpp".to_string(),
            "gosilang".to_string(),
        ]
    }

    #[napi]
    pub fn bustcall_severity_levels() -> String {
        serde_json::json!({
            "ok": { "range": "0-3", "action": "monitor" },
            "warning": { "range": "3-6", "action": "cache_bust" },
            "danger": { "range": "6-9", "action": "force_rebuild" },
            "critical": { "range": "9-12", "action": "restart_process" },
            "panic": { "range": "12+", "action": "emergency_protocols" }
        }).to_string()
    }
}

// =============================================================================
// Python FFI Integration via PyO3
// =============================================================================

#[cfg(feature = "python-bindings")]
mod python_bindings {
    use pyo3::prelude::*;
    use pyo3::types::PyDict;
    use crate::{BustCall, BustCallConfig, SeverityLevel};

    #[pyclass]
    pub struct PyBustCall {
        inner: BustCall,
    }

    #[pymethods]
    impl PyBustCall {
        #[new]
        #[pyo3(signature = (config=None))]
        pub fn new(config: Option<&PyDict>) -> PyResult<Self> {
            let config = if let Some(config_dict) = config {
                BustCallConfig {
                    enable_self_healing: config_dict
                        .get_item("enable_self_healing")?
                        .and_then(|v| v.extract().ok())
                        .unwrap_or(true),
                    enable_panic_restart: config_dict
                        .get_item("enable_panic_restart")?
                        .and_then(|v| v.extract().ok())
                        .unwrap_or(true),
                    max_retries: config_dict
                        .get_item("max_retries")?
                        .and_then(|v| v.extract().ok())
                        .unwrap_or(3),
                    constitutional_compliance: config_dict
                        .get_item("constitutional_compliance")?
                        .and_then(|v| v.extract().ok())
                        .unwrap_or(true),
                }
            } else {
                BustCallConfig::default()
            };

            Ok(Self {
                inner: BustCall::new(config),
            })
        }

        #[pyo3(signature = (package, language))]
        pub fn bust_cache(&mut self, package: &str, language: &str) -> PyResult<PyObject> {
            Python::with_gil(|py| {
                match self.inner.execute_bust(package, language) {
                    Ok(_) => {
                        let result = PyDict::new(py);
                        result.set_item("success", true)?;
                        result.set_item("severity", 0)?;
                        result.set_item("message", "Cache bust completed successfully")?;
                        result.set_item("component", "cache_buster")?;
                        result.set_item("recovery_action", py.None())?;
                        Ok(result.into())
                    }
                    Err(error) => {
                        let result = PyDict::new(py);
                        result.set_item("success", false)?;
                        result.set_item("severity", error.severity as u8)?;
                        result.set_item("message", error.message)?;
                        result.set_item("component", error.component)?;
                        result.set_item("recovery_action", error.recovery_action)?;
                        Ok(result.into())
                    }
                }
            })
        }

        #[pyo3(signature = (packages, language))]
        pub fn bust_multiple(&mut self, packages: Vec<&str>, language: &str) -> PyResult<Vec<PyObject>> {
            let mut results = Vec::new();
            
            for package in packages {
                Python::with_gil(|py| {
                    let result_dict = match self.inner.execute_bust(package, language) {
                        Ok(_) => {
                            let result = PyDict::new(py);
                            result.set_item("success", true).unwrap();
                            result.set_item("severity", 0).unwrap();
                            result.set_item("message", format!("Cache bust completed for {}", package)).unwrap();
                            result.set_item("component", "cache_buster").unwrap();
                            result.set_item("recovery_action", py.None()).unwrap();
                            result.into()
                        }
                        Err(error) => {
                            let result = PyDict::new(py);
                            result.set_item("success", false).unwrap();
                            result.set_item("severity", error.severity as u8).unwrap();
                            result.set_item("message", error.message).unwrap();
                            result.set_item("component", error.component).unwrap();
                            result.set_item("recovery_action", error.recovery_action).unwrap();
                            result.into()
                        }
                    };
                    results.push(result_dict);
                }).unwrap();
            }
            
            Ok(results)
        }

        pub fn get_health_metrics(&self) -> PyResult<String> {
            let metrics = serde_json::json!({
                "system_status": "operational",
                "supported_languages": ["node", "python", "c", "cpp", "gosilang"],
                "constitutional_compliance": true,
                "polycore_version": "v2",
                "python_binding_version": "1.0.0"
            });
            
            Ok(metrics.to_string())
        }

        #[staticmethod]
        pub fn version() -> &'static str {
            env!("CARGO_PKG_VERSION")
        }

        #[staticmethod]
        pub fn supported_languages() -> Vec<&'static str> {
            vec!["node", "python", "c", "cpp", "gosilang"]
        }

        #[staticmethod]
        pub fn severity_levels() -> PyResult<String> {
            let levels = serde_json::json!({
                "ok": { "range": "0-3", "action": "monitor", "level": 0 },
                "warning": { "range": "3-6", "action": "cache_bust", "level": 3 },
                "danger": { "range": "6-9", "action": "force_rebuild", "level": 6 },
                "critical": { "range": "9-12", "action": "restart_process", "level": 9 },
                "panic": { "range": "12+", "action": "emergency_protocols", "level": 12 }
            });
            
            Ok(levels.to_string())
        }
    }

    /// Python module initialization
    #[pymodule]
    fn bustcall(py: Python, m: &PyModule) -> PyResult<()> {
        m.add_class::<PyBustCall>()?;
        m.add("__version__", env!("CARGO_PKG_VERSION"))?;
        m.add("SUPPORTED_LANGUAGES", vec!["node", "python", "c", "cpp", "gosilang"])?;
        
        // Add severity level constants
        m.add("SEVERITY_OK", 0)?;
        m.add("SEVERITY_WARNING", 3)?;
        m.add("SEVERITY_DANGER", 6)?;
        m.add("SEVERITY_CRITICAL", 9)?;
        m.add("SEVERITY_PANIC", 12)?;
        
        Ok(())
    }
}

// =============================================================================
// WebAssembly FFI (Future Implementation)
// =============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm_bindings {
    use wasm_bindgen::prelude::*;
    use crate::{BustCall, BustCallConfig};

    #[wasm_bindgen]
    pub struct WasmBustCall {
        inner: BustCall,
    }

    #[wasm_bindgen]
    impl WasmBustCall {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            Self {
                inner: BustCall::new(BustCallConfig::default()),
            }
        }

        #[wasm_bindgen]
        pub fn bust_cache(&mut self, package: &str, language: &str) -> String {
            match self.inner.execute_bust(package, language) {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "message": "Cache bust completed successfully"
                }).to_string(),
                Err(error) => serde_json::json!({
                    "success": false,
                    "severity": error.severity as u8,
                    "message": error.message,
                    "component": error.component
                }).to_string(),
            }
        }

        #[wasm_bindgen]
        pub fn version() -> String {
            env!("CARGO_PKG_VERSION").to_string()
        }
    }
}

// =============================================================================
// GosiLang Integration (OBINexus RIFT Architecture)
// =============================================================================

/// GosiLang FFI interface per OBINexus RIFT architecture
#[no_mangle]
pub extern "C" fn gosilang_bustcall_init() -> *mut BustCall {
    let config = BustCallConfig {
        enable_self_healing: true,
        enable_panic_restart: true,
        max_retries: 5,
        constitutional_compliance: true,
    };
    
    Box::into_raw(Box::new(BustCall::new(config)))
}

#[no_mangle]
pub extern "C" fn gosilang_bustcall_execute(
    instance: *mut BustCall,
    package: *const c_char,
) -> CBustResult {
    if instance.is_null() || package.is_null() {
        return CBustResult {
            success: 0,
            severity: 12,
            message: CString::new("Invalid GosiLang FFI input").unwrap().into_raw(),
            component: CString::new("gosilang_ffi").unwrap().into_raw(),
            recovery_action: CString::new("Check GosiLang integration").unwrap().into_raw(),
        };
    }

    let instance = unsafe { &mut *instance };
    let package_str = unsafe { CStr::from_ptr(package).to_string_lossy() };

    // GosiLang uses "gosilang" as the language identifier
    match instance.execute_bust(&package_str, "gosilang") {
        Ok(_) => CBustResult {
            success: 1,
            severity: 0,
            message: CString::new("GosiLang cache bust completed").unwrap().into_raw(),
            component: CString::new("gosilang_cache_buster").unwrap().into_raw(),
            recovery_action: std::ptr::null_mut(),
        },
        Err(error) => CBustResult {
            success: 0,
            severity: error.severity as c_uint,
            message: CString::new(format!("GosiLang error: {}", error.message)).unwrap().into_raw(),
            component: CString::new(format!("gosilang_{}", error.component)).unwrap().into_raw(),
            recovery_action: error.recovery_action
                .map(|action| CString::new(format!("GosiLang recovery: {}", action)).unwrap().into_raw())
                .unwrap_or(std::ptr::null_mut()),
        },
    }
}

// =============================================================================
// FFI Utilities and Helpers
// =============================================================================

/// Convert Rust error to standardized FFI error format
pub fn error_to_ffi_result(error: BustCallError) -> CBustResult {
    CBustResult {
        success: 0,
        severity: error.severity as c_uint,
        message: CString::new(error.message).unwrap().into_raw(),
        component: CString::new(error.component).unwrap().into_raw(),
        recovery_action: error.recovery_action
            .map(|action| CString::new(action).unwrap().into_raw())
            .unwrap_or(std::ptr::null_mut()),
    }
}

/// Get FFI interface version for compatibility checking
#[no_mangle]
pub extern "C" fn bustcall_ffi_version() -> *mut c_char {
    CString::new(format!("bustcall-ffi-{}", env!("CARGO_PKG_VERSION")))
        .unwrap()
        .into_raw()
}

/// Get supported language ecosystems
#[no_mangle]
pub extern "C" fn bustcall_supported_languages() -> *mut c_char {
    CString::new("node,python,c,cpp,gosilang,wasm")
        .unwrap()
        .into_raw()
}

/// Check constitutional compliance capability
#[no_mangle]
pub extern "C" fn bustcall_constitutional_compliance_enabled() -> c_int {
    1 // Always enabled for OBINexus compliance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_ffi_initialization() {
        let config = CBustConfig {
            enable_self_healing: 1,
            enable_panic_restart: 1,
            max_retries: 3,
            constitutional_compliance: 1,
        };
        
        let instance = bustcall_init(&config);
        assert!(!instance.is_null());
        
        bustcall_free(instance);
    }

    #[test]
    fn test_ffi_error_conversion() {
        let error = BustCallError {
            severity: SeverityLevel::Warning,
            message: "Test error".to_string(),
            component: "test_component".to_string(),
            recovery_action: Some("Test recovery".to_string()),
        };
        
        let ffi_result = error_to_ffi_result(error);
        assert_eq!(ffi_result.success, 0);
        assert_eq!(ffi_result.severity, 3);
    }
}