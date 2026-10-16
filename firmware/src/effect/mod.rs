//! WebAssembly-backed lighting effects.

mod instance;
mod runtime;

use instance::EffectInstance;
pub(crate) use runtime::{load_active_effect, task, unload_active_effect};
