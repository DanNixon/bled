//! UUIDs of the emulated serial GATT service.
//!
//! The emulated serial service provides a bidirectional byte stream over BLE,
//! the device API RPC protocol runs over this stream.

use nutype::nutype;
use uuid::{Uuid, uuid};

/// A 6 digit Bluetooth Low Energy pairing passkey.
#[nutype(
    validate(less_or_equal = 999_999),
    default = 123_456,
    derive(
        Debug,
        Copy,
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
pub struct BlePasskey(u32);

/// Emulated serial GATT service UUID (Nordic UART Service).
pub const SERIAL_SERVICE_UUID: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e");

/// Emulated serial RX GATT characteristic UUID (host to device, writable).
pub const SERIAL_RX_UUID: Uuid = uuid!("6e400002-b5a3-f393-e0a9-e50e24dcca9e");

/// Emulated serial TX GATT characteristic UUID (device to host, notifiable).
pub const SERIAL_TX_UUID: Uuid = uuid!("6e400003-b5a3-f393-e0a9-e50e24dcca9e");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passkey_validation_enforces_six_digits() {
        use core::str::FromStr;

        assert_eq!(*BlePasskey::default(), 123_456);
        assert!(BlePasskey::try_new(0).is_ok());
        assert!(BlePasskey::try_new(123_456).is_ok());
        assert!(BlePasskey::try_new(999_999).is_ok());
        assert!(BlePasskey::try_new(1_000_000).is_err());

        assert_eq!(*BlePasskey::from_str("654321").unwrap(), 654_321);
        assert!(BlePasskey::from_str("1000000").is_err());
        assert!(BlePasskey::from_str("abc").is_err());
    }

    #[test]
    fn uuid_constants_are_valid_and_unique() {
        let all = [SERIAL_SERVICE_UUID, SERIAL_RX_UUID, SERIAL_TX_UUID];

        for (index, &uuid) in all.iter().enumerate() {
            assert!(
                !all[..index].contains(&uuid),
                "UUID 0x{uuid:032x} must be unique"
            );
        }
    }
}
