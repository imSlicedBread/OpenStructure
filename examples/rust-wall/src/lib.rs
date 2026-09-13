#![deny(unsafe_code)]
mod wall;
pub use wall::respond;
// Reuse the small audited export adapter; semantic code remains forbidden unsafe.
#[cfg(any(target_arch = "wasm32", test))]
#[allow(unsafe_code)]
#[path = "../../rust-column/src/abi.rs"]
mod abi;
