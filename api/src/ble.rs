//! GATT service and characteristic constants for BLE.

use uuid::{Uuid, uuid};

pub const MAX_ATTRIBUTE_VALUE_LEN: usize = 512;

/// LED control GATT service UUID
pub const SERVICE_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000101");

/// Device Info GATT characteristic UUID (read-only).
pub const DEVICE_INFO_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000120");
/// Reset device GATT characteristic UUID (write-only).
pub const RESET_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000121");

/// Config control GATT characteristic UUID (write-only).
pub const CONFIG_CONTROL_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000122");
/// Config data GATT characteristic UUID (read).
pub const CONFIG_DATA_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000123");

/// File IO control GATT characteristic UUID (write-only).
pub const FILE_IO_CONTROL_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000130");
/// File IO data GATT characteristic UUID (read/write).
pub const FILE_IO_DATA_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000131");

/// Render staged channel GATT characteristic UUID (write-only).
pub const COMMIT_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000140");

/// LED buffer control GATT characteristic UUID (write-only).
pub const LED_BUFFER_CONTROL_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000141");

/// LED buffer data GATT characteristic UUID (read/write).
pub const LED_BUFFER_DATA_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000142");

/// Packed pixel range GATT characteristic UUID (write-only).
pub const LED_RANGE_UUID: Uuid = uuid!("408813df-5dd4-1f87-ec11-cdb000000143");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_constants_are_valid_and_unique() {
        let all = [
            SERVICE_UUID,
            DEVICE_INFO_UUID,
            RESET_UUID,
            FILE_IO_CONTROL_UUID,
            FILE_IO_DATA_UUID,
            COMMIT_UUID,
            LED_BUFFER_CONTROL_UUID,
            LED_BUFFER_DATA_UUID,
            LED_RANGE_UUID,
        ];

        for (index, &uuid) in all.iter().enumerate() {
            assert!(
                !all[..index].contains(&uuid),
                "UUID 0x{uuid:032x} must be unique"
            );
        }
    }
}
