//! Device information and configuration decoders.

use wasm_bindgen::prelude::*;

/// Pure Rust decoding of `DeviceInfo` from CBOR.
pub fn internal_decode_device_info(data: &[u8]) -> Result<bled_core::api::DeviceInfo, String> {
    bled_core::io::decode_cbor(data).map_err(|e| format!("failed to decode DeviceInfo: {e}"))
}

/// Pure Rust decoding of `DeviceConfig` from CBOR or JSON.
pub fn internal_decode_device_config(
    data: &[u8],
) -> Result<bled_core::config::DeviceConfig, String> {
    match bled_core::io::decode_cbor(data) {
        Ok(c) => Ok(c),
        Err(_) => bled_core::io::decode_json(data)
            .map_err(|e| format!("failed to decode DeviceConfig: {e}")),
    }
}

/// Decodes the CBOR payload read from the Device Info characteristic into a JS object.
#[wasm_bindgen]
pub fn decode_device_info(data: &[u8]) -> Result<JsValue, JsError> {
    let info = internal_decode_device_info(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&info)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceInfo: {e}")))
}

/// Decodes the payload for DeviceConfig (CBOR or JSON) into a JS object.
#[wasm_bindgen]
pub fn decode_device_config(data: &[u8]) -> Result<JsValue, JsError> {
    let config = internal_decode_device_config(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&config)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceConfig: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_core::{
        ChannelNumber,
        api::{BootReason, DeviceInfo},
        config::{DeviceConfig, Fixture, LinearArray, PixelLayout, PixelSpan},
    };

    #[test]
    fn test_device_info_cbor_decoding() {
        let info = DeviceInfo::new(
            "rev1234".try_into().unwrap(),
            BootReason::Normal,
            12_345,
            16 * 1024,
            4,
        );
        let mut buffer = [0u8; 128];
        let len = bled_core::io::encode_cbor(&info, &mut buffer).unwrap();

        let decoded = internal_decode_device_info(&buffer[..len]).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn test_device_config_json_decoding() {
        let json = r#"{
            "name": "test-dev",
            "fixtures": [
                {
                    "name": "onboard",
                    "layout": { "linear_array": {} },
                    "spans": [
                        { "channel": 0, "start": 0, "length": 1 },
                        { "channel": 1, "start": 0, "length": 1 }
                    ]
                }
            ]
        }"#;

        let decoded = internal_decode_device_config(json.as_bytes()).unwrap();
        assert_eq!(decoded.name().as_str(), "test-dev");
        assert_eq!(decoded.fixtures().len(), 1);
        assert_eq!(decoded.fixtures()[0].name().as_str(), "onboard");
        assert_eq!(decoded.fixtures()[0].spans().len(), 2);
    }

    #[test]
    fn test_device_config_cbor_decoding() {
        let fixture = Fixture::new(
            "test-fixture".try_into().unwrap(),
            PixelLayout::LinearArray(LinearArray::default()),
            [PixelSpan::new(ChannelNumber::new(0), 0, 4)]
                .try_into()
                .unwrap(),
        );
        let config = DeviceConfig {
            name: "dev-cbor".try_into().unwrap(),
            fixtures: [fixture].into(),
        };

        let mut buffer = [0u8; 512];
        let len = bled_core::io::encode_cbor(&config, &mut buffer).unwrap();

        let decoded = internal_decode_device_config(&buffer[..len]).unwrap();
        assert_eq!(decoded, config);
    }
}
