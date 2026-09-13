//! Independently compiled semantic example; no host/model/kernel dependencies.
#![deny(unsafe_code)]

mod column;
#[cfg(feature = "plan-graphics")]
mod plan_graphics;

// Only the three Wasm export attributes need an exception. No native exports.
#[cfg(any(target_arch = "wasm32", test))]
#[allow(unsafe_code)]
mod abi;

pub use column::respond;
