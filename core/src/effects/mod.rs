mod fixed_color;

pub use fixed_color::*;

use crate::{
    api::ChannelMask,
    config::{MAX_CHANNEL_COUNT, MAX_NAME_LEN},
};
use core::{ops::Range, time::Duration};
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

pub trait Effect: Sized {
    type Config;
    type Error;

    /// Creates a new effect with the given configuration.
    fn new(config: Self::Config) -> Result<Self, Self::Error>;

    /// Steps the effect forward, returning the next step result.
    fn step(&mut self, now: Duration) -> Result<StepResult, Self::Error>;
}

/// The maximum number of targets that can be configured for a single effect.
pub const MAX_TARGETS: usize = MAX_CHANNEL_COUNT;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TargetConfig {
    Channels {
        channels: Vec<String<MAX_NAME_LEN>, MAX_TARGETS>,
    },

    Section {
        sections: Vec<TargetSectionConfig, MAX_TARGETS>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TargetSectionConfig {
    channel: String<MAX_NAME_LEN>,
    section: String<MAX_NAME_LEN>,
}

/// A single target for an effect, represented as a continuous range in a channel.
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Target {
    channel: ChannelMask,
    range: Range<u32>,
}

/// The result of a single step of an effect.
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct StepResult {
    /// Channels that need to be rendered.
    render: ChannelMask,

    /// The time that the next step should take place.
    next: Duration,
}
