//! Device information and configuration decoders.

use wasm_bindgen::prelude::*;

/// Decodes the CBOR payload read from the Device Info characteristic into a JS object.
#[wasm_bindgen]
pub fn decode_device_info(data: &[u8]) -> Result<JsValue, JsError> {
    let info: bled_core::DeviceInfo = bled_core::io::decode_cbor(data)
        .map_err(|e| JsError::new(&format!("failed to decode DeviceInfo: {e}")))?;
    serde_wasm_bindgen::to_value(&info)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceInfo: {e}")))
}

/// Decodes the payload for DeviceConfig (CBOR or JSON) into a JS object.
#[wasm_bindgen]
pub fn decode_device_config(data: &[u8]) -> Result<JsValue, JsError> {
    let config: bled_core::DeviceConfig = match bled_core::io::decode_cbor(data) {
        Ok(c) => c,
        Err(_) => bled_core::io::decode_json(data)
            .map_err(|e| JsError::new(&format!("failed to decode DeviceConfig: {e}")))?,
    };
    serde_wasm_bindgen::to_value(&config)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceConfig: {e}")))
}
