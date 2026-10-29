//! Public protocol constants for the web client.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn serial_service_uuid() -> String {
    bled_core::ble::SERIAL_SERVICE_UUID.to_string()
}

#[wasm_bindgen]
pub fn serial_rx_uuid() -> String {
    bled_core::ble::SERIAL_RX_UUID.to_string()
}

#[wasm_bindgen]
pub fn serial_tx_uuid() -> String {
    bled_core::ble::SERIAL_TX_UUID.to_string()
}

#[wasm_bindgen]
pub fn max_chunk_size() -> usize {
    bled_core::api::MAX_CHUNK_SIZE
}

#[wasm_bindgen]
pub fn max_path_len() -> usize {
    bled_core::api::MAX_PATH_LEN
}

#[wasm_bindgen]
pub fn max_name_len() -> usize {
    bled_core::config::MAX_NAME_LEN
}

#[wasm_bindgen]
pub fn max_fixture_count() -> usize {
    bled_core::config::MAX_FIXTURE_COUNT
}

#[wasm_bindgen]
pub fn max_span_per_fixture_count() -> usize {
    bled_core::config::MAX_SPAN_PER_FIXTURE_COUNT
}

#[wasm_bindgen]
pub fn max_config_json_size() -> usize {
    bled_core::config::MAX_CONFIG_JSON_SIZE
}

#[wasm_bindgen]
pub fn max_config_cbor_size() -> usize {
    bled_core::config::MAX_CONFIG_CBOR_SIZE
}

#[wasm_bindgen]
pub fn git_revision() -> String {
    git_version::git_version!().to_string()
}
