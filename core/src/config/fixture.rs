use crate::{
    NameString,
    config::{PixelLayout, PixelSpan},
};
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Maximum number of spans that may be used in a single fixture.
pub const MAX_SPAN_PER_FIXTURE_COUNT: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Fixture {
    /// Human-readable fixture name.
    name: NameString,

    /// Layout of the pixels in this fixture.
    layout: PixelLayout,

    /// Physical pixel spans that make up this fixture.
    spans: Vec<PixelSpan, { MAX_SPAN_PER_FIXTURE_COUNT }>,
}

impl Fixture {
    pub fn new(
        name: NameString,
        layout: PixelLayout,
        spans: Vec<PixelSpan, { MAX_SPAN_PER_FIXTURE_COUNT }>,
    ) -> Self {
        Self {
            name,
            layout,
            spans,
        }
    }

    /// Get a span by name.
    pub fn find_span(&self, name: &str) -> Option<&PixelSpan> {
        self.spans.iter().find(|s| s.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<Fixture>(), 360);
    }
}
