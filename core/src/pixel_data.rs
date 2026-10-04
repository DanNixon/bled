use crate::{LedBufferInformation, MAX_CHANNEL_COUNT};
use bytemuck::{AnyBitPattern, NoUninit, Pod};
use core::ops::Range;
use heapless::Vec;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PixelDataBufferError {
    /// The number of channels provided to reshape exceeds the maximum channel count.
    #[error("too many channels (provided: {provided}, capacity: {capacity})")]
    TooManyChannels { provided: usize, capacity: usize },

    /// The total required buffer size exceeds the allocated capacity.
    #[error("required buffer size ({required}) exceeds capacity ({capacity})")]
    BufferOverflow { required: usize, capacity: usize },

    /// A channel index is out of bounds for the buffer.
    #[error("channel index {channel} is out of bounds (channel count: {channel_count})")]
    InvalidChannel {
        channel: usize,
        channel_count: usize,
    },

    /// The slice memory alignment or size is invalid for the requested pixel type.
    #[error("slice memory alignment or size invalid for pixel type")]
    TypeCastFailed,

    /// The number of diff ranges exceeds the vector capacity.
    #[error("diff range count exceeds capacity ({capacity})")]
    DiffOverflow { capacity: usize },

    /// A diff was requested between buffers of different sizes.
    #[error("diff size mismatch (a: {a}, b: {b})")]
    DiffSizeMismatch { a: usize, b: usize },
}

pub struct PixelDataBuffer<const LEN: usize> {
    ranges: Vec<Range<usize>, { MAX_CHANNEL_COUNT }>,
    data: [u8; LEN],
}

impl<const LEN: usize> Default for PixelDataBuffer<LEN> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const LEN: usize> PixelDataBuffer<LEN> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ranges: Vec::new(),
            data: [0u8; LEN],
        }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        LEN
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.ranges.iter().map(|r| r.end - r.start).sum()
    }

    #[must_use]
    pub fn info(&self) -> LedBufferInformation {
        LedBufferInformation::new(self.capacity() as u32, self.size() as u32)
    }

    #[must_use]
    pub fn inner(&self) -> &[u8] {
        &self.data
    }

    pub fn inner_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn try_reshape(
        &mut self,
        sizes: &[PixelDataChannelSize],
    ) -> Result<(), PixelDataBufferError> {
        self.ranges.clear();

        if sizes.len() > self.ranges.capacity() {
            return Err(PixelDataBufferError::TooManyChannels {
                provided: sizes.len(),
                capacity: self.ranges.capacity(),
            });
        }

        let mut required = 0usize;
        for size in sizes {
            let Some(bytes) = size.bytes() else {
                return Err(PixelDataBufferError::BufferOverflow {
                    required: usize::MAX,
                    capacity: LEN,
                });
            };
            let Some(next) = required.checked_add(bytes) else {
                return Err(PixelDataBufferError::BufferOverflow {
                    required: usize::MAX,
                    capacity: LEN,
                });
            };
            required = next;
        }

        if required > LEN {
            return Err(PixelDataBufferError::BufferOverflow {
                required,
                capacity: LEN,
            });
        }

        let mut cursor = 0;
        for size in sizes.iter() {
            let a = cursor;
            let b = cursor + size.bytes().unwrap_or(0);

            self.ranges.push(a..b).unwrap();
            cursor = b;
        }

        Ok(())
    }

    pub fn try_channel<P: AnyBitPattern>(
        &self,
        channel: usize,
    ) -> Result<&[P], PixelDataBufferError> {
        let r = self
            .ranges
            .get(channel)
            .ok_or(PixelDataBufferError::InvalidChannel {
                channel,
                channel_count: self.ranges.len(),
            })?;
        let s = self
            .data
            .get(r.start..r.end)
            .ok_or(PixelDataBufferError::InvalidChannel {
                channel,
                channel_count: self.ranges.len(),
            })?;
        bytemuck::try_cast_slice(s).map_err(|_| PixelDataBufferError::TypeCastFailed)
    }

    pub fn try_channel_mut<P: AnyBitPattern + NoUninit>(
        &mut self,
        channel: usize,
    ) -> Result<&mut [P], PixelDataBufferError> {
        let r = self
            .ranges
            .get(channel)
            .ok_or(PixelDataBufferError::InvalidChannel {
                channel,
                channel_count: self.ranges.len(),
            })?;
        let s = self
            .data
            .get_mut(r.start..r.end)
            .ok_or(PixelDataBufferError::InvalidChannel {
                channel,
                channel_count: self.ranges.len(),
            })?;
        bytemuck::try_cast_slice_mut(s).map_err(|_| PixelDataBufferError::TypeCastFailed)
    }

    /// Returns a vector of ranges of the underlying pixel data that differ between `self` and `other`.
    /// Ranges that are separated by less than `min_space` bytes are considered as a single range.
    /// Ranges are limited to `chunk_size` bytes per range (potentially splitting contiguous changed spans).
    /// If `chunk_size` is 0, no chunk size limit is applied.
    pub fn diff<const N: usize>(
        &self,
        other: &Self,
        min_space: usize,
        chunk_size: usize,
    ) -> Result<Vec<Range<usize>, N>, PixelDataBufferError> {
        let chunk_size = if chunk_size == 0 {
            usize::MAX
        } else {
            chunk_size
        };
        let mut ranges = Vec::new();
        let mut current: Option<Range<usize>> = None;

        if self.size() != other.size() {
            return Err(PixelDataBufferError::DiffSizeMismatch {
                a: self.size(),
                b: other.size(),
            });
        }
        let limit = self.size();

        for (i, (&a, &b)) in self.data[..limit]
            .iter()
            .zip(other.data[..limit].iter())
            .enumerate()
        {
            if a != b {
                match current {
                    Some(ref mut curr) => {
                        let space = i - curr.end;
                        let len = (i + 1) - curr.start;
                        if (space == 0 || space < min_space) && len <= chunk_size {
                            curr.end = i + 1;
                        } else {
                            ranges
                                .push(curr.clone())
                                .map_err(|_| PixelDataBufferError::DiffOverflow { capacity: N })?;
                            *curr = Range {
                                start: i,
                                end: i + 1,
                            };
                        }
                    }
                    None => {
                        current = Some(Range {
                            start: i,
                            end: i + 1,
                        });
                    }
                }
            }
        }

        if let Some(curr) = current {
            ranges
                .push(curr)
                .map_err(|_| PixelDataBufferError::DiffOverflow { capacity: N })?;
        }

        Ok(ranges)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PixelDataChannelSize {
    count: usize,
    bytes_per_pixel: usize,
}

impl PixelDataChannelSize {
    #[must_use]
    pub const fn new<P: Pod>(count: usize) -> Self {
        Self {
            count,
            bytes_per_pixel: core::mem::size_of::<P>(),
        }
    }

    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn bytes_per_pixel(&self) -> usize {
        self.bytes_per_pixel
    }

    #[must_use]
    pub const fn bytes(&self) -> Option<usize> {
        self.count.checked_mul(self.bytes_per_pixel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgb::RGB8;

    #[test]
    fn buffer_reshape_and_channel_access() {
        let mut buffer = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(3),
        ];

        assert_eq!(buffer.try_reshape(&sizes), Ok(()));
        let ch0 = buffer.try_channel_mut::<RGB8>(0).unwrap();
        assert_eq!(ch0.len(), 2);
        ch0[0] = RGB8::new(1, 2, 3);
        ch0[1] = RGB8::new(4, 5, 6);

        let ch1 = buffer.try_channel_mut::<RGB8>(1).unwrap();
        assert_eq!(ch1.len(), 3);
        ch1[2] = RGB8::new(7, 8, 9);

        assert_eq!(
            buffer.try_channel::<RGB8>(0).unwrap()[0],
            RGB8::new(1, 2, 3)
        );
        assert_eq!(
            buffer.try_channel::<RGB8>(1).unwrap()[2],
            RGB8::new(7, 8, 9)
        );
    }

    #[test]
    fn buffer_reshape_overflow() {
        let mut buffer = PixelDataBuffer::<10>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2), // 6 bytes
            PixelDataChannelSize::new::<RGB8>(2), // 6 bytes -> total 12 > 10
        ];

        assert_eq!(
            buffer.try_reshape(&sizes),
            Err(PixelDataBufferError::BufferOverflow {
                required: 12,
                capacity: 10,
            })
        );
    }

    #[test]
    fn buffer_reshape_size_multiplication_overflow() {
        let mut buffer = PixelDataBuffer::<10>::new();
        let size = PixelDataChannelSize::new::<RGB8>(usize::MAX);
        assert_eq!(size.count(), usize::MAX);
        assert_eq!(size.bytes_per_pixel(), 3);
        assert_eq!(size.bytes(), None);

        assert_eq!(
            buffer.try_reshape(&[size]),
            Err(PixelDataBufferError::BufferOverflow {
                required: usize::MAX,
                capacity: 10,
            })
        );
    }

    #[test]
    fn buffer_channel_out_of_bounds() {
        let buffer = PixelDataBuffer::<32>::new();
        assert_eq!(
            buffer.try_channel::<RGB8>(2),
            Err(PixelDataBufferError::InvalidChannel {
                channel: 2,
                channel_count: 0,
            })
        );
    }

    #[test]
    fn buffer_diff_identical() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert!(diffs.is_empty());
    }

    #[test]
    fn buffer_diff_single_byte() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[5] = 42;

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 5, end: 6 }]);
    }

    #[test]
    fn buffer_diff_contiguous_bytes() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[5..8].copy_from_slice(&[1, 2, 3]);

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 5, end: 8 }]);
    }

    #[test]
    fn buffer_diff_min_space_merging() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // Byte 2 and 5 differ. Separation is 2 bytes (indices 3 and 4).
        a.inner_mut()[2] = 1;
        a.inner_mut()[5] = 2;

        // With min_space = 2, separation 2 is not < 2, so keep separate.
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 2, 32).unwrap();
        assert_eq!(
            &diffs[..],
            &[Range { start: 2, end: 3 }, Range { start: 5, end: 6 }]
        );

        // With min_space = 3, separation 2 is < 3, so merged.
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 3, 32).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 2, end: 6 }]);
    }

    #[test]
    fn buffer_diff_min_space_zero() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // Bytes 2 and 4 differ. Separation is 1 byte (index 3).
        a.inner_mut()[2] = 1;
        a.inner_mut()[4] = 2;

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert_eq!(
            &diffs[..],
            &[Range { start: 2, end: 3 }, Range { start: 4, end: 5 }]
        );
    }

    #[test]
    fn buffer_diff_capacity_overflow() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[2] = 1;
        a.inner_mut()[5] = 2;
        a.inner_mut()[8] = 3;

        // Capacity is 2, but 3 distinct ranges exist (fails on post-loop push).
        assert_eq!(
            a.diff::<2>(&b, 0, 32),
            Err(PixelDataBufferError::DiffOverflow { capacity: 2 })
        );

        // 4 distinct ranges (fails inside the loop).
        a.inner_mut()[11] = 4;
        assert_eq!(
            a.diff::<2>(&b, 0, 32),
            Err(PixelDataBufferError::DiffOverflow { capacity: 2 })
        );
    }

    #[test]
    fn buffer_diff_with_channel_writes() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // Mutate channel 1, pixel 0 (bytes 6..9)
        a.try_channel_mut::<RGB8>(1).unwrap()[0] = RGB8::new(10, 20, 30);

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 6, end: 9 }]);
    }

    #[test]
    fn buffer_diff_ignores_unconfigured_trailing_capacity() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(1), // 3 bytes
            PixelDataChannelSize::new::<RGB8>(1), // 3 bytes -> total 6 bytes
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // Mutate byte 20 (beyond configured 6 bytes)
        a.inner_mut()[20] = 99;
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert!(diffs.is_empty());

        // Mutate byte 2 (within active channel 0)
        a.inner_mut()[2] = 42;
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 32).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 2, end: 3 }]);
    }

    #[test]
    fn buffer_diff_chunk_size_splitting() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // 10 contiguous bytes differ (0..10)
        a.inner_mut()[0..10].fill(1);

        // chunk_size = 4 splits into 0..4, 4..8, 8..10
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 4).unwrap();
        assert_eq!(
            &diffs[..],
            &[
                Range { start: 0, end: 4 },
                Range { start: 4, end: 8 },
                Range { start: 8, end: 10 },
            ]
        );
    }

    #[test]
    fn buffer_diff_chunk_size_exact_multiple() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[0..8].fill(1);

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 4).unwrap();
        assert_eq!(
            &diffs[..],
            &[Range { start: 0, end: 4 }, Range { start: 4, end: 8 },]
        );
    }

    #[test]
    fn buffer_diff_chunk_size_one() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[0..3].fill(1);

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 1).unwrap();
        assert_eq!(
            &diffs[..],
            &[
                Range { start: 0, end: 1 },
                Range { start: 1, end: 2 },
                Range { start: 2, end: 3 },
            ]
        );
    }

    #[test]
    fn buffer_diff_chunk_size_with_min_space() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // Bytes 0 and 2 differ (gap of 1 at index 1).
        a.inner_mut()[0] = 1;
        a.inner_mut()[2] = 2;

        // With min_space = 2 and chunk_size = 4, span 0..3 fits in one chunk.
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 2, 4).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 0, end: 3 }]);

        // With min_space = 2 and chunk_size = 2, span 0..3 exceeds chunk_size,
        // so byte 0 and byte 2 are kept in separate chunks.
        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 2, 2).unwrap();
        assert_eq!(
            &diffs[..],
            &[Range { start: 0, end: 1 }, Range { start: 2, end: 3 }]
        );
    }

    #[test]
    fn buffer_diff_chunk_size_overflow() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        // 10 contiguous bytes split by chunk_size 2 requires 5 ranges.
        a.inner_mut()[0..10].fill(1);

        assert_eq!(
            a.diff::<4>(&b, 0, 2),
            Err(PixelDataBufferError::DiffOverflow { capacity: 4 })
        );
    }

    #[test]
    fn buffer_diff_chunk_size_zero_means_unlimited() {
        let mut a = PixelDataBuffer::<32>::new();
        let mut b = PixelDataBuffer::<32>::new();
        let sizes = [
            PixelDataChannelSize::new::<RGB8>(2),
            PixelDataChannelSize::new::<RGB8>(2),
        ];
        a.try_reshape(&sizes).unwrap();
        b.try_reshape(&sizes).unwrap();

        a.inner_mut()[0..10].fill(1);

        let diffs: Vec<Range<usize>, 4> = a.diff(&b, 0, 0).unwrap();
        assert_eq!(&diffs[..], &[Range { start: 0, end: 10 }]);
    }

    #[test]
    fn buffer_capacity_size_info() {
        let mut buffer = PixelDataBuffer::<64>::new();
        assert_eq!(buffer.capacity(), 64);
        assert_eq!(buffer.size(), 0);
        assert_eq!(buffer.info().capacity(), &64u32);
        assert_eq!(buffer.info().size(), &0u32);

        let sizes = [
            PixelDataChannelSize::new::<RGB8>(5),  // 15 bytes
            PixelDataChannelSize::new::<RGB8>(10), // 30 bytes
        ];
        buffer.try_reshape(&sizes).unwrap();
        assert_eq!(buffer.capacity(), 64);
        assert_eq!(buffer.size(), 45);
        assert_eq!(buffer.info().capacity(), &64u32);
        assert_eq!(buffer.info().size(), &45u32);
    }
}
