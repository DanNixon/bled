use crate::{
    api::ChannelMask,
    effects::{Effect, EffectRun, StepResult},
    pixel_data::PixelDataBuffer,
};
use core::time::Duration;
use heapless::Vec;

#[derive(Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EffectRunner<const N: usize> {
    effects: Vec<RunningEffect, N>,
}

impl<const N: usize> EffectRunner<N> {
    pub fn step<const P: usize>(
        &mut self,
        now: Duration,
        pixel_data: &mut PixelDataBuffer<P>,
    ) -> StepResult {
        let mut updated_channels = ChannelMask::default();
        let mut earliest = Duration::MAX;

        for effect in &mut self.effects {
            if effect.next_step <= now {
                match effect.effect.kind_mut().step(now, pixel_data) {
                    Ok(step) => {
                        updated_channels |= *step.updated_channels();
                        effect.next_step = *step.next();
                    }
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        defmt::warn!("Effect step failed: {}", e);

                        #[cfg(not(feature = "defmt"))]
                        let _ = e;
                    }
                }
            }

            earliest = core::cmp::min(earliest, effect.next_step)
        }

        StepResult::new(updated_channels, earliest)
    }

    pub fn add_effect(&mut self, effect: Effect) -> Result<(), Effect> {
        self.effects
            .push(RunningEffect {
                effect,
                next_step: Duration::from_millis(0),
            })
            .map_err(|e| e.effect)
    }

    pub fn remove_effect(&mut self, name: &str) -> Option<Effect> {
        let index = self.effects.iter().position(|e| e.effect.name() == name)?;
        Some(self.effects.remove(index).effect)
    }
}

#[derive(Debug)]
struct RunningEffect {
    /// The effect being run
    effect: Effect,

    /// The next time the effect should be run
    next_step: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{ChannelMask, ChannelNumber},
        config::PixelLayout,
        effects::{EffectCreate, EffectKind, FixedColor, FixedColorConfig, Target},
        pixel_data::PixelDataChannelSize,
    };
    use rgb::RGB8;

    fn effect(name: &str, channel: u8, color: RGB8) -> Effect {
        let target = Target::new(
            ChannelNumber::try_new(channel).unwrap(),
            0,
            PixelLayout::linear_array(2),
        );
        let kind =
            EffectKind::FixedColor(FixedColor::new(target, FixedColorConfig::new(color)).unwrap());
        Effect::new(name, kind).unwrap()
    }

    fn pixel_data() -> PixelDataBuffer<12> {
        let mut pixels = PixelDataBuffer::new();
        pixels
            .try_reshape(&[
                PixelDataChannelSize::new::<RGB8>(2),
                PixelDataChannelSize::new::<RGB8>(2),
            ])
            .unwrap();
        pixels
    }

    #[test]
    fn steps_effects_at_or_after_their_deadline() {
        let mut runner = EffectRunner::<1>::default();
        runner
            .add_effect(effect("red", 0, RGB8::new(255, 0, 0)))
            .unwrap();
        let mut pixels = pixel_data();

        let first = runner.step(Duration::from_secs(0), &mut pixels);
        assert_eq!(
            *first.updated_channels(),
            ChannelMask::from_index(0).unwrap()
        );
        assert_eq!(*first.next(), Duration::from_secs(1));

        let before_deadline = runner.step(Duration::from_millis(500), &mut pixels);
        assert_eq!(*before_deadline.updated_channels(), ChannelMask::empty());
        assert_eq!(*before_deadline.next(), Duration::from_secs(1));

        let at_deadline = runner.step(Duration::from_secs(1), &mut pixels);
        assert_eq!(
            *at_deadline.updated_channels(),
            ChannelMask::from_index(0).unwrap()
        );
        assert_eq!(*at_deadline.next(), Duration::from_secs(2));
        assert_eq!(
            pixels
                .try_channel::<RGB8>(ChannelNumber::try_new(0).unwrap())
                .unwrap()[0],
            RGB8::new(255, 0, 0)
        );
    }

    #[test]
    fn combines_updated_channels_and_returns_earliest_deadline() {
        let mut runner = EffectRunner::<2>::default();
        runner
            .add_effect(effect("red", 0, RGB8::new(255, 0, 0)))
            .unwrap();
        runner
            .add_effect(effect("green", 1, RGB8::new(0, 255, 0)))
            .unwrap();

        let result = runner.step(Duration::ZERO, &mut pixel_data());
        assert_eq!(
            *result.updated_channels(),
            ChannelMask::from_indices([0, 1]).unwrap()
        );
        assert_eq!(*result.next(), Duration::from_secs(1));
    }

    #[test]
    fn removes_effects_by_name_and_rejects_capacity_overflow() {
        let mut runner = EffectRunner::<1>::default();
        runner
            .add_effect(effect("first", 0, RGB8::new(1, 2, 3)))
            .unwrap();
        let rejected = runner
            .add_effect(effect("second", 1, RGB8::new(4, 5, 6)))
            .unwrap_err();
        assert_eq!(rejected.name(), "second");

        assert!(runner.remove_effect("missing").is_none());
        assert_eq!(runner.remove_effect("first").unwrap().name(), "first");
        assert!(runner.remove_effect("first").is_none());
    }

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<RunningEffect>(), 64);
        assert_eq!(core::mem::size_of::<EffectRunner<16>>(), 1032);
    }
}
