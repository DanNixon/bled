use crate::config::{ChannelSpan, ChannelSectionLayout, MAX_NAME_LEN};
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
    layout: ChannelSectionLayout,
}

impl ChannelSectionConfig {
    pub fn new(name: String<{ MAX_NAME_LEN }>, start: u32, layout: ChannelSectionLayout) -> Self {
        Self {
            name,
            start,
            layout,
        }
    }
}

impl ChannelSpan for ChannelSectionConfig {
    fn len(&self) -> u32 {
        self.layout.len()
    }
}
