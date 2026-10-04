use crate::{api::LedBufferInformation, config::MAX_CHANNEL_COUNT};
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
