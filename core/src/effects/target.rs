use crate::{
    api::ChannelMask,
    config::{ChannelSectionConfig, MAX_CHANNEL_COUNT, MAX_NAME_LEN, PixelLayout},
};
use getset::Getters;
use heapless::{String, Vec};
use nutype::nutype;
use serde::{Deserialize, Serialize};

/// The maximum number of targets that can be configured for a single effect.
pub const MAX_TARGETS: usize = MAX_CHANNEL_COUNT;

#[nutype(
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct TargetConfigs(Vec<TargetSectionConfig, MAX_TARGETS>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct TargetSectionConfig {
    channel: String<MAX_NAME_LEN>,
    section: String<MAX_NAME_LEN>,
}

#[nutype(
    derive(Debug, Clone, PartialEq, Eq),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct Targets(Vec<Target, MAX_TARGETS>);

/// A single target for an effect, represented as a continuous range in a channel.
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Target {
    /// Channel the target resides on
    channel: ChannelMask,

    /// Start index of the section
    start: u32,

    /// Layout of the section
    layout: PixelLayout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_types_have_known_sizes() {
        assert_eq!(core::mem::size_of::<TargetConfigs>(), 392);
        assert_eq!(core::mem::size_of::<TargetSectionConfig>(), 48);
        assert_eq!(core::mem::size_of::<Targets>(), 136);
        assert_eq!(core::mem::size_of::<Target>(), 16);
    }
}
