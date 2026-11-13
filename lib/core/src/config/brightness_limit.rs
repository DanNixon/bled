use crate::{
    PixelIndex, RGB8,
    config::{Fixtures, MAX_TOTAL_SPAN_COUNT},
};
use core::{cmp::Ordering, ops::Range};
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct BrightnessLimit {
    mode: BrightnessLimitMode,
    limit: RGB8,
}

impl Default for BrightnessLimit {
    fn default() -> Self {
        // Clamp to full brightness (i.e. do not perform any limiting)
        Self {
            mode: BrightnessLimitMode::Clamp,
            limit: RGB8::new(u8::MAX, u8::MAX, u8::MAX),
        }
    }
}

impl BrightnessLimit {
    #[must_use]
    pub const fn new(mode: BrightnessLimitMode, limit: RGB8) -> Self {
        Self { mode, limit }
    }

    #[must_use]
    pub fn apply(&self, pixel: RGB8) -> RGB8 {
        match self.mode {
            BrightnessLimitMode::Clamp => RGB8::new(
                pixel.r.clamp(0, self.limit.r),
                pixel.g.clamp(0, self.limit.g),
                pixel.b.clamp(0, self.limit.b),
            ),
            BrightnessLimitMode::Rescale => RGB8::new(
                ((u16::from(pixel.r) * u16::from(self.limit.r)) / u16::from(u8::MAX)) as u8,
                ((u16::from(pixel.g) * u16::from(self.limit.g)) / u16::from(u8::MAX)) as u8,
                ((u16::from(pixel.b) * u16::from(self.limit.b)) / u16::from(u8::MAX)) as u8,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[serde(rename_all = "snake_case")]
pub enum BrightnessLimitMode {
    /// Clamp the pixel/channel intensity to the limit (i.e. set the intensity to `limit` if it exceeds it).
    Clamp,

    /// Rescale the pixel/channel intensity to the limit (i.e. map the range `[0, 255]` to `[0, limit]`).
    Rescale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct SpanBrightnessLimit {
    range: Range<PixelIndex>,
    limit: BrightnessLimit,
}

/// Precomputed per-channel brightness limit lookup table derived from [`Fixtures`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChannelBrightnessLimits<const CHANNEL_COUNT: usize> {
    channel_ranges: [Range<usize>; CHANNEL_COUNT],
    spans: Vec<SpanBrightnessLimit, MAX_TOTAL_SPAN_COUNT>,
}

impl<const CHANNEL_COUNT: usize> ChannelBrightnessLimits<CHANNEL_COUNT> {
    /// Builds the per-channel brightness limit cache from a fixture configuration.
    ///
    /// Returns `None` if fixture validation fails (e.g. a span references an out-of-range
    /// channel, a span's pixel range overflows, or spans on the same channel overlap).
    #[must_use]
    pub fn from_fixtures(fixtures: &Fixtures) -> Option<Self> {
        fixtures.validate::<CHANNEL_COUNT>().ok()?;

        let mut channel_ranges = [const { 0..0 }; CHANNEL_COUNT];
        let mut spans = Vec::new();

        for (channel, channel_range) in channel_ranges.iter_mut().enumerate() {
            let start_idx = spans.len();

            for (_, _, span) in fixtures.spans() {
                if usize::from(*span.channel().as_ref()) == channel
                    && *span.length() > 0
                    && *span.brightness_limit() != BrightnessLimit::default()
                {
                    let end = span.start().checked_add(*span.length())?;
                    spans
                        .push(SpanBrightnessLimit {
                            range: *span.start()..end,
                            limit: *span.brightness_limit(),
                        })
                        .ok()?;
                }
            }

            spans[start_idx..].sort_unstable_by_key(|s| s.range.start);
            *channel_range = start_idx..spans.len();
        }

        Some(Self {
            channel_ranges,
            spans,
        })
    }

    /// Looks up the effective [`BrightnessLimit`] for a pixel on `channel`.
    ///
    /// Returns `None` if `channel >= CHANNEL_COUNT`. Pixels not covered by a span
    /// (or covered by a span with a no-op limit) return [`BrightnessLimit::default()`].
    #[must_use]
    pub fn get(&self, channel: usize, pixel_index: PixelIndex) -> Option<BrightnessLimit> {
        let range = self.channel_ranges.get(channel)?;
        let channel_spans = &self.spans[range.clone()];

        match channel_spans.binary_search_by(|span| {
            if pixel_index < span.range.start {
                Ordering::Greater
            } else if pixel_index >= span.range.end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }) {
            Ok(idx) => Some(channel_spans[idx].limit),
            Err(_) => Some(BrightnessLimit::default()),
        }
    }

    /// Returns an iterator that yields the pixels of `channel` with per-span brightness limits applied.
    ///
    /// Returns `None` if `channel >= CHANNEL_COUNT`.
    #[must_use]
    pub fn apply_channel<'a>(
        &'a self,
        channel: usize,
        pixels: &'a [RGB8],
    ) -> Option<ChannelBrightnessLimitIter<'a>> {
        let range = self.channel_ranges.get(channel)?;
        Some(ChannelBrightnessLimitIter {
            pixels: pixels.iter(),
            spans: &self.spans[range.clone()],
            pixel_index: 0,
        })
    }
}

/// Iterator that applies pre-sorted per-span brightness limits to a channel's pixels in O(1) time per pixel.
pub struct ChannelBrightnessLimitIter<'a> {
    pixels: core::slice::Iter<'a, RGB8>,
    spans: &'a [SpanBrightnessLimit],
    pixel_index: PixelIndex,
}

impl Iterator for ChannelBrightnessLimitIter<'_> {
    type Item = RGB8;

    fn next(&mut self) -> Option<Self::Item> {
        let pixel = *self.pixels.next()?;
        let index = self.pixel_index;
        self.pixel_index += 1;

        while let Some(span) = self.spans.first() {
            if index >= span.range.end {
                self.spans = &self.spans[1..];
            } else {
                break;
            }
        }

        if let Some(span) = self.spans.first()
            && index >= span.range.start
        {
            Some(span.limit.apply(pixel))
        } else {
            Some(pixel)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.pixels.size_hint()
    }
}

impl ExactSizeIterator for ChannelBrightnessLimitIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limit_is_noop() {
        let limit = BrightnessLimit::default();
        assert_eq!(*limit.mode(), BrightnessLimitMode::Clamp);
        assert_eq!(*limit.limit(), RGB8::new(255, 255, 255));
        assert_eq!(limit.apply(RGB8::new(0, 128, 255)), RGB8::new(0, 128, 255));
    }

    #[test]
    fn clamp_mode_clamps_channels_independently() {
        let limit = BrightnessLimit::new(BrightnessLimitMode::Clamp, RGB8::new(100, 50, 200));
        assert_eq!(limit.apply(RGB8::new(50, 50, 50)), RGB8::new(50, 50, 50));
        assert_eq!(
            limit.apply(RGB8::new(255, 128, 250)),
            RGB8::new(100, 50, 200)
        );
    }

    #[test]
    fn rescale_mode_scales_channels_proportionally() {
        let limit = BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(100, 50, 255));
        assert_eq!(limit.apply(RGB8::new(0, 0, 0)), RGB8::new(0, 0, 0));
        assert_eq!(
            limit.apply(RGB8::new(255, 255, 255)),
            RGB8::new(100, 50, 255)
        );
        assert_eq!(
            limit.apply(RGB8::new(128, 128, 128)),
            RGB8::new(50, 25, 128)
        );
    }

    #[test]
    fn rescale_mode_preserves_exact_bounds_for_all_limits() {
        for max in 0..=u8::MAX {
            let limit =
                BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(max, max, max));
            assert_eq!(limit.apply(RGB8::new(0, 0, 0)), RGB8::new(0, 0, 0));
            assert_eq!(
                limit.apply(RGB8::new(u8::MAX, u8::MAX, u8::MAX)),
                RGB8::new(max, max, max)
            );
        }
    }

    #[test]
    fn channel_brightness_limits_applies_spans_and_preserves_gaps() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                {
                    "name": "first",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {
                            "name": "b",
                            "channel": 0,
                            "start": 4,
                            "length": 2,
                            "limit": {
                                "mode": "rescale",
                                "limit": {"r": 100, "g": 50, "b": 255}
                            }
                        },
                        {
                            "name": "default_noop",
                            "channel": 1,
                            "start": 0,
                            "length": 2
                        }
                    ]
                },
                {
                    "name": "second",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {
                            "name": "a",
                            "channel": 0,
                            "start": 1,
                            "length": 2,
                            "limit": {
                                "mode": "clamp",
                                "limit": {"r": 60, "g": 70, "b": 80}
                            }
                        }
                    ]
                }
            ]"#,
        )
        .unwrap();

        let cache = ChannelBrightnessLimits::<4>::from_fixtures(&fixtures).unwrap();

        // Point lookups on channel 0:
        // pixel 0: gap -> default
        // pixels 1..3: "a" -> Clamp(60, 70, 80)
        // pixel 3: gap -> default
        // pixels 4..6: "b" -> Rescale(100, 50, 255)
        // pixel 6: after all spans -> default
        assert_eq!(cache.get(0, 0), Some(BrightnessLimit::default()));
        assert_eq!(
            cache.get(0, 1),
            Some(BrightnessLimit::new(
                BrightnessLimitMode::Clamp,
                RGB8::new(60, 70, 80)
            ))
        );
        assert_eq!(
            cache.get(0, 2),
            Some(BrightnessLimit::new(
                BrightnessLimitMode::Clamp,
                RGB8::new(60, 70, 80)
            ))
        );
        assert_eq!(cache.get(0, 3), Some(BrightnessLimit::default()));
        assert_eq!(
            cache.get(0, 4),
            Some(BrightnessLimit::new(
                BrightnessLimitMode::Rescale,
                RGB8::new(100, 50, 255)
            ))
        );
        assert_eq!(cache.get(0, 6), Some(BrightnessLimit::default()));
        assert_eq!(cache.get(4, 0), None);

        // Sequential streaming on channel 0
        let input = [RGB8::new(255, 255, 255); 7];
        let output: Vec<RGB8, 7> = cache.apply_channel(0, &input).unwrap().collect();
        assert_eq!(
            output.as_slice(),
            &[
                RGB8::new(255, 255, 255), // pixel 0 (gap)
                RGB8::new(60, 70, 80),    // pixel 1 (a : clamp)
                RGB8::new(60, 70, 80),    // pixel 2 (a : clamp)
                RGB8::new(255, 255, 255), // pixel 3 (gap)
                RGB8::new(100, 50, 255),  // pixel 4 (b : rescale)
                RGB8::new(100, 50, 255),  // pixel 5 (b : rescale)
                RGB8::new(255, 255, 255), // pixel 6 (past end)
            ]
        );

        // Channel 1 has only a default (no-op) span
        assert!(cache.channel_ranges[1].is_empty());
        assert_eq!(cache.get(1, 0), Some(BrightnessLimit::default()));
        let ch1_input = [RGB8::new(200, 150, 100); 2];
        let ch1_output: Vec<RGB8, 2> = cache.apply_channel(1, &ch1_input).unwrap().collect();
        assert_eq!(ch1_output.as_slice(), &ch1_input);
    }

    #[test]
    fn channel_brightness_limits_rejects_invalid_channel() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                {
                    "name": "fixture",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {"name": "a", "channel": 4, "start": 0, "length": 1}
                    ]
                }
            ]"#,
        )
        .unwrap();

        assert!(ChannelBrightnessLimits::<4>::from_fixtures(&fixtures).is_none());
    }

    #[test]
    fn channel_brightness_limits_rejects_overflowing_span_range() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                {
                    "name": "fixture",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {"name": "a", "channel": 0, "start": 4294967295, "length": 1}
                    ]
                }
            ]"#,
        )
        .unwrap();

        assert!(ChannelBrightnessLimits::<4>::from_fixtures(&fixtures).is_none());
    }

    #[test]
    fn channel_brightness_limits_rejects_overlapping_spans() {
        let fixtures: Fixtures = crate::io::decode_json(
            br#"[
                {
                    "name": "first",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {"name": "a", "channel": 0, "start": 0, "length": 3}
                    ]
                },
                {
                    "name": "second",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {"name": "b", "channel": 0, "start": 2, "length": 2}
                    ]
                }
            ]"#,
        )
        .unwrap();

        assert!(ChannelBrightnessLimits::<4>::from_fixtures(&fixtures).is_none());
    }
}
