use crate::{
    api::{ChannelMask, MAX_PATH_LEN},
    effects::{Effect, EffectRun, StepResult},
    pixel_data::PixelDataBuffer,
};
use core::time::Duration;
use heapless::String;
use nutype::nutype;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EffectRunner<const N: usize> {
    effects: [Option<RunningEffect>; N],
}

impl<const N: usize> Default for EffectRunner<N> {
    fn default() -> Self {
        Self {
            effects: [const { None }; N],
        }
    }
}

impl<const N: usize> EffectRunner<N> {
    pub fn capacity(&self) -> usize {
        N
    }

    pub fn used(&self) -> usize {
        self.effects.iter().filter(|e| e.is_some()).count()
    }

    pub fn step<const P: usize>(
        &mut self,
        now: Duration,
        pixel_data: &mut PixelDataBuffer<P>,
    ) -> StepResult {
        let mut updated_channels = ChannelMask::default();
        let mut earliest = Duration::MAX;

        for effect in self.effects.iter_mut().flatten() {
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

    pub fn add_effect(&mut self, slot: EffectSlot, effect: Effect) -> Result<(), Effect> {
        let slot = slot.into_inner() as usize;

        if slot > self.effects.len() {
            return Err(effect);
        }

        let slot = self.effects.get_mut(slot).unwrap();

        *slot = Some(RunningEffect {
            effect,
            next_step: Duration::from_millis(0),
        });

        Ok(())
    }

    pub fn remove_effect(&mut self, slot: EffectSlot) -> Option<Effect> {
        let slot = slot.into_inner() as usize;

        if slot > self.effects.len() {
            return None;
        }

        let slot = self.effects.get_mut(slot).unwrap();

        slot.take().map(|effect| effect.effect)
    }
}

#[derive(Debug)]
struct RunningEffect {
    /// The effect being run
    effect: Effect,

    /// The next time the effect should be run
    next_step: Duration,
}

#[nutype(
    derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct EffectSlot(u8);

#[nutype(
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct EffectSlotContents(Option<String<{ MAX_PATH_LEN }>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<RunningEffect>(), 112);
        assert_eq!(core::mem::size_of::<EffectRunner<24>>(), 2688);
        assert_eq!(core::mem::size_of::<EffectSlotContents>(), 80);
    }
}
