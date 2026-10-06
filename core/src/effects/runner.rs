use crate::effects::{Effect, StepResult};
use core::time::Duration;
use heapless::Vec;

#[derive(Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EffectRunner<const N: usize> {
    effects: Vec<RunningEffect, N>,
}

impl<const N: usize> EffectRunner<N> {
    pub fn step(&mut self, now: Duration) -> StepResult {
        todo!()
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

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<RunningEffect>(), 64);
    }
}
