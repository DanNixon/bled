use crate::{
    api::ChannelMask,
    config::PositionalPixelSpan,
    effects::{EffectCreate, EffectRun, StepResult, Target},
    pixel_data::PixelDataBuffer,
};
use core::time::Duration;
use getset::Getters;
use rgb::RGB8;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FixedColor {
    target: Target,
    color: RGB8,
}

impl FixedColorConfig {
    #[must_use]
    pub const fn new(color: RGB8) -> Self {
        Self { color }
    }
}

impl EffectCreate for FixedColor {
    type Config = FixedColorConfig;

    fn new(target: Target, config: Self::Config) -> Result<Self, &'static str> {
        Ok(Self {
            target,
            color: config.color,
        })
    }
}

impl EffectRun for FixedColor {
    fn step<const N: usize>(
        &mut self,
        now: Duration,
        led_data: &mut PixelDataBuffer<N>,
    ) -> Result<crate::effects::StepResult, &'static str> {
        let mask = ChannelMask::from_index(self.target.channel().into_inner() as usize).unwrap();

        if let Ok(channel) = led_data.try_channel_mut::<RGB8>(*self.target.channel()) {
            let s = self.target.range();
            channel[s.start as usize..s.end as usize].fill(self.color);
        }

        Ok(StepResult::new(mask, now + Duration::from_secs(1)))
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct FixedColorConfig {
    color: RGB8,
}
