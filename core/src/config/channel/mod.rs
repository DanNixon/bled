mod config;
mod layouts;
mod section;

pub use config::*;
pub use layouts::*;
pub use section::*;

use core::ops::Range;

/// A range of pixels of known length in a single channel.
pub trait SizedPixelSpan {
    /// Number of pixels in this span.
    fn len(&self) -> u32;

    /// Returns `true` if this span is empty (i.e. has no pixels).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A range of pixels at a known position in a single channel.
pub trait PositionalPixelSpan {
    /// Returns the pixels in this span as a range of the channel.
    fn range(&self) -> Range<u32>;
}
