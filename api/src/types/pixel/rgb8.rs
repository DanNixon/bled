use super::{PixelError, PixelRanges};
use core::ops::Range;
use heapless::Vec;
use serde::{Deserialize, Serialize};

pub use rgb::RGB8;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(try_from = "RawRgb8Ranges<N>")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rgb8Ranges<const N: usize> {
    #[serde(rename = "s")]
    start: Vec<u16, N>,

    #[serde(rename = "e")]
    end: Vec<u16, N>,

    #[serde(rename = "r")]
    red: Vec<u8, N>,

    #[serde(rename = "g")]
    green: Vec<u8, N>,

    #[serde(rename = "b")]
    blue: Vec<u8, N>,
}

#[derive(Deserialize)]
struct RawRgb8Ranges<const N: usize> {
    #[serde(rename = "s")]
    start: Vec<u16, N>,

    #[serde(rename = "e")]
    end: Vec<u16, N>,

    #[serde(rename = "r")]
    red: Vec<u8, N>,

    #[serde(rename = "g")]
    green: Vec<u8, N>,

    #[serde(rename = "b")]
    blue: Vec<u8, N>,
}

impl<const N: usize> TryFrom<RawRgb8Ranges<N>> for Rgb8Ranges<N> {
    type Error = PixelError;

    fn try_from(raw: RawRgb8Ranges<N>) -> Result<Self, Self::Error> {
        let len = raw.start.len();
        if raw.end.len() != len
            || raw.red.len() != len
            || raw.green.len() != len
            || raw.blue.len() != len
        {
            return Err(PixelError::MismatchedPlanes);
        }

        for (&start, &end) in raw.start.iter().zip(raw.end.iter()) {
            if start >= end {
                return Err(PixelError::InvalidRange { start, end });
            }
        }

        Ok(Self {
            start: raw.start,
            end: raw.end,
            red: raw.red,
            green: raw.green,
            blue: raw.blue,
        })
    }
}

impl<const N: usize> PixelRanges<RGB8> for Rgb8Ranges<N> {
    fn capacity(&self) -> usize {
        self.start.capacity()
    }

    fn len(&self) -> usize {
        self.start.len()
    }

    fn is_full(&self) -> bool {
        self.start.is_full()
    }

    fn insert(&mut self, range: Range<u16>, value: RGB8) -> Result<(), PixelError> {
        if range.start >= range.end {
            return Err(PixelError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }

        if self.start.is_full() {
            Err(PixelError::TooManyRanges {
                quantity: self.len() + 1,
                limit: self.capacity(),
            })
        } else {
            self.start.push(range.start).unwrap();
            self.end.push(range.end).unwrap();
            self.red.push(value.r).unwrap();
            self.green.push(value.g).unwrap();
            self.blue.push(value.b).unwrap();
            Ok(())
        }
    }

    fn iter(&self) -> impl Iterator<Item = (Range<usize>, RGB8)> {
        let range = 0..self.len();
        range.into_iter().map(|i| {
            let start = self.start[i] as usize;
            let end = self.end[i] as usize;
            (
                start..end,
                RGB8::new(self.red[i], self.green[i], self.blue[i]),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_insert_valid_and_full() {
        let mut ranges = Rgb8Ranges::<2>::default();
        assert_eq!(ranges.insert(0..10, RGB8::new(1, 2, 3)), Ok(()));
        assert_eq!(ranges.insert(10..20, RGB8::new(4, 5, 6)), Ok(()));
        assert_eq!(
            ranges.insert(20..30, RGB8::new(7, 8, 9)),
            Err(PixelError::TooManyRanges {
                quantity: 3,
                limit: 2,
            })
        );
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)]
    fn ranges_insert_invalid_range() {
        let mut ranges = Rgb8Ranges::<2>::default();
        assert_eq!(
            ranges.insert(10..10, RGB8::new(1, 2, 3)),
            Err(PixelError::InvalidRange { start: 10, end: 10 })
        );
        assert_eq!(
            ranges.insert(20..10, RGB8::new(1, 2, 3)),
            Err(PixelError::InvalidRange { start: 20, end: 10 })
        );
    }

    #[test]
    fn ranges_deserialize_rejects_mismatched_planes() {
        // JSON: {"s":[0],"e":[10],"r":[1],"g":[2],"b":[]}
        let json = br#"{"s":[0],"e":[10],"r":[1],"g":[2],"b":[]}"#;
        let res = crate::io::decode_json::<Rgb8Ranges<4>>(json);
        assert!(res.is_err());
    }

    #[test]
    fn ranges_deserialize_rejects_invalid_range() {
        // JSON: {"s":[10],"e":[5],"r":[1],"g":[2],"b":[3]}
        let json = br#"{"s":[10],"e":[5],"r":[1],"g":[2],"b":[3]}"#;
        let res = crate::io::decode_json::<Rgb8Ranges<4>>(json);
        assert!(res.is_err());
    }
}
