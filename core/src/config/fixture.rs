use crate::config::{MAX_NAME_LEN, MAX_SPAN_PER_FIXTURE_COUNT, PixelLayout, Span};
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Fixture {
    /// Human-readable fixture name.
    name: String<{ MAX_NAME_LEN }>,

    layout: PixelLayout,

    spans: Vec<Span, { MAX_SPAN_PER_FIXTURE_COUNT }>,
}
