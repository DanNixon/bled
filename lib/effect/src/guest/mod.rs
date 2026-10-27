//! API used by effects running inside the WASM guest.

mod context;
mod entrypoint;
mod frame;
mod runtime;

pub use context::Context;
pub use frame::Frame;
pub use runtime::Runtime;

use bled_core::config::Fixtures;
use bled_core::effect::abi::export::StepResult;
use core::cell::UnsafeCell;

/// The interface an effect implements to run inside the WASM guest.
pub trait Effect: Sized {
    /// Create the effect from the device's fixture configuration.
    ///
    /// The runtime reports initialization failures as a generic ABI error, so
    /// effects return an opaque unit error here.
    #[allow(clippy::result_unit_err)]
    fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()>;

    /// Return the desired interval until the next step, in milliseconds.
    fn step_time(&self, ctx: &Context<'_>) -> u32;

    /// Advance the effect by one frame.
    fn step(&mut self, ctx: &mut Context<'_>, timestamp_ms: u32) -> StepResult;
}

/// Single-threaded static state for a WASM guest.
pub struct GuestState<T>(UnsafeCell<T>);

impl<T> GuestState<T> {
    /// Create new guest state.
    pub const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    /// Access the state for the duration of a closure.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // The WASM guest is single-threaded and cannot re-enter its own exports.
        f(unsafe { &mut *self.0.get() })
    }
}

// SAFETY: access is restricted to `with`, and the WASM guest is single-threaded.
unsafe impl<T> Sync for GuestState<T> {}

/// Common imports for effect implementations.
pub mod prelude {
    pub use super::{Context, Effect, Frame};
    pub use crate::wasm_effect;
    pub use bled_core::{RGB8, config::Fixtures, effect::abi::export::StepResult};
}
