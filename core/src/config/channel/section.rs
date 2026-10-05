use crate::config::{MAX_NAME_LEN, PixelLayout, PositionalPixelSpan, SizedPixelSpan};
use core::ops::Range;
use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};

/// Configuration for an LED output channel, i.e. a physical channel of LEDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ChannelSectionConfig {
    /// Friendly name for the section
    name: String<{ MAX_NAME_LEN }>,

    /// Start index of the section
    start: u32,

    /// Layout of the section
    layout: PixelLayout,
}

impl ChannelSectionConfig {
    pub fn new(name: String<{ MAX_NAME_LEN }>, start: u32, layout: PixelLayout) -> Self {
        Self {
            name,
            start,
            layout,
        }
    }
}

impl SizedPixelSpan for ChannelSectionConfig {
    fn len(&self) -> u32 {
        self.layout.len()
    }
}

impl PositionalPixelSpan for ChannelSectionConfig {
    fn range(&self) -> Range<u32> {
        self.start..self.start + self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_types_have_known_sizes() {
        assert_eq!(core::mem::size_of::<ChannelSectionConfig>(), 40);
    }
}
