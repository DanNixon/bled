use crate::{ChannelConfig, MAX_CHANNEL_COUNT, MAX_NAME_LEN};
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: String<{ MAX_NAME_LEN }>,

    pub channels: Vec<ChannelConfig, { MAX_CHANNEL_COUNT }>,
}

impl DeviceConfig {
    #[must_use]
    pub fn new(
        name: String<{ MAX_NAME_LEN }>,
        channels: Vec<ChannelConfig, { MAX_CHANNEL_COUNT }>,
    ) -> Self {
        Self { name, channels }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChannelSectionConfig, ChannelSectionMode};

    #[test]
    fn json_round_trip() {
        let strip_config: Vec<_, _> = [
            ChannelSectionConfig::new(
                "onboard".try_into().unwrap(),
                0,
                ChannelSectionMode::single_pixel(),
            ),
            ChannelSectionConfig::new(
                "test".try_into().unwrap(),
                1,
                ChannelSectionMode::linear_array(12),
            ),
        ]
        .into();

        let config = DeviceConfig::new(
            "bled".try_into().unwrap(),
            [
                ChannelConfig::new("a".try_into().unwrap(), strip_config.clone()),
                ChannelConfig::new("b".try_into().unwrap(), strip_config.clone()),
                ChannelConfig::new("c".try_into().unwrap(), strip_config.clone()),
                ChannelConfig::new("d".try_into().unwrap(), strip_config),
            ]
            .into(),
        );

        let mut bytes = [0u8; 2048];
        let len = crate::io::encode_json(&config, &mut bytes).unwrap();
        let decoded: DeviceConfig = crate::io::decode_json(&bytes[..len]).unwrap();
        assert_eq!(config, decoded);
    }
}
