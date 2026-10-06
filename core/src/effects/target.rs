use crate::{
    api::{ChannelNumber, PixelIndex},
    config::{MAX_NAME_LEN, PixelLayout, PositionalPixelSpan, SizedPixelSpan},
};
use core::ops::Range;
use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct TargetSectionConfig {
    channel: String<MAX_NAME_LEN>,
    section: String<MAX_NAME_LEN>,
}

/// A single target for an effect, represented as a continuous range in a channel.
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Target {
    /// Channel the target resides on
    channel: ChannelNumber,

    /// Start index of the section
    start: PixelIndex,

    /// Layout of the section
    layout: PixelLayout,
}

impl SizedPixelSpan for Target {
    fn len(&self) -> usize {
        self.layout.len()
    }
}

impl PositionalPixelSpan for Target {
    fn range(&self) -> Range<usize> {
        self.start as usize..self.start as usize + self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_types_have_known_sizes() {
        assert_eq!(core::mem::size_of::<TargetSectionConfig>(), 48);
        assert_eq!(core::mem::size_of::<Target>(), 16);
    }
}
