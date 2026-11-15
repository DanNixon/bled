use crate::PathString;
use getset::Getters;
use serde::{Deserialize, Serialize};

/// Information about the currently loaded playlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ActivePlaylistInfo {
    /// Path of the playlist file on the SD card.
    path: PathString,

    /// Current entry index in the active playlist, or `None` if the playlist is empty.
    position: Option<u8>,
}

impl ActivePlaylistInfo {
    #[must_use]
    pub const fn new(path: PathString, position: Option<u8>) -> Self {
        Self { path, position }
    }
}
