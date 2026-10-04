mod linear_array;
mod single_pixel;

use serde::{Deserialize, Serialize};

pub use linear_array::LinearArray;
pub use single_pixel::SinglePixel;

use crate::config::ChannelSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelSectionLayout {
    /// A single pixel
    SinglePixel(SinglePixel),

    /// A linear channel of pixels
    LinearArray(LinearArray),
}

impl ChannelSectionLayout {
    #[must_use]
    pub fn single_pixel() -> Self {
        Self::SinglePixel(SinglePixel::default())
    }

    #[must_use]
    pub fn linear_array(len: u32) -> Self {
        Self::LinearArray(LinearArray::new(len))
    }
}

impl ChannelSpan for ChannelSectionLayout {
    fn len(&self) -> u32 {
        match self {
            Self::SinglePixel(m) => m.len(),
            Self::LinearArray(m) => m.len(),
        }
    }
}
