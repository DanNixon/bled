use nutype::nutype;

pub use rgb::RGB8;

pub type PixelIndex = u32;

#[nutype(
    derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct ChannelNumber(u8);
