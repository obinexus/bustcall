//! Foreign Function Interface exports for Python and C bindings

#[cfg(feature = "python-bindings")]
pub mod python;

#[cfg(feature = "ffi")]
pub mod c;
