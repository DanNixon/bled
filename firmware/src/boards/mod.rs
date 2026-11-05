//! Board specific hardware configurations.

#[cfg(feature = "board-backpack-pico2")]
mod backpack_pico2;
#[cfg(feature = "board-backpack-pico2")]
pub(crate) use backpack_pico2::*;

#[cfg(feature = "board-backpack-pico2w")]
mod backpack_pico2w;
#[cfg(feature = "board-backpack-pico2w")]
pub(crate) use backpack_pico2w::*;

#[cfg(feature = "board-stamp")]
mod stamp;
#[cfg(feature = "board-stamp")]
pub(crate) use stamp::*;

#[cfg(feature = "board-stamp-rm2")]
mod stamp_rm2;
#[cfg(feature = "board-stamp-rm2")]
pub(crate) use stamp_rm2::*;

#[cfg(any(feature = "board-stamp", feature = "board-stamp-rm2"))]
pub(crate) mod stamp_bms;
