use crate::{
    api::MAX_CHANNEL_COUNT,
    config::{Fixture, MAX_FIXTURE_COUNT, MAX_NAME_LEN, Span},
};
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: String<{ MAX_NAME_LEN }>,

    /// Light fixtures controlled by this device.
    pub fixtures: Vec<Fixture, { MAX_FIXTURE_COUNT }>,
}

impl DeviceConfig {
    /// Gets the required length of output channels.
    pub fn channel_lengths(&self) -> [usize; MAX_CHANNEL_COUNT] {
        let mut lengths = [0; MAX_CHANNEL_COUNT];

        for fixture in &self.fixtures {
            for span in fixture.spans() {
                let channel = usize::from(*span.channel().as_ref());
                let end = (*span.start() as usize).saturating_add(*span.length() as usize);
                lengths[channel] = lengths[channel].max(end);
            }
        }

        lengths
    }

    /// Validates the device config.
    pub fn validate(&self) -> Result<(), ()> {
        let overlaps = |first: &Span, second: &Span| -> Result<bool, ()> {
            if first.channel() != second.channel() {
                return Ok(false);
            }

            let first_end = first.start().checked_add(*first.length()).ok_or(())?;
            let second_end = second.start().checked_add(*second.length()).ok_or(())?;

            Ok(*first.start() < second_end && *second.start() < first_end)
        };

        for (fixture_index, fixture) in self.fixtures.iter().enumerate() {
            for (span_index, span) in fixture.spans().iter().enumerate() {
                // Compare against earlier spans in the same fixture.
                for previous in fixture.spans().iter().take(span_index) {
                    if overlaps(span, previous)? {
                        return Err(());
                    }
                }

                // Compare against spans in earlier fixtures.
                for previous_fixture in self.fixtures.iter().take(fixture_index) {
                    for previous in previous_fixture.spans() {
                        if overlaps(span, previous)? {
                            return Err(());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceConfig;

    fn config(json: &str) -> DeviceConfig {
        serde_json_core::from_slice(json.as_bytes()).unwrap().0
    }

    #[test]
    fn empty_configuration_has_zero_length_channels() {
        let device = config(r#"{"name":"test","fixtures":[]}"#);

        assert_eq!(device.channel_lengths().as_slice(), &[0; 8]);
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
                        {"channel":1,"start":1,"length":6},
                        {"channel":3,"start":5,"length":2}
                    ]}
                ]
            }"#,
        );

        assert_eq!(
            device.channel_lengths().as_slice(),
            &[5, 7, 0, 7, 0, 0, 0, 0]
        );
    }

    #[test]
    fn empty_configuration_is_valid() {
        assert_eq!(
            config(r#"{"name":"test","fixtures":[]}"#).validate(),
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

        assert_eq!(device.validate(), Ok(()));
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

        assert_eq!(device.validate(), Err(()));
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

        assert_eq!(device.validate(), Err(()));
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

        assert_eq!(device.validate(), Ok(()));
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

        assert_eq!(device.validate(), Err(()));
    }
}
