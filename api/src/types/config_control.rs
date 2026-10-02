//! Config API
//!
//! Allows reading active configuration from the device.
//! Config data is returned in its CBOR encoded form.
//!
//! ## Stat config
//!
//! 1. Send `ConfigControl::Stat` to the `CONFIG_CONTROL` characteristic.
//! 2. Receive a CBOR encoded `ConfigStatResponse` from the `CONFIG_DATA` characteristic.
//!
//! ## Reading config
//!
//! 1. Send `ConfigControl::Read` to the `CONFIG_CONTROL` characteristic.
//! 2. Read from the `CONFIG_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of config is reached.

use getset::Getters;
use serde::{Deserialize, Serialize};

/// Command sent to the Config Control GATT characteristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigControl {
    /// Read a chunk from the config the given offset.
    Read { offset: u32 },

    /// Resets the read state machine.
    Reset,

    /// Stat the config data.
    Stat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ConfigStatResponse {
    /// Size of the config in bytes.
    size: u32,
}

impl ConfigStatResponse {
    #[must_use]
    pub fn new(size: u32) -> Self {
        Self { size }
    }
}
