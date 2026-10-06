use crate::config::SizedPixelSpan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SinglePixel {}

impl SizedPixelSpan for SinglePixel {
    fn len(&self) -> usize {
        1
    }
}
