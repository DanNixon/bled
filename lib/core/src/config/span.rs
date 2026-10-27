use crate::{ChannelNumber, NameString, PixelIndex};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct PixelSpan {
    /// Human-readable span name.
    name: NameString,

    /// The output channel the pixels are driven by
    channel: ChannelNumber,

    /// The index of the first pixel in the span.
    start: PixelIndex,

    /// The number of pixels in the span.
    length: PixelIndex,
}

impl PixelSpan {
    pub fn new(
        name: NameString,
        channel: ChannelNumber,
        start: PixelIndex,
        length: PixelIndex,
    ) -> Self {
        Self {
            name,
            channel,
            start,
            length,
        }
    }

    pub fn end(&self) -> PixelIndex {
        self.start + self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<PixelSpan>(), 40);
    }
}
