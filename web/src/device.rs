//! Device configuration decoder.

use wasm_bindgen::prelude::*;

/// Pure Rust decoding of `DeviceConfig` from Postcard or JSON.
pub fn internal_decode_device_config(
    data: &[u8],
) -> Result<bled_core::config::DeviceConfig, String> {
    match bled_core::io::decode_postcard(data) {
        Ok(c) => Ok(c),
        Err(_) => bled_core::io::decode_json(data)
            .map_err(|e| format!("failed to decode DeviceConfig: {e}")),
    }
}

/// Decodes the Postcard payload returned by the configuration RPC into a JS object.
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
        ChannelNumber, RGB8,
        ble::BlePasskey,
        config::{
            BrightnessLimit, BrightnessLimitMode, DeviceConfig, Fixture, Fixtures, LinearArray,
            PixelLayout, PixelSpan,
        },
    };

    #[test]
    fn test_device_config_json_decoding() {
        let json = r#"{
            "name": "test-dev",
            "ble_passkey": 123456,
            "fixtures": [
                {
                    "name": "onboard",
                    "layout": { "linear_array": {} },
                    "spans": [
                        { "name": "a", "channel": 0, "start": 0, "length": 1 },
                        {
                            "name": "b",
                            "channel": 1,
                            "start": 0,
                            "length": 1,
                            "limit": {
                                "mode": "rescale",
                                "limit": { "r": 63, "g": 127, "b": 255 }
                            }
                        }
                    ]
                }
            ],
            "defaultEffect": null
        }"#;

        let decoded = internal_decode_device_config(json.as_bytes()).unwrap();
        assert_eq!(decoded.name().as_str(), "test-dev");
        assert_eq!(*decoded.ble_passkey().as_ref(), 123_456);
        assert_eq!(decoded.fixtures().as_slice().len(), 1);
        assert_eq!(decoded.fixtures().as_slice()[0].name().as_str(), "onboard");
        assert_eq!(decoded.fixtures().as_slice()[0].spans().len(), 2);
        assert_eq!(
            *decoded.fixtures().as_slice()[0].spans()[0].brightness_limit(),
            BrightnessLimit::default()
        );
        assert_eq!(
            *decoded.fixtures().as_slice()[0].spans()[1].brightness_limit(),
            BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(63, 127, 255))
        );
        assert!(decoded.default_effect().is_none());
    }

    #[test]
    fn test_device_config_postcard_decoding() {
        let fixture = Fixture::new(
            "test-fixture".try_into().unwrap(),
            PixelLayout::LinearArray(LinearArray::default()),
            [PixelSpan::new(
                "a".try_into().unwrap(),
                ChannelNumber::new(0),
                0,
                4,
                BrightnessLimit::default(),
            )]
            .try_into()
            .unwrap(),
        );
        let config = DeviceConfig {
            name: "dev-postcard".try_into().unwrap(),
            ble_passkey: BlePasskey::try_new(654_321).unwrap(),
            fixtures: Fixtures::new([fixture].into()),
            default_effect: None,
        };

        let mut buffer = [0u8; 512];
        let len = bled_core::io::encode_postcard(&config, &mut buffer).unwrap();

        let decoded = internal_decode_device_config(&buffer[..len]).unwrap();
        assert_eq!(decoded, config);
    }
}
