//! WebAssembly bindings for `bled-core`.
//!
//! Provides the `teeny-rpc` client and bindings for Web Bluetooth serial.

mod constants;
mod device;
mod effect_manifest;
mod rpc;

pub use constants::*;
pub use device::*;
pub use effect_manifest::*;
pub use rpc::*;
