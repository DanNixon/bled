use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceInfo {
    git_revision: String<32>,
    boot_reason: BootReason,
    uptime_ms: u64,
    led_buffer_capacity: u64,
    channel_count: u8,
}

impl DeviceInfo {
    #[must_use]
    pub fn new(
        git_revision: String<32>,
        boot_reason: BootReason,
        uptime_ms: u64,
        led_buffer_capacity: u64,
        channel_count: u8,
    ) -> Self {
        Self {
            git_revision,
            boot_reason,
            uptime_ms,
            led_buffer_capacity,
            channel_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BootReason {
    Normal,
    WatchdogForced,
    WatchdogTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_round_trip() {
        let value = DeviceInfo::new(
            heapless::String::try_from("a1b2c3d4").unwrap(),
            BootReason::WatchdogTimeout,
            42_000,
            16 * 1024,
            4,
        );
        let mut data = [0; 128];

        let length = crate::io::encode_cbor(&value, &mut data).unwrap();
        assert_eq!(length, 64);

        let decoded = crate::io::decode_cbor::<DeviceInfo>(&data[..length]).unwrap();
        assert_eq!(decoded, value);
    }
}
