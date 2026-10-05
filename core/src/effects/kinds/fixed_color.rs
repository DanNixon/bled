use crate::{
    api::ChannelMask,
    effects::{EffectCreate, EffectRun, StepResult, Targets},
    pixel_data::PixelDataBuffer,
};
use core::time::Duration;
use getset::Getters;
use rgb::RGB8;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FixedColor {
    targets: Targets,
    color: RGB8,
}

impl EffectCreate for FixedColor {
    type Config = FixedColorConfig;

    fn new(targets: Targets, config: Self::Config) -> Result<Self, &'static str> {
        Ok(Self {
            targets,
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
        let channels = ChannelMask::empty();

        for t in self.targets.as_ref() {
            if let Ok(channel) = led_data.try_channel_mut::<RGB8>(*t.channel().as_ref() as usize) {
                // TODO
            }
        }

        Ok(StepResult::new(channels, now + Duration::from_secs(1)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct FixedColorConfig {
    color: RGB8,
}
