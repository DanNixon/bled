use crate::{
    NameString, PathString, PixelIndex,
    config::{Fixture, MAX_FIXTURE_COUNT, PixelSpan},
};
use core::ops::Range;
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: NameString,

    /// Light fixtures controlled by this device.
    pub fixtures: Vec<Fixture, { MAX_FIXTURE_COUNT }>,

    /// Filename of the WASM binary or JSON preset for the effect to load on boot.
    pub default_effect: Option<PathString>,
}

impl DeviceConfig {
    /// Gets the required length of output channels.
    pub fn channel_lengths<const OUTPUT_CHANNEL_COUNT: usize>(
        &self,
    ) -> Option<[usize; OUTPUT_CHANNEL_COUNT]> {
        let mut lengths = [0; OUTPUT_CHANNEL_COUNT];

        for fixture in &self.fixtures {
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

    /// Validates the device config.
    pub fn validate<const OUTPUT_CHANNEL_COUNT: usize>(
        &self,
    ) -> Result<(), DeviceConfigValidationError> {
        for fixture in &self.fixtures {
            for span in fixture.spans() {
                let channel = *span.channel().as_ref();
                if usize::from(channel) >= OUTPUT_CHANNEL_COUNT {
                    return Err(DeviceConfigValidationError::InvalidOutputChannel {
                        channel,
                        output_channel_count: OUTPUT_CHANNEL_COUNT,
                    });
                }

                span.start()
                    .checked_add(*span.length())
                    .ok_or(DeviceConfigValidationError::SpanEndOverflow)?;
            }
        }

        // Check for overlapping spans.
        let overlaps =
            |first: &PixelSpan, second: &PixelSpan| -> Result<bool, DeviceConfigValidationError> {
                if first.channel() != second.channel() {
                    return Ok(false);
                }

                let first_end = first
                    .start()
                    .checked_add(*first.length())
                    .ok_or(DeviceConfigValidationError::SpanEndOverflow)?;
                let second_end = second
                    .start()
                    .checked_add(*second.length())
                    .ok_or(DeviceConfigValidationError::SpanEndOverflow)?;

                Ok(*first.start() < second_end && *second.start() < first_end)
            };

        for (fixture_index, fixture) in self.fixtures.iter().enumerate() {
            for (span_index, span) in fixture.spans().iter().enumerate() {
                // Compare against earlier spans in the same fixture.
                for previous in fixture.spans().iter().take(span_index) {
                    if overlaps(span, previous)? {
                        return Err(DeviceConfigValidationError::OverlappingSpans);
                    }
                }

                // Compare against spans in earlier fixtures.
                for previous_fixture in self.fixtures.iter().take(fixture_index) {
                    for previous in previous_fixture.spans() {
                        if overlaps(span, previous)? {
                            return Err(DeviceConfigValidationError::OverlappingSpans);
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
pub enum DeviceConfigValidationError {
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

#[cfg(test)]
mod tests {
    use super::{DeviceConfig, DeviceConfigValidationError};

    fn config(json: &str) -> DeviceConfig {
        serde_json_core::from_slice(json.as_bytes()).unwrap().0
    }

    #[test]
    fn empty_configuration_has_zero_length_channels() {
        let device = config(r#"{"name":"test","fixtures":[]}"#);
        device.validate::<4>().unwrap();

        assert_eq!(device.channel_lengths::<4>().unwrap().as_slice(), &[0; 4]);
    }

    #[test]
    fn channel_lengths_cover_all_spans_on_each_channel() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[
                    {"name":"first","layout":{"linear_array":{}},"spans":[
                        {"channel":0,"start":2,"length":3},
                        {"channel":1,"start":4,"length":2}
                    ]},
                    {"name":"second","layout":{"linear_array":{}},"spans":[
                        {"channel":0,"start":0,"length":1},
                        {"channel":1,"start":1,"length":3},
                        {"channel":3,"start":5,"length":2}
                    ]}
                ]
            }"#,
        );
        device.validate::<4>().unwrap();

        assert_eq!(
            device.channel_lengths::<4>().unwrap().as_slice(),
            &[5, 6, 0, 7]
        );
        assert_eq!(device.total_length::<4>(), 18);
    }

    #[test]
    fn span_pixel_range_on_first_channel_uses_span_coordinates() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":3,"length":2}
                ]}]
            }"#,
        );
        device.validate::<4>().unwrap();

        assert_eq!(
            device.span_pixel_range::<4>(&device.fixtures[0].spans()[0]),
            Some(3..5)
        );
    }

    #[test]
    fn span_pixel_range_offsets_by_maximum_end_of_previous_channels() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[
                    {"name":"first","layout":{"linear_array":{}},"spans":[
                        {"channel":0,"start":2,"length":3},
                        {"channel":1,"start":4,"length":2}
                    ]},
                    {"name":"second","layout":{"linear_array":{}},"spans":[
                        {"channel":0,"start":0,"length":1},
                        {"channel":1,"start":1,"length":3},
                        {"channel":3,"start":5,"length":2}
                    ]}
                ]
            }"#,
        );
        device.validate::<4>().unwrap();

        assert_eq!(
            device.span_pixel_range::<4>(&device.fixtures[0].spans()[1]),
            Some(9..11)
        );
        assert_eq!(
            device.span_pixel_range::<4>(&device.fixtures[1].spans()[2]),
            Some(16..18)
        );
    }

    #[test]
    fn empty_configuration_is_valid() {
        assert_eq!(
            config(r#"{"name":"test","fixtures":[]}"#).validate::<4>(),
            Ok(())
        );
    }

    #[test]
    fn non_overlapping_spans_on_the_same_channel_are_valid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":0,"length":3},
                    {"channel":0,"start":3,"length":2}
                ]}]
            }"#,
        );

        assert_eq!(device.validate::<4>(), Ok(()));
    }

    #[test]
    fn overlapping_spans_in_the_same_fixture_are_invalid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":0,"length":3},
                    {"channel":0,"start":2,"length":2}
                ]}]
            }"#,
        );

        assert_eq!(
            device.validate::<4>(),
            Err(DeviceConfigValidationError::OverlappingSpans)
        );
    }

    #[test]
    fn overlapping_spans_in_different_fixtures_are_invalid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[
                    {"name":"first","layout":{"linear_array":{}},"spans":[{"channel":0,"start":4,"length":3}]},
                    {"name":"second","layout":{"linear_array":{}},"spans":[{"channel":0,"start":6,"length":2}]}
                ]
            }"#,
        );

        assert_eq!(
            device.validate::<4>(),
            Err(DeviceConfigValidationError::OverlappingSpans)
        );
    }

    #[test]
    fn spans_on_different_channels_do_not_overlap() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":0,"length":4},
                    {"channel":1,"start":0,"length":4}
                ]}]
            }"#,
        );

        assert_eq!(device.validate::<4>(), Ok(()));
    }

    #[test]
    fn arithmetic_overflow_is_invalid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":0,"length":1},
                    {"channel":0,"start":4294967295,"length":1}
                ]}]
            }"#,
        );

        assert_eq!(
            device.validate::<4>(),
            Err(DeviceConfigValidationError::SpanEndOverflow)
        );
    }

    #[test]
    fn span_on_unavailable_output_channel_is_invalid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":4,"start":0,"length":1}
                ]}]
            }"#,
        );

        assert_eq!(
            device.validate::<4>(),
            Err(DeviceConfigValidationError::InvalidOutputChannel {
                channel: 4,
                output_channel_count: 4,
            })
        );
    }

    #[test]
    fn single_span_with_overflowing_end_is_invalid() {
        let device = config(
            r#"{
                "name":"test",
                "fixtures":[{"name":"fixture","layout":{"linear_array":{}},"spans":[
                    {"channel":0,"start":4294967295,"length":1}
                ]}]
            }"#,
        );

        assert_eq!(
            device.validate::<4>(),
            Err(DeviceConfigValidationError::SpanEndOverflow)
        );
    }
}
