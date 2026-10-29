//! File IO API data types.

use crate::PathString;
use getset::Getters;
use serde::{Deserialize, Serialize};

pub const MAX_PATH_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DirEntry {
    /// Name of the file or directory.
    name: PathString,

    kind: DirEntryType,
}

impl DirEntry {
    #[must_use]
    pub fn new(name: PathString, kind: DirEntryType) -> Self {
        Self { name, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DirEntryType {
    File {
        /// Size of the file in bytes.
        size: u64,
    },
    Directory,
}
