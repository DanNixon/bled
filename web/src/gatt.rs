//! GATT UUID constants and device limits.

use wasm_bindgen::prelude::*;

/// LED control GATT service UUID.
#[wasm_bindgen]
pub fn service_uuid() -> String {
    bled_core::ble::SERVICE_UUID.to_string()
}

/// Device Info GATT characteristic UUID.
#[wasm_bindgen]
pub fn device_info_uuid() -> String {
    bled_core::ble::DEVICE_INFO_UUID.to_string()
}

/// Reset device GATT characteristic UUID.
#[wasm_bindgen]
pub fn reset_uuid() -> String {
    bled_core::ble::RESET_UUID.to_string()
}

/// Config control GATT characteristic UUID.
#[wasm_bindgen]
pub fn config_control_uuid() -> String {
    bled_core::ble::CONFIG_CONTROL_UUID.to_string()
}

/// Config data GATT characteristic UUID.
#[wasm_bindgen]
pub fn config_data_uuid() -> String {
    bled_core::ble::CONFIG_DATA_UUID.to_string()
}

/// Render staged channel GATT characteristic UUID.
#[wasm_bindgen]
pub fn commit_uuid() -> String {
    bled_core::ble::COMMIT_UUID.to_string()
}

/// LED buffer control GATT characteristic UUID.
#[wasm_bindgen]
pub fn led_buffer_control_uuid() -> String {
    bled_core::ble::LED_BUFFER_CONTROL_UUID.to_string()
}

/// LED buffer data GATT characteristic UUID.
#[wasm_bindgen]
pub fn led_buffer_data_uuid() -> String {
    bled_core::ble::LED_BUFFER_DATA_UUID.to_string()
}

/// File IO control GATT characteristic UUID.
#[wasm_bindgen]
pub fn file_io_control_uuid() -> String {
    bled_core::ble::FILE_IO_CONTROL_UUID.to_string()
}

/// File IO data GATT characteristic UUID.
#[wasm_bindgen]
pub fn file_io_data_uuid() -> String {
    bled_core::ble::FILE_IO_DATA_UUID.to_string()
}

/// LED buffer data GATT characteristic UUID (backward compatibility alias).
#[wasm_bindgen]
pub fn led_data_uuid() -> String {
    bled_core::ble::LED_BUFFER_DATA_UUID.to_string()
}

/// Maximum number of LED channels addressable on one device.
#[wasm_bindgen]
pub fn max_channel_count() -> usize {
    bled_core::MAX_CHANNEL_COUNT
}

/// Maximum length of an attribute value in bytes.
#[wasm_bindgen]
pub fn max_attribute_value_len() -> usize {
    bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN
}

/// Maximum path length in bytes for File IO operations.
#[wasm_bindgen]
pub fn max_path_len() -> usize {
    bled_core::MAX_PATH_LEN
}

/// Git revision of the build.
#[wasm_bindgen]
pub fn git_revision() -> String {
    git_version::git_version!().to_string()
}
