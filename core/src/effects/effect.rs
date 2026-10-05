use crate::api::ChannelMask;
use core::time::Duration;
use enum_dispatch::enum_dispatch;
use getset::Getters;
use serde::{Serialize, de::DeserializeOwned};

pub trait EffectCreate: Sized {
    type Config: Serialize + DeserializeOwned;

    /// Creates a new effect with the given configuration.
    fn new(config: Self::Config) -> Result<Self, &'static str>;
}

#[enum_dispatch]
pub trait EffectRun {
    /// Steps the effect forward, returning the next step result.
    fn step<const N: usize>(
        &mut self,
        now: core::time::Duration,
        led_data: &mut crate::pixel_data::PixelDataBuffer<N>,
    ) -> Result<crate::effects::StepResult, &'static str>;
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
