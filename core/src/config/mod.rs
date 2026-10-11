mod device;
mod fixture;
mod layouts;
mod span;

pub use device::*;
pub use fixture::*;
pub use layouts::*;
pub use span::*;

/// Maximum length of any name.
/// Shoudl never be more than the maximum length of an allowable BLE device name.
pub const MAX_NAME_LEN: usize = 16;

/// Maximum number of pixels that may be configured on a single device.
pub const MAX_FIXTURE_COUNT: usize = 8;

/// Maximum number of spanls that may be used in a single fixture.
pub const MAX_SPAN_PER_FIXTURE_COUNT: usize = 8;

/// Maximum allowable size of the config file in bytes when encoded as JSON.
pub const MAX_CONFIG_JSON_SIZE: usize = 1024 * 4; // TODO: calculate

/// Maximum allowable size of the config file in bytes when encoded as CBOR.
pub const MAX_CONFIG_CBOR_SIZE: usize = 1024 * 2; // TODO: calculate
