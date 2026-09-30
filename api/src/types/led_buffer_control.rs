//! LED raw buffer API
//!
//! Allows reading and writing of LED raw buffer data over BLE GATT.
//!
//! ## Reading
//!
//! 1. Send `LedBufferControl::Read` to the `LED_BUFFER_CONTROL` characteristic.
//! 2. Read from the `LED_BUFFER_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of buffer is reached.
//!
//! ## Writing
//!
//! 1. Send `LedBufferControl::Write` to the `LED_BUFFER_CONTROL` characteristic.
//! 2. Write data to the `LED_BUFFER_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of buffer is reached.

use getset::Getters;
use serde::{Deserialize, Serialize};

/// Command sent to the LED Buffer Control GATT characteristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LedBufferControl {
    /// Read a chunk from the buffer at the given offset.
    Read { offset: u32 },

    /// Write a chunk to the buffer at the given offset.
    Write { offset: u32 },

    /// Resets the read/write state machine. Will not roll back any in-progress write.
    Reset,

    /// Returns the current buffer information.
    Info,
}

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
