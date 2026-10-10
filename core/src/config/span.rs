use crate::api::{ChannelNumber, PixelIndex};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Span {
    /// The output channel the pixels are driven by
    channel: ChannelNumber,

    /// The index of the first pixel in the span.
    start: PixelIndex,

    /// The number of pixels in the span.
    length: PixelIndex,
}
