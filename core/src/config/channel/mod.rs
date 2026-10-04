mod config;
mod layouts;
mod section;

pub use config::*;
pub use layouts::*;
pub use section::*;

/// A range of pixels in a single channel.
pub trait PixelSpan {
    /// Number of pixels in this span.
    fn len(&self) -> u32;

    /// Returns `true` if this span is empty (i.e. has no pixels).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
