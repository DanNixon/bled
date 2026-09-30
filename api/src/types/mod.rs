//! Domain types used by the bled API.

mod channel_mask;
mod channel_range;
mod config;
mod device_info;
mod file_io_control;
mod led_buffer_control;
mod pixel;

pub use channel_mask::*;
pub use channel_range::*;
pub use config::*;
pub use device_info::*;
pub use file_io_control::*;
pub use led_buffer_control::*;
pub use pixel::*;

use thiserror::Error;

/// Error returned by channel operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelError {
    /// A channel index is greater than or equal to the maximum supported channel count.
    #[error("channel index {channel} is invalid (max: {max})")]
    InvalidChannel { channel: u8, max: usize },
}
