mod linear_array;
mod single_pixel;

pub use linear_array::LinearArray;
pub use single_pixel::SinglePixel;

use crate::{api::PixelIndex, config::SizedPixelSpan};
use serde::{Deserialize, Serialize};

/// The physical layout of a span of pixels on a single channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum PixelLayout {
    /// A single pixel
    SinglePixel(SinglePixel),

    /// A linear channel of pixels
    LinearArray(LinearArray),
}

impl PixelLayout {
    #[must_use]
    pub fn single_pixel() -> Self {
        Self::SinglePixel(SinglePixel::default())
    }

    #[must_use]
    pub fn linear_array(len: PixelIndex) -> Self {
        Self::LinearArray(LinearArray::new(len))
    }
}

impl SizedPixelSpan for PixelLayout {
    fn len(&self) -> usize {
        match self {
            Self::SinglePixel(m) => m.len(),
            Self::LinearArray(m) => m.len(),
        }
    }
}
