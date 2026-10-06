use crate::{
    api::PixelIndex,
    config::{
        MAX_NAME_LEN, MAX_SECTIONS_PER_CHANNEL, PixelLayout, PositionalPixelSpan, SizedPixelSpan,
    },
};
use core::ops::Range;
use getset::Getters;
use heapless::{String, Vec};
use nutype::nutype;
use serde::{Deserialize, Serialize};

/// Configuration for an LED output channel, i.e. a physical channel of LEDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ChannelSectionConfig {
    /// Friendly name for the section
    name: String<{ MAX_NAME_LEN }>,

    /// Start index of the section
    start: PixelIndex,

    /// Layout of the section
    layout: PixelLayout,
}

impl ChannelSectionConfig {
    pub fn new(name: String<{ MAX_NAME_LEN }>, start: PixelIndex, layout: PixelLayout) -> Self {
        Self {
            name,
            start,
            layout,
        }
    }
}

#[nutype(
    validate(predicate = validate_channel_sections),
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct ChannelSectionsConfig(Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }>);

fn validate_channel_sections(
    sections: &Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }>,
) -> bool {
    let mut cursor = 0;

    for section in sections {
        let start = section.range().start;
        let end = section.range().end;

        if start != cursor {
            return false;
        }
        cursor = end;
    }

    if cursor > PixelIndex::MAX as usize {
        return false;
    }

    true
}

impl SizedPixelSpan for ChannelSectionConfig {
    fn len(&self) -> usize {
        self.layout.len()
    }
}

impl PositionalPixelSpan for ChannelSectionConfig {
    fn range(&self) -> Range<usize> {
        self.start as usize..self.start as usize + self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_types_have_known_sizes() {
        assert_eq!(core::mem::size_of::<ChannelSectionConfig>(), 40);
    }

    fn section(start: PixelIndex, length: PixelIndex) -> ChannelSectionConfig {
        ChannelSectionConfig::new(
            "section".try_into().unwrap(),
            start,
            PixelLayout::linear_array(length),
        )
    }

    #[test]
    fn empty_sections_are_valid() {
        assert!(ChannelSectionsConfig::try_new([].into()).is_ok());
    }

    #[test]
    fn contiguous_sections_are_valid() {
        let sections = [section(0, 2), section(2, 3), section(5, 1)].into();
        assert!(ChannelSectionsConfig::try_new(sections).is_ok());
    }

    #[test]
    fn gaps_between_sections_are_invalid() {
        let sections = [section(0, 2), section(3, 1)].into();
        assert!(ChannelSectionsConfig::try_new(sections).is_err());
    }

    #[test]
    fn overlapping_sections_are_invalid() {
        let sections = [section(0, 2), section(1, 1)].into();
        assert!(ChannelSectionsConfig::try_new(sections).is_err());
    }

    #[test]
    fn section_ending_at_pixelindex_max_is_valid() {
        let sections = [
            section(0, PixelIndex::MAX - 1),
            section(PixelIndex::MAX - 1, 1),
        ]
        .into();
        assert!(ChannelSectionsConfig::try_new(sections).is_ok());
    }

    #[test]
    fn section_ending_after_pixelindex_max_is_invalid() {
        let sections = [section(0, PixelIndex::MAX), section(PixelIndex::MAX, 1)].into();
        assert!(ChannelSectionsConfig::try_new(sections).is_err());
    }

    #[test]
    fn invalid_sections_are_rejected_when_decoded_from_json() {
        let json = br#"[
            {
                "name": "first",
                "start": 0,
                "layout": { "linear_array": { "length": 2 } }
            },
            {
                "name": "second-with-gap",
                "start": 3,
                "layout": { "single_pixel": {} }
            }
        ]"#;
        assert!(crate::io::decode_json::<ChannelSectionsConfig>(json).is_err());
    }
}
