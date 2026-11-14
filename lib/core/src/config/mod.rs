mod brightness_limit;
mod fixture;
mod layouts;
mod span;

pub use brightness_limit::*;
pub use fixture::*;
pub use layouts::*;
pub use span::*;

/// Maximum length of any name.
/// Should never be more than the maximum length of an allowable BLE device name.
pub const MAX_NAME_LEN: usize = 16;
