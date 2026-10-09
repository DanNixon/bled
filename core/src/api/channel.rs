use crate::api::MAX_CHANNEL_COUNT;
use nutype::nutype;

#[nutype(
    validate(greater_or_equal = 0, less = MAX_CHANNEL_COUNT as u8),
    derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct ChannelNumber(u8);
