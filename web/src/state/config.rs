use bled_core::config::{DeviceConfig, PixelLayout};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSpan {
    pub index: usize,
    pub name: String,
    pub channel: usize,
    pub start: usize,
    pub length: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFixture {
    pub index: usize,
    pub name: String,
    pub layout_name: String,
    pub total_pixels: usize,
    pub total_bytes: usize,
    pub spans: Vec<NormalizedSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedChannel {
    pub index: usize,
    pub name: String,
    pub length: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedConfig {
    pub name: String,
    pub fixtures: Vec<NormalizedFixture>,
    pub channel_lengths: Vec<usize>,
    pub channel_offsets: Vec<usize>,
    pub channels: Vec<NormalizedChannel>,
}

pub fn normalize_config(raw: &DeviceConfig) -> NormalizedConfig {
    let mut max_channel: isize = -1;
    for fixture in raw.fixtures().as_slice() {
        for span in fixture.spans() {
            let ch = usize::from(*span.channel().as_ref()) as isize;
            max_channel = max_channel.max(ch);
        }
    }

    let channel_count = (max_channel + 1).max(4) as usize;
    let mut channel_lengths = vec![0usize; channel_count];

    for fixture in raw.fixtures().as_slice() {
        for span in fixture.spans() {
            let channel = usize::from(*span.channel().as_ref());
            let end = (*span.start() + *span.length()) as usize;
            if channel < channel_count {
                channel_lengths[channel] = channel_lengths[channel].max(end);
            }
        }
    }

    let mut channel_offsets = vec![0usize; channel_count];
    let mut cursor = 0usize;
    for i in 0..channel_count {
        channel_offsets[i] = cursor;
        cursor += channel_lengths[i] * 3;
    }

    let mut fixtures = Vec::new();
    for (fixture_index, fixture) in raw.fixtures().as_slice().iter().enumerate() {
        let layout_name = match fixture.layout() {
            PixelLayout::LinearArray(_) => "Linear Array".into(),
            PixelLayout::CircularArray(_) => "Circular Array".into(),
        };

        let mut spans = Vec::new();
        for (span_index, span) in fixture.spans().iter().enumerate() {
            let channel = usize::from(*span.channel().as_ref());
            let start = *span.start() as usize;
            let length = *span.length() as usize;
            let byte_offset = channel_offsets.get(channel).copied().unwrap_or(0) + start * 3;

            spans.push(NormalizedSpan {
                index: span_index,
                name: span.name().to_string(),
                channel,
                start,
                length,
                byte_offset,
                byte_length: length * 3,
            });
        }

        let total_pixels = spans.iter().map(|s| s.length).sum();
        fixtures.push(NormalizedFixture {
            index: fixture_index,
            name: fixture.name().to_string(),
            layout_name,
            total_pixels,
            total_bytes: total_pixels * 3,
            spans,
        });
    }

    let channels = channel_lengths
        .iter()
        .enumerate()
        .map(|(index, &length)| NormalizedChannel {
            index,
            name: format!("Channel {index}"),
            length,
            byte_offset: channel_offsets[index],
            byte_length: length * 3,
        })
        .collect();

    NormalizedConfig {
        name: raw.name().to_string(),
        fixtures,
        channel_lengths,
        channel_offsets,
        channels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_core::{
        ChannelNumber,
        config::{Fixture, Fixtures, LinearArray, PixelSpan},
    };

    #[test]
    fn test_normalize_config() {
        let fixture = Fixture::new(
            "test-fixture".try_into().unwrap(),
            PixelLayout::LinearArray(LinearArray::default()),
            [
                PixelSpan::new("a".try_into().unwrap(), ChannelNumber::new(0), 0, 4),
                PixelSpan::new("b".try_into().unwrap(), ChannelNumber::new(1), 2, 5),
            ]
            .try_into()
            .unwrap(),
        );

        let config = DeviceConfig {
            name: "test-device".try_into().unwrap(),
            fixtures: Fixtures::new([fixture].into()),
            default_effect: None,
        };

        let norm = normalize_config(&config);
        assert_eq!(norm.name, "test-device");
        assert_eq!(norm.fixtures.len(), 1);
        assert_eq!(norm.channel_lengths[0], 4);
        assert_eq!(norm.channel_lengths[1], 7);
        assert_eq!(norm.channel_offsets[0], 0);
        assert_eq!(norm.channel_offsets[1], 12);
        assert_eq!(norm.fixtures[0].spans[0].byte_offset, 0);
        assert_eq!(norm.fixtures[0].spans[1].byte_offset, 12 + 6);
    }
}
