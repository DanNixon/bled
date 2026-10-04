//! WebAssembly bindings for `bled-core`.
//!
//! Provides protocol encoding, decoding, and client framing for Web Bluetooth
//! and other JavaScript/TypeScript environments.

pub mod commit;
pub mod config;
pub mod device;
pub mod file_io;
pub mod gatt;
pub mod led_buffer;

pub use commit::*;
pub use config::*;
pub use device::*;
pub use file_io::*;
pub use gatt::*;
pub use led_buffer::*;
