mod channel;
mod device;

pub use channel::*;
pub use device::*;

/// Maximum length of any name.
/// Limited by the maximum length of an allowable BLE device name.
pub const MAX_NAME_LEN: usize = 16;

/// Maximum number of LED channels addressable on one device.
pub const MAX_CHANNEL_COUNT: usize = 8;

/// Maximum number of sections per channel.
pub const MAX_SECTIONS_PER_CHANNEL: usize = 16;
