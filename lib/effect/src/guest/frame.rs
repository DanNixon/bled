use bled_core::RGB8;

/// A view over a three-bytes-per-pixel RGB frame buffer.
pub struct Frame<'a> {
    data: &'a mut [u8],
}

impl<'a> Frame<'a> {
    pub(crate) fn new(data: &'a mut [u8]) -> Self {
        assert!(data.len().is_multiple_of(3));
        Self { data }
    }

    /// View the frame as raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.data
    }

    /// View the frame as mutable raw bytes.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        self.data
    }

    /// View the frame as RGB pixels.
    pub fn pixels(&self) -> &[RGB8] {
        bytemuck::cast_slice(self.data)
    }

    /// View the frame as mutable RGB pixels.
    pub fn pixels_mut(&mut self) -> &mut [RGB8] {
        bytemuck::cast_slice_mut(self.data)
    }
}
