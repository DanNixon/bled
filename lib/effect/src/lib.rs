//! Helper library for building BLED effects as Rust WebAssembly modules.

#![cfg_attr(not(feature = "std"), no_std)]

#[doc(hidden)]
pub use bled_core;

pub mod guest;

#[cfg(feature = "std")]
pub mod presets;
