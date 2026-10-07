use crate::effects::{EffectKindConfig, TargetSectionConfig};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectConfig {
    /// Target section to apply the effect to
    target: TargetSectionConfig,

    /// Effect logic configuration
    effect: EffectKindConfig,
}

/// Maximum allowable size of a single effect config file in bytes when encoded as JSON.
pub const MAX_EFFECT_CONFIG_JSON_SIZE: usize = 1024 * 2; // TODO: calculate

#[cfg(test)]
mod tests {
    use super::*;
    use rgb::RGB8;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<EffectConfig>(), 56);
    }

    #[test]
    fn json_deserialization() {
        let json = br#"{
            "target": {
                "channel": "a",
                "section": "onboard"
            },
            "effect": {
                "fixed_color": {
                    "color": { "r": 255, "g": 127, "b": 63 }
                }
            }
        }"#;

        let config: EffectConfig = crate::io::decode_json(json).unwrap();

        let targets = config.target();
        assert_eq!(targets.channel().as_str(), "a");
        assert_eq!(targets.section().as_str(), "onboard");
        let EffectKindConfig::FixedColor(effect) = config.effect();
        assert_eq!(*effect.color(), RGB8::new(255, 127, 63));
    }
}
