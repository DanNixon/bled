use crate::{
    config::MAX_NAME_LEN,
    effects::{EffectKindConfig, TargetSectionConfig},
};
use getset::Getters;
use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectConfig {
    name: String<{ MAX_NAME_LEN }>,
    target: TargetSectionConfig,
    effect: EffectKindConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgb::RGB8;

    #[test]
    fn json_deserialization() {
        let json = br#"{
            "name": "warm white",
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

        assert_eq!(config.name().as_str(), "warm white");
        let targets = config.target();
        assert_eq!(targets.channel().as_str(), "a");
        assert_eq!(targets.section().as_str(), "onboard");
        let EffectKindConfig::FixedColor(effect) = config.effect();
        assert_eq!(*effect.color(), RGB8::new(255, 127, 63));
    }
}
