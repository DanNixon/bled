use crate::{ChannelNumber, NameString, PixelIndex, config::BrightnessLimit};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct PixelSpan {
    /// Human-readable span name.
    name: NameString,

    /// The output channel the pixels are driven by.
    channel: ChannelNumber,

    /// The index of the first pixel in the span.
    start: PixelIndex,

    /// The number of pixels in the span.
    length: PixelIndex,

    /// The brightness limit for pixels in the span.
    #[serde(default, rename = "limit")]
    brightness_limit: BrightnessLimit,
}

impl PixelSpan {
    pub fn new(
        name: NameString,
        channel: ChannelNumber,
        start: PixelIndex,
        length: PixelIndex,
        brightness_limit: BrightnessLimit,
    ) -> Self {
        Self {
            name,
            channel,
            start,
            length,
            brightness_limit,
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

    #[test]
    fn deserializes_with_default_brightness_limit() {
        let span: PixelSpan =
            crate::io::decode_json(br#"{"name":"a","channel":0,"start":1,"length":2}"#).unwrap();
        assert_eq!(*span.brightness_limit(), BrightnessLimit::default());
    }

    #[test]
    fn deserializes_with_explicit_brightness_limit() {
        let span: PixelSpan = crate::io::decode_json(
            br#"{
                "name":"b",
                "channel":1,
                "start":0,
                "length":4,
                "limit":{
                    "mode":"rescale",
                    "limit":{"r":64,"g":128,"b":255}
                }
            }"#,
        )
        .unwrap();
        assert_eq!(
            *span.brightness_limit(),
            BrightnessLimit::new(
                crate::config::BrightnessLimitMode::Rescale,
                crate::RGB8::new(64, 128, 255)
            )
        );
    }
}
