use crate::{ChannelNumber, PixelIndex};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct PixelSpan {
    /// The output channel the pixels are driven by
    channel: ChannelNumber,

    /// The index of the first pixel in the span.
    start: PixelIndex,

    /// The number of pixels in the span.
    length: PixelIndex,
}

impl PixelSpan {
    pub fn new(channel: ChannelNumber, start: PixelIndex, length: PixelIndex) -> Self {
        Self {
            channel,
            start,
            length,
        }
    }
}
