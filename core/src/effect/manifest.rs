use crate::{NameString, PathString};
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectManifest {
    /// Human-readable effect name.
    name: NameString,

    /// Path to the WASM module.
    wasm_module: PathString,
}

/// Maximum allowable size of the effect manifest file in bytes when encoded as JSON.
pub const MAX_EFFECT_MANIFEST_JSON_SIZE: usize = 1024 * 2; // TODO: calculate
