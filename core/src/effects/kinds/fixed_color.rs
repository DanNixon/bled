use crate::effects::{EffectCreate, EffectRun};
use core::time::Duration;
use getset::Getters;
use rgb::RGB8;
use serde::{Deserialize, Serialize};

pub struct FixedColor {}

impl EffectCreate for FixedColor {
    type Config = FixedColorConfig;

    fn new(config: Self::Config) -> Result<Self, &'static str> {
        todo!()
    }
}

impl EffectRun for FixedColor {
    fn step(&mut self, _now: Duration) -> Result<crate::effects::StepResult, &'static str> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct FixedColorConfig {
    color: RGB8,
}
