use crate::config::{ChannelSectionConfig, MAX_NAME_LEN, MAX_SECTIONS_PER_CHANNEL, PixelSpan};
use getset::Getters;
use heapless::{String, Vec};
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

    /// A section's start index plus its length overflows `u32`.
    #[error("section '{name}' start ({start}) + length ({len}) overflows address space")]
    SectionOverflow {
        name: String<MAX_NAME_LEN>,
        start: u32,
        len: u32,
    },

    /// There is a gap in the pixel address space.
    #[error(
        "gap in channel address space before section '{name}' (expected start: {expected}, actual: {actual})"
    )]
    Gap {
        name: String<MAX_NAME_LEN>,
        expected: u32,
        actual: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ChannelConfig {
    /// Friendly name for the channel
    name: String<{ MAX_NAME_LEN }>,

    /// Subsections of the channel
    sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }>,
}

impl ChannelConfig {
    #[must_use]
    pub fn new(
        name: String<{ MAX_NAME_LEN }>,
        sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }>,
    ) -> Self {
        Self { name, sections }
    }

    /// Checks if the channel configuration is valid.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Collect non-empty sections and check for overflow
        let mut active_sections: Vec<&ChannelSectionConfig, MAX_SECTIONS_PER_CHANNEL> = Vec::new();
        for section in &self.sections {
            let len = section.len();
            if len == 0 {
                continue;
            }
            let start = *section.start();
            if start.checked_add(len).is_none() {
                return Err(ConfigValidationError::SectionOverflow {
                    name: section.name().clone(),
                    start,
                    len,
                });
            }
            let _ = active_sections.push(section);
        }

        if active_sections.is_empty() {
            return Ok(());
        }

        // Sort sections by start index to detect gaps and overlaps
        active_sections.sort_unstable_by_key(|s| *s.start());

        let mut cursor = 0;
        let mut prev_name = active_sections[0].name().clone();

        for (i, section) in active_sections.iter().enumerate() {
            let start = *section.start();
            let len = section.len();
            let end = start + len;

            if i == 0 {
                if start != 0 {
                    return Err(ConfigValidationError::Gap {
                        name: section.name().clone(),
                        expected: 0,
                        actual: start,
                    });
                }
            } else if start > cursor {
                return Err(ConfigValidationError::Gap {
                    name: section.name().clone(),
                    expected: cursor,
                    actual: start,
                });
            } else if start < cursor {
                return Err(ConfigValidationError::OverlappingSections {
                    a: prev_name,
                    b: section.name().clone(),
                });
            }

            cursor = end;
            prev_name = section.name().clone();
        }

        Ok(())
    }

    /// Returns `true` if the channel configuration is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }
}

impl PixelSpan for ChannelConfig {
    fn len(&self) -> u32 {
        self.sections.iter().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PixelLayout;

    #[test]
    fn empty_channel_has_zero_length() {
        let channel = ChannelConfig::new("empty".try_into().unwrap(), Vec::new());
        assert_eq!(channel.validate(), Ok(()));
        assert!(channel.is_valid());
        assert_eq!(channel.len(), 0);
    }

    #[test]
    fn length_is_sum_of_section_lengths() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "onboard".try_into().unwrap(),
                0,
                PixelLayout::single_pixel(),
            ),
            ChannelSectionConfig::new("gap".try_into().unwrap(), 1, PixelLayout::linear_array(4)),
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
        .into();

        let channel = ChannelConfig::new("main".try_into().unwrap(), sections);
        assert_eq!(channel.validate(), Ok(()));
        assert!(channel.is_valid());
        assert_eq!(channel.len(), 25);
    }

    #[test]
    fn validate_accepts_adjacent_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "first".try_into().unwrap(),
                0,
                PixelLayout::linear_array(10),
            ),
            ChannelSectionConfig::new(
                "second".try_into().unwrap(),
                10,
                PixelLayout::linear_array(5),
            ),
            ChannelSectionConfig::new(
                "third".try_into().unwrap(),
                15,
                PixelLayout::linear_array(5),
            ),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(channel.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_gapped_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "first".try_into().unwrap(),
                0,
                PixelLayout::linear_array(10),
            ),
            ChannelSectionConfig::new(
                "second".try_into().unwrap(),
                10,
                PixelLayout::linear_array(5),
            ),
            ChannelSectionConfig::new(
                "third".try_into().unwrap(),
                20,
                PixelLayout::linear_array(5),
            ),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::Gap {
                name: "third".try_into().unwrap(),
                expected: 15,
                actual: 20,
            })
        );
    }

    #[test]
    fn validate_rejects_gap_at_start() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> =
            [ChannelSectionConfig::new(
                "first".try_into().unwrap(),
                2,
                PixelLayout::linear_array(10),
            )]
            .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::Gap {
                name: "first".try_into().unwrap(),
                expected: 0,
                actual: 2,
            })
        );
    }

    #[test]
    fn validate_accepts_out_of_order_non_overlapping_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "second".try_into().unwrap(),
                10,
                PixelLayout::linear_array(5),
            ),
            ChannelSectionConfig::new(
                "first".try_into().unwrap(),
                0,
                PixelLayout::linear_array(10),
            ),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(channel.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_overlapping_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "first".try_into().unwrap(),
                0,
                PixelLayout::linear_array(10),
            ),
            ChannelSectionConfig::new(
                "second".try_into().unwrap(),
                8,
                PixelLayout::linear_array(10),
            ),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::OverlappingSections {
                a: "first".try_into().unwrap(),
                b: "second".try_into().unwrap(),
            })
        );
        assert!(!channel.is_valid());
    }

    #[test]
    fn validate_rejects_contained_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new(
                "outer".try_into().unwrap(),
                0,
                PixelLayout::linear_array(20),
            ),
            ChannelSectionConfig::new("inner".try_into().unwrap(), 5, PixelLayout::linear_array(5)),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::OverlappingSections {
                a: "outer".try_into().unwrap(),
                b: "inner".try_into().unwrap(),
            })
        );
    }

    #[test]
    fn validate_rejects_identical_sections() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> = [
            ChannelSectionConfig::new("a".try_into().unwrap(), 0, PixelLayout::linear_array(10)),
            ChannelSectionConfig::new("b".try_into().unwrap(), 0, PixelLayout::linear_array(10)),
        ]
        .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::OverlappingSections {
                a: "a".try_into().unwrap(),
                b: "b".try_into().unwrap(),
            })
        );
    }

    #[test]
    fn validate_rejects_overflowing_section() {
        let sections: Vec<ChannelSectionConfig, { MAX_SECTIONS_PER_CHANNEL }> =
            [ChannelSectionConfig::new(
                "overflow".try_into().unwrap(),
                u32::MAX - 2,
                PixelLayout::linear_array(5),
            )]
            .into();

        let channel = ChannelConfig::new("test".try_into().unwrap(), sections);
        assert_eq!(
            channel.validate(),
            Err(ConfigValidationError::SectionOverflow {
                name: "overflow".try_into().unwrap(),
                start: u32::MAX - 2,
                len: 5,
            })
        );
    }
}
