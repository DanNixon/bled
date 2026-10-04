use crate::{ChannelPartOps, ChannelSectionMode, MAX_NAME_LEN};
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

    /// Type of the section
    mode: ChannelSectionMode,
}

impl ChannelSectionConfig {
    pub fn new(name: String<{ MAX_NAME_LEN }>, start: u32, mode: ChannelSectionMode) -> Self {
        Self { name, start, mode }
    }
}

impl ChannelPartOps for ChannelSectionConfig {
    fn len(&self) -> u32 {
        self.mode.len()
    }
}
