use crate::{NameString, PathString};
use getset::Getters;
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// A preset for an effect, the supplied command and parameters to set on load
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectPreset {
    /// Name of the preset
    name: NameString,

    /// WASM module for the effect
    module: PathString,

    /// Command buffer to set on load
    #[serde(default)]
    command: Vec<u8, 512>,

    /// Parameters to set on load
    #[serde(default)]
    parameters: Vec<u8, 512>,
}

/// Maximum allowable size of an effect preset in bytes when encoded as JSON.
pub const MAX_EFFECT_PRESET_JSON_SIZE: usize = 1024 * 4; // TODO: calculate

#[cfg(test)]
mod tests {
    use super::EffectPreset;

    #[test]
    fn omitted_fields_default_to_empty_values() {
        let preset = crate::io::decode_json::<EffectPreset>(
            br#"{"name":"demo","module":"demo.wasm","parameters":[255,0,255]}"#,
        )
        .unwrap();

        assert_eq!(preset.name(), "demo");
        assert_eq!(preset.module(), "demo.wasm");
        assert!(preset.command().is_empty());
        assert_eq!(preset.parameters().as_slice(), &[255, 0, 255]);
    }
}
