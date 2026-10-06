use crate::api::ChannelMask;
use core::time::Duration;
use getset::Getters;

/// The result of a single step of an effect or the effect runner.
#[derive(Debug, PartialEq, Eq, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct StepResult {
    /// Channels that need to be rendered.
    render: ChannelMask,

    /// The time that the next step should take place.
    next: Duration,
}

impl StepResult {
    pub fn new(render: ChannelMask, next: Duration) -> Self {
        Self { render, next }
    }
}
