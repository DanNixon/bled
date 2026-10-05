use crate::{
    api::ChannelMask,
    config::{ChannelSectionConfig, MAX_CHANNEL_COUNT, MAX_NAME_LEN},
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
pub struct TargetConfig(Vec<TargetSectionConfig, MAX_TARGETS>);

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
    channel: ChannelMask,
    range: ChannelSectionConfig,
}
