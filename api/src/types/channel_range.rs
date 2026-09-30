use crate::{ChannelError, MAX_CHANNEL_COUNT, PixelRanges};
use core::marker::PhantomData;
use getset::Getters;
use serde::{Deserialize, Serialize};

/// An owned, bounded sequence of pixel ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[serde(try_from = "RawChannelRanges<P, R>")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct ChannelRanges<P, R: PixelRanges<P>> {
    /// Zero-based index of the channel to update.
    channel: u8,

    /// The pixel ranges to update.
    ranges: R,

    #[serde(skip)]
    _pixel_type: PhantomData<P>,
}

#[derive(Deserialize)]
struct RawChannelRanges<P, R> {
    channel: u8,
    ranges: R,
    #[serde(skip)]
    _pixel_type: PhantomData<P>,
}

impl<P, R: PixelRanges<P>> TryFrom<RawChannelRanges<P, R>> for ChannelRanges<P, R> {
    type Error = ChannelError;

    fn try_from(raw: RawChannelRanges<P, R>) -> Result<Self, Self::Error> {
        Self::new(raw.channel, raw.ranges)
    }
}

impl<T, R: PixelRanges<T>> ChannelRanges<T, R> {
    /// Creates a bounded sequence of pixel ranges for one channel.
    pub fn new(channel: u8, ranges: R) -> Result<Self, ChannelError> {
        if usize::from(channel) >= MAX_CHANNEL_COUNT {
            return Err(ChannelError::InvalidChannel {
                channel,
                max: MAX_CHANNEL_COUNT,
            });
        }

        Ok(Self {
            channel,
            ranges,
            _pixel_type: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RGB8, Rgb8Ranges};

    #[test]
    fn cbor_round_trip() {
        let mut ranges = Rgb8Ranges::<6>::default();
        ranges.insert(2..5, RGB8::new(1, 2, 3)).unwrap();
        let value = ChannelRanges::new(3, ranges).unwrap();

        let mut data = [0; 128];

        let length = crate::io::encode_cbor(&value, &mut data).unwrap();
        assert_eq!(length, 38);
        assert_eq!(
            &data[..length],
            &[
                0xa2, 0x67, b'c', b'h', b'a', b'n', b'n', b'e', b'l', 0x03, 0x66, b'r', b'a', b'n',
                b'g', b'e', b's', 0xa5, 0x61, b's', 0x81, 0x02, 0x61, b'e', 0x81, 0x05, 0x61, b'r',
                0x81, 0x01, 0x61, b'g', 0x81, 0x02, 0x61, b'b', 0x81, 0x03,
            ]
        );

        let decoded =
            crate::io::decode_cbor::<ChannelRanges<RGB8, Rgb8Ranges<6>>>(&data[..length]).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn new_accepts_first_and_last_channels() {
        for channel in [0, MAX_CHANNEL_COUNT as u8 - 1] {
            let ranges = Rgb8Ranges::<6>::default();
            let value = ChannelRanges::new(channel, ranges).unwrap();

            assert_eq!(*value.channel(), channel);
        }
    }

    #[test]
    fn new_rejects_channels_outside_the_device() {
        for channel in [MAX_CHANNEL_COUNT as u8, u8::MAX] {
            assert_eq!(
                ChannelRanges::<RGB8, Rgb8Ranges<6>>::new(channel, Rgb8Ranges::default()),
                Err(ChannelError::InvalidChannel {
                    channel,
                    max: MAX_CHANNEL_COUNT,
                })
            );
        }
    }

    #[test]
    fn new_preserves_ranges() {
        let mut ranges = Rgb8Ranges::<6>::default();
        ranges.insert(2..5, RGB8::new(1, 2, 3)).unwrap();

        let value = ChannelRanges::new(3, ranges.clone()).unwrap();

        assert_eq!(*value.channel(), 3);
        assert_eq!(value.ranges(), &ranges);
    }

    #[test]
    fn deserialize_rejects_invalid_channel() {
        let json = br#"{"channel":8,"ranges":{"s":[],"e":[],"r":[],"g":[],"b":[]}}"#;
        let res = crate::io::decode_json::<ChannelRanges<RGB8, Rgb8Ranges<6>>>(json);
        assert!(res.is_err());
    }
}
