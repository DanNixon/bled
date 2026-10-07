use crate::api::ChannelMask;
use core::time::Duration;
use getset::Getters;

/// The result of a single step of an effect or the effect runner.
#[derive(Debug, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct StepResult {
    /// Channels that need to be rendered.
    updated_channels: ChannelMask,

    /// The time that the next step should take place.
    next: Duration,
}

impl StepResult {
    pub fn new(updated_channels: ChannelMask, next: Duration) -> Self {
        Self {
            updated_channels,
            next,
        }
    }
}
