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

/// Maximum allowable size of the config file in bytes when encoded as JSON.
pub const MAX_CONFIG_JSON_SIZE: usize = 1024 * 4; // TODO: calculate

/// Maximum allowable size of the config file in bytes when encoded as CBOR.
pub const MAX_CONFIG_CBOR_SIZE: usize = 1024 * 2; // TODO: calculate
