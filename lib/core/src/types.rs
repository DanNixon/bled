use crate::{api::MAX_PATH_LEN, config::MAX_NAME_LEN};
use heapless::String;
use nutype::nutype;

pub type NameString = String<{ MAX_NAME_LEN }>;
pub type PathString = String<{ MAX_PATH_LEN }>;

/// Human-readable device name.
#[nutype(
    validate(predicate = |s: &NameString| !s.is_empty()),
    default = NameString::try_from("bled-default").unwrap(),
    derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Default,
        Serialize,
        Deserialize,
        AsRef,
        Deref,
        Display,
        FromStr,
    ),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct DeviceName(NameString);

pub use rgb::RGB8;

pub type PixelIndex = u32;

#[nutype(
    derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef),
    cfg_attr(feature = "defmt", derive_unchecked(defmt::Format))
)]
pub struct ChannelNumber(u8);

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    #[test]
    fn device_name_default_and_validation() {
        let default = DeviceName::default();
        assert_eq!(default.as_str(), "bled-default");

        assert!(DeviceName::from_str("").is_err());
        assert_eq!(
            DeviceName::from_str("my-device").unwrap().as_str(),
            "my-device"
        );
        assert!(DeviceName::from_str("0123456789abcdef").is_ok());
        assert!(DeviceName::from_str("0123456789abcdef0").is_err());
    }
}
