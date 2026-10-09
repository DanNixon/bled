mod linear_array;

pub use linear_array::LinearArray;

use serde::{Deserialize, Serialize};

/// The physical layout of a span of pixels on a single channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum PixelLayout {
    /// A linear channel of pixels
    LinearArray(LinearArray),
}

impl PixelLayout {
    #[must_use]
    pub fn linear_array() -> Self {
        Self::LinearArray(LinearArray::default())
    }
}
