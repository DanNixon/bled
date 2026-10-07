use crate::{
    api::MAX_PATH_LEN,
    effects::{EffectKind, Target},
};
use enum_dispatch::enum_dispatch;
use getset::{Getters, MutGetters};
use heapless::String;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug, Getters, MutGetters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub", get_mut = "pub")]
pub struct Effect {
    /// File the effect was loaded from
    filename: String<{ MAX_PATH_LEN }>,

    /// Effect logic
    kind: EffectKind,
}

impl Effect {
    pub fn new(filename: String<{ MAX_PATH_LEN }>, kind: EffectKind) -> Result<Self, &'static str> {
        Ok(Self { filename, kind })
    }
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
