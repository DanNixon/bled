use crate::{
    NameString, PixelIndex,
    config::{PixelLayout, PixelSpan},
};
use core::ops::Range;
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of fixtures that may be configured on a single device.
pub const MAX_FIXTURE_COUNT: usize = 8;

/// A collection of fixtures configured on a single device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Fixtures(Vec<Fixture, { MAX_FIXTURE_COUNT }>);

impl Fixtures {
    pub fn new(vec_inner: Vec<Fixture, { MAX_FIXTURE_COUNT }>) -> Self {
        Self(vec_inner)
    }

    pub fn as_slice(&self) -> &[Fixture] {
        &self.0
    }

    /// Get a fixture by name.
    pub fn find_fixture(&self, name: &str) -> Option<&Fixture> {
        self.0.iter().find(|f| f.name() == name)
    }

    /// Gets the range of pixels covered by a span with reference to the entire device.
    /// i.e. where in the pixel-wise buffer the span starts and ends.
    pub fn span_pixel_range<const OUTPUT_CHANNEL_COUNT: usize>(
        &self,
        span: &PixelSpan,
    ) -> Option<Range<PixelIndex>> {
        let channel_lengths = self.channel_lengths::<OUTPUT_CHANNEL_COUNT>()?;

        // Find the end of the previous channel to the one that the span is in
        let span_channel = span.channel().into_inner() as usize;
        let end_of_previous_channel: PixelIndex = channel_lengths[..span_channel]
            .iter()
            .map(|i| *i as PixelIndex)
            .sum();

        let start = end_of_previous_channel + span.start();
        let end = start + span.length();

        Some(start..end)
    }

    /// Gets the required length of output channels.
    pub fn channel_lengths<const OUTPUT_CHANNEL_COUNT: usize>(
        &self,
    ) -> Option<[usize; OUTPUT_CHANNEL_COUNT]> {
        let mut lengths = [0; OUTPUT_CHANNEL_COUNT];

        for fixture in &self.0 {
            for span in fixture.spans() {
                let channel = usize::from(*span.channel().as_ref());
                let end = (*span.start() as usize).saturating_add(*span.length() as usize);
                match lengths.get_mut(channel) {
                    Some(l) => {
                        *l = (*l).max(end);
                    }
                    None => {
                        return None;
                    }
                }
            }
        }

        Some(lengths)
    }

    /// Gets the required length of all output channels (e.g. for calculating required buffer size)
    pub fn total_length<const OUTPUT_CHANNEL_COUNT: usize>(&self) -> usize {
        let channel_lengths = self.channel_lengths::<OUTPUT_CHANNEL_COUNT>();
        channel_lengths.map_or(0, |l| l.into_iter().sum())
    }

    /// Validates the device config.
    pub fn validate<const OUTPUT_CHANNEL_COUNT: usize>(
        &self,
    ) -> Result<(), FixturesValidationError> {
        for fixture in &self.0 {
            for span in fixture.spans() {
                let channel = *span.channel().as_ref();
                if usize::from(channel) >= OUTPUT_CHANNEL_COUNT {
                    return Err(FixturesValidationError::InvalidOutputChannel {
                        channel,
                        output_channel_count: OUTPUT_CHANNEL_COUNT,
                    });
                }

                span.start()
                    .checked_add(*span.length())
                    .ok_or(FixturesValidationError::SpanEndOverflow)?;
            }
        }

        // Check for overlapping spans.
        let overlaps =
            |first: &PixelSpan, second: &PixelSpan| -> Result<bool, FixturesValidationError> {
                if first.channel() != second.channel() {
                    return Ok(false);
                }

                let first_end = first
                    .start()
                    .checked_add(*first.length())
                    .ok_or(FixturesValidationError::SpanEndOverflow)?;
                let second_end = second
                    .start()
                    .checked_add(*second.length())
                    .ok_or(FixturesValidationError::SpanEndOverflow)?;

                Ok(*first.start() < second_end && *second.start() < first_end)
            };

        for (fixture_index, fixture) in self.0.iter().enumerate() {
            for (span_index, span) in fixture.spans().iter().enumerate() {
                // Compare against earlier spans in the same fixture.
                for previous in fixture.spans().iter().take(span_index) {
                    if overlaps(span, previous)? {
                        return Err(FixturesValidationError::OverlappingSpans);
                    }
                }

                // Compare against spans in earlier fixtures.
                for previous_fixture in self.0.iter().take(fixture_index) {
                    for previous in previous_fixture.spans() {
                        if overlaps(span, previous)? {
                            return Err(FixturesValidationError::OverlappingSpans);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Error returned when a device configuration is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FixturesValidationError {
    /// A span references an output channel that is not available.
    #[error("output channel {channel} is invalid (count: {output_channel_count})")]
    InvalidOutputChannel {
        channel: u8,
        output_channel_count: usize,
    },

    /// A span's end index cannot be represented.
    #[error("span end index overflows")]
    SpanEndOverflow,

    /// Two spans on the same output channel overlap.
    #[error("spans overlap")]
    OverlappingSpans,
}
/// Maximum number of spans that may be used in a single fixture.
pub const MAX_SPAN_PER_FIXTURE_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Fixture {
    /// Human-readable fixture name.
    name: NameString,

    /// Layout of the pixels in this fixture.
    layout: PixelLayout,

    /// Physical pixel spans that make up this fixture.
    spans: Vec<PixelSpan, { MAX_SPAN_PER_FIXTURE_COUNT }>,
}

impl Fixture {
    pub fn new(
        name: NameString,
        layout: PixelLayout,
        spans: Vec<PixelSpan, { MAX_SPAN_PER_FIXTURE_COUNT }>,
    ) -> Self {
        Self {
            name,
            layout,
            spans,
        }
    }

    /// Get a span by name.
    pub fn find_span(&self, name: &str) -> Option<&PixelSpan> {
        self.spans.iter().find(|s| s.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<Fixture>(), 360);
        assert_eq!(core::mem::size_of::<Fixtures>(), 2888);
    }

    #[test]
    fn empty_configuration_has_zero_length_channels() {
        let fixtures: Fixtures = crate::io::decode_json(br#"[]"#).unwrap();
        fixtures.validate::<4>().unwrap();

        assert_eq!(fixtures.channel_lengths::<4>().unwrap().as_slice(), &[0; 4]);
    }

    #[test]
    fn channel_lengths_cover_all_spans_on_each_channel() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {"name":"first","layout":{"linear_array":{}},"spans":[
                      {"name":"first","channel":0,"start":2,"length":3},
                      {"name":"second","channel":1,"start":4,"length":2}
                  ]},
                  {"name":"second","layout":{"linear_array":{}},"spans":[
                      {"name":"first","channel":0,"start":0,"length":1},
                      {"name":"second","channel":1,"start":1,"length":3},
                      {"name":"third","channel":3,"start":5,"length":2}
                  ]}
                ]"#,
        )
        .unwrap();
        fixtures.validate::<4>().unwrap();

        assert_eq!(
            fixtures.channel_lengths::<4>().unwrap().as_slice(),
            &[5, 6, 0, 7]
        );
        assert_eq!(fixtures.total_length::<4>(), 18);
    }

    #[test]
    fn span_pixel_range_on_first_channel_uses_span_coordinates() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":3,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();
        fixtures.validate::<4>().unwrap();

        assert_eq!(
            fixtures.span_pixel_range::<4>(&fixtures.as_slice()[0].spans()[0]),
            Some(3..5)
        );
    }

    #[test]
    fn span_pixel_range_offsets_by_maximum_end_of_previous_channels() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"first",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":2,"length":3},
                      {"name":"second","channel":1,"start":4,"length":2}
                    ]
                  },
                  {
                    "name":"second",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":0,"length":1},
                      {"name":"second","channel":1,"start":1,"length":3},
                      {"name":"third","channel":3,"start":5,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();
        fixtures.validate::<4>().unwrap();

        assert_eq!(
            fixtures.span_pixel_range::<4>(&fixtures.as_slice()[0].spans()[1]),
            Some(9..11)
        );
        assert_eq!(
            fixtures.span_pixel_range::<4>(&fixtures.as_slice()[1].spans()[2]),
            Some(16..18)
        );
    }

    #[test]
    fn empty_configuration_is_valid() {
        assert_eq!(
            crate::io::decode_json::<Fixtures>(br#"[]"#)
                .unwrap()
                .validate::<4>(),
            Ok(())
        );
    }

    #[test]
    fn non_overlapping_spans_on_the_same_channel_are_valid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"first",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":2,"length":3},
                      {"name":"second","channel":1,"start":4,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(fixtures.validate::<4>(), Ok(()));
    }

    #[test]
    fn overlapping_spans_in_the_same_fixture_are_invalid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":0,"length":3},
                      {"name":"second","channel":0,"start":2,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(
            fixtures.validate::<4>(),
            Err(FixturesValidationError::OverlappingSpans)
        );
    }

    #[test]
    fn overlapping_spans_in_different_fixtures_are_invalid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"first",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":4,"length":3}
                    ]
                  },
                  {
                    "name":"second",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"second","channel":0,"start":6,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(
            fixtures.validate::<4>(),
            Err(FixturesValidationError::OverlappingSpans)
        );
    }

    #[test]
    fn spans_on_different_channels_do_not_overlap() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":0,"length":4},
                      {"name":"second","channel":1,"start":0,"length":4}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(fixtures.validate::<4>(), Ok(()));
    }

    #[test]
    fn arithmetic_overflow_is_invalid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":0,"length":1},
                      {"name":"second","channel":0,"start":4294967295,"length":1}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(
            fixtures.validate::<4>(),
            Err(FixturesValidationError::SpanEndOverflow)
        );
    }

    #[test]
    fn span_on_unavailable_output_channel_is_invalid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":4,"start":0,"length":1}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(
            fixtures.validate::<4>(),
            Err(FixturesValidationError::InvalidOutputChannel {
                channel: 4,
                output_channel_count: 4,
            })
        );
    }

    #[test]
    fn single_span_with_overflowing_end_is_invalid() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"fixture",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":4294967295,"length":1}
                    ]
                  }
                ]"#,
        )
        .unwrap();

        assert_eq!(
            fixtures.validate::<4>(),
            Err(FixturesValidationError::SpanEndOverflow)
        );
    }

    #[test]
    fn find_fixtures_and_spans_by_name() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                  {
                    "name":"first",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":2,"length":3},
                      {"name":"second","channel":1,"start":4,"length":2}
                    ]
                  },
                  {
                    "name":"second",
                    "layout":{"linear_array":{}},
                    "spans":[
                      {"name":"first","channel":0,"start":0,"length":1},
                      {"name":"second","channel":1,"start":1,"length":3},
                      {"name":"third","channel":3,"start":5,"length":2}
                    ]
                  }
                ]"#,
        )
        .unwrap();
        fixtures.validate::<4>().unwrap();

        assert_eq!(
            *fixtures.find_fixture("first").unwrap(),
            fixtures.as_slice()[0]
        );
        assert!(fixtures.find_fixture("third").is_none());

        assert_eq!(
            *fixtures
                .find_fixture("second")
                .unwrap()
                .find_span("third")
                .unwrap(),
            fixtures.as_slice()[1].spans()[2]
        );
    }
}
