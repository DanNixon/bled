mod rgb8;

use core::ops::Range;
use thiserror::Error;

pub use rgb8::{RGB8, Rgb8Ranges};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PixelError {
    /// The number of pixel ranges exceeds the container capacity.
    #[error("pixel range count {quantity} exceeds capacity {limit}")]
    TooManyRanges { quantity: usize, limit: usize },

    /// A pixel range is invalid (e.g. start is greater than or equal to end).
    #[error("pixel range start ({start}) >= end ({end})")]
    InvalidRange { start: u16, end: u16 },

    /// Color plane lengths do not match.
    #[error("mismatched color plane lengths")]
    MismatchedPlanes,
}

pub trait PixelRanges<T>: Sized + Default {
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn is_full(&self) -> bool;

    fn insert(&mut self, range: Range<u16>, value: T) -> Result<(), PixelError>;

    fn iter(&self) -> impl Iterator<Item = (Range<usize>, T)>;
}
