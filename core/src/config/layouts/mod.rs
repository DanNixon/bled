mod circular_array;
mod linear_array;

pub use circular_array::CircularArray;
pub use linear_array::LinearArray;

use serde::{Deserialize, Serialize};

/// The physical layout of a span of pixels on a single channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum PixelLayout {
    /// Pixels arranged in a line
    LinearArray(LinearArray),

    /// Pixels arranged in a circle
    CircularArray(CircularArray),
}
