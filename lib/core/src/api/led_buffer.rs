//! LED buffer API data types.

use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct LedBufferInformation {
    /// Capacity of the buffer in bytes.
    capacity: u32,

    /// Used size of the buffer in bytes.
    size: u32,
}

impl LedBufferInformation {
    #[must_use]
    pub fn new(capacity: u32, size: u32) -> Self {
        Self { capacity, size }
    }
}
