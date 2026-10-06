use crate::config::{ChannelSectionsConfig, MAX_NAME_LEN, PositionalPixelSpan, SizedPixelSpan};
use core::ops::Range;
use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigValidationError {
    /// Two sections on the same channel overlap in pixel address space.
    #[error("sections '{a}' and '{b}' overlap")]
    OverlappingSections {
        a: String<MAX_NAME_LEN>,
        b: String<MAX_NAME_LEN>,
    },

    /// A section's start index plus its length overflows `PixelIndex`.
    #[error("section '{name}' overflows address space")]
    SectionOverflow { name: String<MAX_NAME_LEN> },

    /// There is a gap in the pixel address space.
    #[error("gap in channel address space before section '{name}'")]
    Gap { name: String<MAX_NAME_LEN> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ChannelConfig {
    /// Friendly name for the channel
    name: String<{ MAX_NAME_LEN }>,

    /// Subsections of the channel
    sections: ChannelSectionsConfig,
}

impl ChannelConfig {
    #[must_use]
    pub fn new(name: String<{ MAX_NAME_LEN }>, sections: ChannelSectionsConfig) -> Self {
        Self { name, sections }
    }
}

impl SizedPixelSpan for ChannelConfig {
    fn len(&self) -> usize {
        self.sections.as_ref().iter().map(|s| s.len()).sum()
    }
}

impl PositionalPixelSpan for ChannelConfig {
    fn range(&self) -> Range<usize> {
        0..self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ChannelSectionConfig, PixelLayout};

    #[test]
    fn deserialize_channel_from_json() {
        let json = r#"{
            "name": "main",
            "sections": [
                {
                    "name": "onboard",
                    "start": 0,
                    "layout": { "single_pixel": {} }
                },
                {
                    "name": "strip",
                    "start": 1,
                    "layout": { "linear_array": { "length": 20 } }
                }
            ]
        }"#;

        let (channel, consumed) = serde_json_core::from_str::<ChannelConfig>(json).unwrap();
        let expected = ChannelConfig::new(
            "main".try_into().unwrap(),
            ChannelSectionsConfig::try_new(
                [
                    ChannelSectionConfig::new(
                        "onboard".try_into().unwrap(),
                        0,
                        PixelLayout::single_pixel(),
                    ),
                    ChannelSectionConfig::new(
                        "strip".try_into().unwrap(),
                        1,
                        PixelLayout::linear_array(20),
                    ),
                ]
                .into(),
            )
            .unwrap(),
        );

        assert_eq!(channel, expected);
        assert_eq!(consumed, json.len());
    }

    #[test]
    fn empty_channel_has_zero_length() {
        let channel = ChannelConfig::new(
            "empty".try_into().unwrap(),
            ChannelSectionsConfig::try_new([].into()).unwrap(),
        );
        assert_eq!(channel.len(), 0);
    }

    #[test]
    fn length_is_sum_of_section_lengths() {
        let sections = ChannelSectionsConfig::try_new(
            [
                ChannelSectionConfig::new(
                    "onboard".try_into().unwrap(),
                    0,
                    PixelLayout::single_pixel(),
                ),
                ChannelSectionConfig::new(
                    "gap".try_into().unwrap(),
                    1,
                    PixelLayout::linear_array(4),
                ),
                ChannelSectionConfig::new(
                    "strip".try_into().unwrap(),
                    5,
                    PixelLayout::linear_array(20),
                ),
                ChannelSectionConfig::new(
                    "unused".try_into().unwrap(),
                    25,
                    PixelLayout::linear_array(0),
                ),
            ]
            .into(),
        )
        .unwrap();

        let channel = ChannelConfig::new("main".try_into().unwrap(), sections);
        assert_eq!(channel.len(), 25);
    }
}
