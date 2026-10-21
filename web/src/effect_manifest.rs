//! Active effect manifest transfer protocol and JSON decoding.

use bled_core::{
    api::{EffectManifestControl, EffectManifestStatResponse},
    effect::EffectManifest,
};
use wasm_bindgen::prelude::*;

/// Pure Rust encoding of a manifest control command into CBOR.
pub fn internal_encode_effect_manifest_control(
    command: &EffectManifestControl,
) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_core::io::encode_cbor(command, &mut buffer)
        .map_err(|e| format!("failed to encode EffectManifestControl: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Pure Rust decoding of the manifest stat response from CBOR.
pub fn internal_decode_effect_manifest_stat(
    data: &[u8],
) -> Result<EffectManifestStatResponse, String> {
    bled_core::io::decode_cbor(data)
        .map_err(|e| format!("failed to decode EffectManifestStatResponse: {e}"))
}

/// Pure Rust decoding of the effect's JSON manifest.
pub fn internal_decode_effect_manifest(data: &[u8]) -> Result<EffectManifest, String> {
    bled_core::io::decode_json(data).map_err(|e| format!("failed to decode EffectManifest: {e}"))
}

/// Encodes a manifest stat request into CBOR.
#[wasm_bindgen]
pub fn encode_effect_manifest_stat() -> Result<Vec<u8>, JsError> {
    internal_encode_effect_manifest_control(&EffectManifestControl::Stat)
        .map_err(|e| JsError::new(&e))
}

/// Encodes a manifest chunk request into CBOR.
#[wasm_bindgen]
pub fn encode_effect_manifest_read(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_effect_manifest_control(&EffectManifestControl::Read { offset })
        .map_err(|e| JsError::new(&e))
}

/// Encodes a manifest session reset into CBOR.
#[wasm_bindgen]
pub fn encode_effect_manifest_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_effect_manifest_control(&EffectManifestControl::Reset)
        .map_err(|e| JsError::new(&e))
}

/// Decodes a CBOR manifest stat response into `{ size: number }`.
#[wasm_bindgen]
pub fn decode_effect_manifest_stat(data: &[u8]) -> Result<JsValue, JsError> {
    let stat = internal_decode_effect_manifest_stat(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&stat).map_err(|e| {
        JsError::new(&format!(
            "failed to serialize EffectManifestStatResponse: {e}"
        ))
    })
}

/// Decodes the JSON manifest into its name and ABI buffer descriptors.
#[wasm_bindgen]
pub fn decode_effect_manifest(data: &[u8]) -> Result<JsValue, JsError> {
    let manifest = internal_decode_effect_manifest(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&manifest)
        .map_err(|e| JsError::new(&format!("failed to serialize EffectManifest: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_control_round_trips() {
        for command in [
            EffectManifestControl::Stat,
            EffectManifestControl::Read { offset: 1024 },
            EffectManifestControl::Reset,
        ] {
            let bytes = internal_encode_effect_manifest_control(&command).unwrap();
            let decoded: EffectManifestControl = bled_core::io::decode_cbor(&bytes).unwrap();
            assert_eq!(decoded, command);
        }
    }

    #[test]
    fn manifest_stat_decodes() {
        let response = EffectManifestStatResponse::new(2048);
        let mut buffer = [0; 64];
        let written = bled_core::io::encode_cbor(&response, &mut buffer).unwrap();
        assert_eq!(
            internal_decode_effect_manifest_stat(&buffer[..written]).unwrap(),
            response
        );
        assert!(internal_decode_effect_manifest_stat(b"invalid").is_err());
    }

    #[test]
    fn manifest_json_decodes_buffer_descriptors() {
        let manifest = internal_decode_effect_manifest(
            br#"{"name":"Demo","command_descriptors":[{"name":"Mode","start":0,"end":3}],
                "parameter_descriptors":[{"name":"Red","start":0,"end":7}]}"#,
        )
        .unwrap();
        assert_eq!(manifest.name().as_str(), "Demo");
        assert_eq!(manifest.command_descriptors()[0].name().as_str(), "Mode");
        assert_eq!(*manifest.command_descriptors()[0].end(), 3);
        assert_eq!(manifest.parameter_descriptors()[0].name().as_str(), "Red");
        assert_eq!(*manifest.parameter_descriptors()[0].start(), 0);
        assert_eq!(*manifest.parameter_descriptors()[0].end(), 7);
    }

    #[test]
    fn invalid_manifest_is_rejected() {
        assert!(internal_decode_effect_manifest(b"not JSON").is_err());
        assert!(internal_decode_effect_manifest(br#"{"name":"Missing descriptors"}"#).is_err());
    }
}
