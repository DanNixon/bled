//! Active effect manifest JSON decoder.

use bled_core::effect::EffectManifest;
use wasm_bindgen::prelude::*;

/// Pure Rust decoding of the effect's JSON manifest.
pub fn internal_decode_effect_manifest(data: &[u8]) -> Result<EffectManifest, String> {
    bled_core::io::decode_json(data).map_err(|e| format!("failed to decode EffectManifest: {e}"))
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
