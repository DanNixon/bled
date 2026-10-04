use crate::config::MAX_CHANNEL_COUNT;
use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    /// The set of LED channel indices selected by a Commit write.
    ///
    /// Bit `n` selects channel index `n`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
    pub struct ChannelMask: u8 {
        /// Every valid channel index.
        const ALL = ((1u16 << MAX_CHANNEL_COUNT) - 1) as u8;
    }
}

// Compile-time check that `MAX_CHANNEL_COUNT` fits in `ChannelMask`
const _: () = assert!(
    MAX_CHANNEL_COUNT <= u8::BITS as usize,
    "MAX_CHANNEL_COUNT exceeds ChannelMask capacity",
);

impl ChannelMask {
    /// Returns the bit for `index`, or `None` when it is out of range.
    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        if index < MAX_CHANNEL_COUNT {
            Some(Self::from_bits_retain(1u8 << index))
        } else {
            None
        }
    }

    /// Creates a mask from an iterator of channel indices, or `None` if any index is out of bounds.
    #[must_use]
    pub fn from_indices(indices: impl IntoIterator<Item = usize>) -> Option<Self> {
        let mut mask = Self::empty();
        for index in indices {
            let bit = Self::from_index(index)?;
            mask |= bit;
        }
        Some(mask)
    }

    /// Returns whether `index` is selected.
    #[must_use]
    pub const fn contains_index(self, index: usize) -> bool {
        match Self::from_index(index) {
            Some(mask) => self.contains(mask),
            None => false,
        }
    }

    /// Iterates over every selected channel index.
    pub fn iter_indices(self) -> impl Iterator<Item = usize> {
        (0..MAX_CHANNEL_COUNT).filter(move |&idx| self.contains_index(idx))
    }

    /// Returns the number of selected channels.
    #[must_use]
    pub const fn count(self) -> usize {
        self.bits().count_ones() as usize
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for ChannelMask {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "ChannelMask(0x{:02x})", self.bits());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_masks_use_zero_based_indices() {
        let mut mask = ChannelMask::empty();
        for index in 0..MAX_CHANNEL_COUNT {
            let bit = ChannelMask::from_index(index).unwrap();
            assert_eq!(bit.bits(), 1 << index);
            assert!(!mask.contains_index(index));
            mask |= bit;
            assert!(mask.contains_index(index));
        }
        assert_eq!(mask, ChannelMask::ALL);
        assert_eq!(ChannelMask::from_index(MAX_CHANNEL_COUNT), None);
        assert!(!mask.contains_index(usize::MAX));
        assert_eq!(mask.count(), MAX_CHANNEL_COUNT);
        let indices: heapless::Vec<usize, MAX_CHANNEL_COUNT> = mask.iter_indices().collect();
        assert_eq!(indices.as_slice(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(
            ChannelMask::from_indices(0..MAX_CHANNEL_COUNT),
            Some(ChannelMask::ALL)
        );
        assert_eq!(ChannelMask::from_indices([MAX_CHANNEL_COUNT]), None);
    }
}
