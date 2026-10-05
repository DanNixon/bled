mod fixed_color;

pub use fixed_color::*;

use crate::effects::{EffectCreate, EffectRun};
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[enum_dispatch(EffectRun)]
pub enum EffectKind {
    FixedColor(FixedColor),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum EffectKindConfig {
    FixedColor(<FixedColor as EffectCreate>::Config),
}
