//! UUIDs of the emulated serial GATT service.
//!
//! The emulated serial service provides a bidirectional byte stream over BLE,
//! the device API RPC protocol runs over this stream.

use uuid::{Uuid, uuid};

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
