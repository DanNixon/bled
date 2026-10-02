//! File IO API
//!
//! Allows basic filesystem operations.
//!
//! ## Stat a file
//!
//! 1. Send `FileIoControl::Stat` to the `FILE_IO_CONTROL` characteristic.
//! 2. Receive a CBOR encoded `FileStatResponse` from the `FILE_IO_DATA` characteristic.
//!
//! ## Reading a file
//!
//! 1. Send `FileIoControl::Read` to the `FILE_IO_CONTROL` characteristic.
//! 2. Read from the `FILE_IO_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of file is reached.
//!
//! ## Creating a file before writing
//!
//! 1. Send `FileIoControl::Create` to the `FILE_IO_CONTROL` characteristic.
//!
//! ## Writing a file
//!
//! 1. Send `FileIoControl::Write` to the `FILE_IO_CONTROL` characteristic.
//! 2. Write data to the `FILE_IO_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of file is reached.
//!
//! ## Deleting a file
//!
//! 1. Send `FileIoControl::Delete` to the `FILE_IO_CONTROL` characteristic.

use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};

pub const MAX_PATH_LEN: usize = 64;

/// Command sent to the File IO Control GATT characteristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FileIoControl {
    /// Read a chunk from the specified file at the given offset.
    Read {
        path: String<MAX_PATH_LEN>,
        offset: u32,
    },

    /// Write a chunk to the specified file at the given offset.
    Write {
        path: String<MAX_PATH_LEN>,
        offset: u32,
    },

    /// Resets the read/write state machine. Will not roll back any in-progress write.
    Reset,

    /// Stat the specified file.
    Stat { path: String<MAX_PATH_LEN> },

    /// Create the specified file and preallocate the specified size.
    Create {
        path: String<MAX_PATH_LEN>,
        size: u32,
    },

    /// Delete the specified file.
    Delete { path: String<MAX_PATH_LEN> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct FileStatResponse {
    /// Size of the file in bytes.
    size: u32,
}

impl FileStatResponse {
    #[must_use]
    pub fn new(size: u32) -> Self {
        Self { size }
    }
}
