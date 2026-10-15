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
