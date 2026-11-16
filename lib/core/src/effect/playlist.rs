use crate::PathString;
use core::{ops::Deref, time::Duration};
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Maximum number of entries in an effect playlist.
pub const MAX_PLAYLIST_ENTRIES: usize = 16;

/// Maximum allowable size of an effect playlist in bytes when encoded as JSON.
pub const MAX_PLAYLIST_JSON_SIZE: usize = 1024 * 4;

/// Maximum allowable size of an effect playlist in bytes when encoded as Postcard.
pub const MAX_PLAYLIST_POSTCARD_SIZE: usize = 1024 * 2;

/// A playlist of multiple effects
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EffectPlaylist(Vec<EffectPlaylistEntry, MAX_PLAYLIST_ENTRIES>);

impl EffectPlaylist {
    #[must_use]
    pub const fn new(entries: Vec<EffectPlaylistEntry, MAX_PLAYLIST_ENTRIES>) -> Self {
        Self(entries)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[EffectPlaylistEntry] {
        self.0.as_slice()
    }

    /// Removes the entry at `index` from the playlist.
    pub fn remove(&mut self, index: usize) -> EffectPlaylistEntry {
        self.0.remove(index)
    }
}

impl Deref for EffectPlaylist {
    type Target = [EffectPlaylistEntry];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl AsRef<[EffectPlaylistEntry]> for EffectPlaylist {
    fn as_ref(&self) -> &[EffectPlaylistEntry] {
        self.0.as_slice()
    }
}

/// A playlist entry for a single effect
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectPlaylistEntry {
    /// Path to the effect module or preset
    path: PathString,

    /// Amount of time that this entry should be active for (encoded in milliseconds)
    #[serde(default, with = "optional_duration_ms")]
    duration: Option<Duration>,

    /// Action that should be taken when this entry is finished
    #[serde(default)]
    after: NextPlaylistEntryAction,
}

impl EffectPlaylistEntry {
    #[must_use]
    pub const fn new(
        path: PathString,
        duration: Option<Duration>,
        after: NextPlaylistEntryAction,
    ) -> Self {
        Self {
            path,
            duration,
            after,
        }
    }
}

/// Action when an entry is finished that will decide what to do next in the playlist
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum NextPlaylistEntryAction {
    /// Move to an item in the playlist relative to the current item
    Shift(i8),

    /// Select a random item from the playlist
    Random,
}

impl NextPlaylistEntryAction {
    /// Resolves the next index in a playlist of length `len` from `current_index`.
    ///
    /// Returns `None` if `len == 0`.
    #[must_use]
    pub fn resolve_index(&self, current_index: usize, len: usize, random: u64) -> Option<usize> {
        if len == 0 {
            return None;
        }
        match *self {
            Self::Shift(offset) => {
                let len_i = len as isize;
                let current_i = (current_index % len) as isize;
                let next = (current_i + offset as isize).rem_euclid(len_i);
                Some(next as usize)
            }
            Self::Random => Some((random % (len as u64)) as usize),
        }
    }

    /// Resolves the next index in a playlist of length `remaining_len` after the entry
    /// at `removed_index` was removed.
    ///
    /// For `Shift(offset)`:
    /// - Positive shifts (`offset > 0`) compensate for the removed element by subtracting 1
    ///   because the element that followed `removed_index` is now at `removed_index`.
    /// - Zero or negative shifts (`offset <= 0`) resolve relative to `removed_index`.
    ///
    /// Returns `None` if `remaining_len == 0`.
    #[must_use]
    pub fn resolve_index_after_removal(
        &self,
        removed_index: usize,
        remaining_len: usize,
        random: u64,
    ) -> Option<usize> {
        if remaining_len == 0 {
            return None;
        }
        match *self {
            Self::Shift(offset) => {
                let len_i = remaining_len as isize;
                let base_i = removed_index as isize;
                let adjusted_offset = if offset > 0 {
                    (offset as isize) - 1
                } else {
                    offset as isize
                };
                let next = (base_i + adjusted_offset).rem_euclid(len_i);
                Some(next as usize)
            }
            Self::Random => Some((random % (remaining_len as u64)) as usize),
        }
    }
}

impl Default for NextPlaylistEntryAction {
    fn default() -> Self {
        // i.e. the next item in the playlist
        Self::Shift(1)
    }
}

mod optional_duration_ms {
    use core::time::Duration;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = duration.map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        millis.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = Option::<u64>::deserialize(deserializer)?;
        Ok(millis.map(Duration::from_millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_sizes() {
        assert_eq!(core::mem::size_of::<EffectPlaylist>(), 1544);
        assert_eq!(core::mem::size_of::<EffectPlaylistEntry>(), 96);
    }

    fn biggest_playlist() -> EffectPlaylist {
        let path: PathString = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .try_into()
            .unwrap();
        let entries: Vec<EffectPlaylistEntry, MAX_PLAYLIST_ENTRIES> = (0..MAX_PLAYLIST_ENTRIES)
            .map(|i| {
                EffectPlaylistEntry::new(
                    path.clone(),
                    Some(Duration::from_millis(u64::MAX)),
                    if i % 2 == 0 {
                        NextPlaylistEntryAction::Shift(-128)
                    } else {
                        NextPlaylistEntryAction::Random
                    },
                )
            })
            .collect();
        EffectPlaylist::new(entries)
    }

    #[test]
    fn max_json_size_is_reasonable() {
        let playlist = biggest_playlist();
        let mut buf = [0; MAX_PLAYLIST_JSON_SIZE];
        let n = crate::io::encode_json(&playlist, &mut buf).unwrap();
        assert!(n < MAX_PLAYLIST_JSON_SIZE - 256);
    }

    #[test]
    fn max_postcard_size_is_reasonable() {
        let playlist = biggest_playlist();
        let mut buf = [0; MAX_PLAYLIST_POSTCARD_SIZE];
        let n = crate::io::encode_postcard(&playlist, &mut buf).unwrap();
        assert!(n < MAX_PLAYLIST_POSTCARD_SIZE);
    }

    #[test]
    fn empty_playlist_json_round_trip() {
        let playlist = EffectPlaylist::default();
        let mut buf = [0u8; 64];
        let n = crate::io::encode_json(&playlist, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"[]");

        let decoded: EffectPlaylist = crate::io::decode_json(&buf[..n]).unwrap();
        assert_eq!(decoded, playlist);
        assert!(decoded.is_empty());
    }

    #[test]
    fn omitted_fields_default_properly() {
        let playlist: EffectPlaylist = crate::io::decode_json(
            br#"[
                {"path": "/effects/a.wasm"},
                {"path": "/effects/b.json", "duration": 30000, "after": "random"},
                {"path": "/effects/c.wasm", "duration": 5000, "after": {"shift": -1}}
            ]"#,
        )
        .unwrap();

        assert_eq!(playlist.len(), 3);
        assert_eq!(playlist[0].path().as_str(), "/effects/a.wasm");
        assert_eq!(*playlist[0].duration(), None);
        assert_eq!(*playlist[0].after(), NextPlaylistEntryAction::Shift(1));

        assert_eq!(playlist[1].path().as_str(), "/effects/b.json");
        assert_eq!(*playlist[1].duration(), Some(Duration::from_millis(30_000)));
        assert_eq!(*playlist[1].after(), NextPlaylistEntryAction::Random);

        assert_eq!(playlist[2].path().as_str(), "/effects/c.wasm");
        assert_eq!(*playlist[2].duration(), Some(Duration::from_millis(5_000)));
        assert_eq!(*playlist[2].after(), NextPlaylistEntryAction::Shift(-1));
    }

    #[test]
    fn shift_wraps_around() {
        let shift_next = NextPlaylistEntryAction::Shift(1);
        assert_eq!(shift_next.resolve_index(0, 3, 0), Some(1));
        assert_eq!(shift_next.resolve_index(2, 3, 0), Some(0));

        let shift_prev = NextPlaylistEntryAction::Shift(-1);
        assert_eq!(shift_prev.resolve_index(0, 3, 0), Some(2));
        assert_eq!(shift_prev.resolve_index(2, 3, 0), Some(1));

        let shift_stay = NextPlaylistEntryAction::Shift(0);
        assert_eq!(shift_stay.resolve_index(1, 3, 0), Some(1));

        assert_eq!(shift_next.resolve_index(0, 0, 0), None);
    }

    #[test]
    fn shift_after_removal_resolves_expected_entry() {
        // Playlist [A, B, C] (len 3). Remove A at index 0 -> [B, C] (len 2).
        // Shift(1) should advance to B (now at index 0).
        let shift_next = NextPlaylistEntryAction::Shift(1);
        assert_eq!(shift_next.resolve_index_after_removal(0, 2, 0), Some(0));

        // Remove C at index 2 -> [A, B] (len 2).
        // Shift(1) should wrap around to A (index 0).
        assert_eq!(shift_next.resolve_index_after_removal(2, 2, 0), Some(0));

        // Remove B at index 1 -> [A, C] (len 2).
        // Shift(-1) should move back to A (index 0).
        let shift_prev = NextPlaylistEntryAction::Shift(-1);
        assert_eq!(shift_prev.resolve_index_after_removal(1, 2, 0), Some(0));

        // Remove A at index 0 -> [B, C] (len 2).
        // Shift(-1) should wrap around to C (index 1).
        assert_eq!(shift_prev.resolve_index_after_removal(0, 2, 0), Some(1));

        assert_eq!(shift_next.resolve_index_after_removal(0, 0, 0), None);
    }

    #[test]
    fn random_selects_within_bounds() {
        let random = NextPlaylistEntryAction::Random;
        assert_eq!(random.resolve_index(0, 4, 7), Some(3));
        assert_eq!(random.resolve_index_after_removal(0, 4, 6), Some(2));
        assert_eq!(random.resolve_index(0, 0, 7), None);
    }
}
