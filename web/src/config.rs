//! Device configuration transfer protocol and command encoding.

use wasm_bindgen::prelude::*;

/// Pure Rust encoding of a `ConfigControl` command into CBOR.
pub fn internal_encode_config_control(cmd: &bled_api::ConfigControl) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_api::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode Config control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Pure Rust encoding of a Config Stat command into CBOR.
pub fn internal_encode_config_stat() -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Stat)
}

/// Pure Rust encoding of a Config Read command at `offset` into CBOR.
pub fn internal_encode_config_read(offset: u32) -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Read { offset })
}

/// Pure Rust encoding of a Config Reset command into CBOR.
pub fn internal_encode_config_reset() -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Reset)
}

/// Pure Rust decoding of a CBOR payload into `ConfigStatResponse`.
pub fn internal_decode_config_stat(data: &[u8]) -> Result<bled_api::ConfigStatResponse, String> {
    bled_api::io::decode_cbor(data).map_err(|e| format!("failed to decode ConfigStatResponse: {e}"))
}

/// Encodes a `ConfigControl::Stat` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_stat() -> Result<Vec<u8>, JsError> {
    internal_encode_config_stat().map_err(|e| JsError::new(&e))
}

/// Encodes a `ConfigControl::Read { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_read(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_config_read(offset).map_err(|e| JsError::new(&e))
}

/// Encodes a `ConfigControl::Reset` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_config_reset().map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload from `CONFIG_DATA_UUID` for a stat request into `{ size: number }`.
#[wasm_bindgen]
pub fn decode_config_stat(data: &[u8]) -> Result<JsValue, JsError> {
    let stat = internal_decode_config_stat(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&stat)
        .map_err(|e| JsError::new(&format!("failed to serialize ConfigStatResponse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_control() {
        // Stat
        let stat_bytes = internal_encode_config_stat().unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&stat_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Stat);

        // Read
        let read_bytes = internal_encode_config_read(128).unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&read_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Read { offset: 128 });

        // Reset
        let reset_bytes = internal_encode_config_reset().unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Reset);

        // Decode Stat
        let stat_response = bled_api::ConfigStatResponse::new(512);
        let mut buffer = [0u8; 64];
        let written = bled_api::io::encode_cbor(&stat_response, &mut buffer).unwrap();
        let decoded_stat = internal_decode_config_stat(&buffer[..written]).unwrap();
        assert_eq!(*decoded_stat.size(), 512);
    }
}
