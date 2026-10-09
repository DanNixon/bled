use crate::api::{ChannelNumber, PixelIndex};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Span {
    channel: ChannelNumber,

    start: PixelIndex,

    length: PixelIndex,
}
