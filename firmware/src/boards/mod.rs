//! Board specific hardware configurations.

#[cfg(feature = "board-backpack-pico2")]
mod backpack_pico2;
#[cfg(feature = "board-backpack-pico2")]
pub(crate) use backpack_pico2::*;

#[cfg(feature = "board-backpack-pico2w")]
mod backpack_pico2w;
#[cfg(feature = "board-backpack-pico2w")]
pub(crate) use backpack_pico2w::*;
