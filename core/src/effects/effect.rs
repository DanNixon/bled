use crate::{
    config::MAX_NAME_LEN,
    effects::{EffectKind, Target},
};
use enum_dispatch::enum_dispatch;
use getset::Getters;
use heapless::String;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Effect {
    /// Name of the effect
    name: String<{ MAX_NAME_LEN }>,

    /// Effect logic
    effect: EffectKind,
}

pub trait EffectCreate: Sized {
    type Config: Serialize + DeserializeOwned;

    /// Creates a new effect with the given configuration.
    fn new(target: Target, config: Self::Config) -> Result<Self, &'static str>;
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
