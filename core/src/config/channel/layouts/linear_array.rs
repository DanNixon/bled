use crate::{api::PixelIndex, config::SizedPixelSpan};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct LinearArray {
    length: PixelIndex,
}

impl LinearArray {
    #[must_use]
    pub fn new(length: PixelIndex) -> Self {
        Self { length }
    }
}

impl SizedPixelSpan for LinearArray {
    fn len(&self) -> usize {
        self.length as usize
    }
}
