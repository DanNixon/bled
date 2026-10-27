//! Effect Manifest API
//!
//! Allows reading JSON manifest for the active/loaded effect.
//!
//! ## Stat manifest
//!
//! 1. Send `EffectManifestControl::Stat` to the `ACTIVE_EFFECT_MANIFEST_CONTROL` characteristic.
//! 2. Receive a CBOR encoded `EffectManifestStatResponse` from the `ACTIVE_EFFECT_MANIFEST_DATA` characteristic.
//!
//! ## Reading manifest
//!
//! 1. Send `EffectManifestControl::Read` to the `ACTIVE_EFFECT_MANIFEST_CONTROL` characteristic.
//! 2. Read from the `ACTIVE_EFFECT_MANIFEST_DATA` characteristic.
//! 3. Repeat from step 1, incrementing the offset as appropriate until the end of the manifest is reached.

use getset::Getters;
use serde::{Deserialize, Serialize};

/// Command sent to the Effect Manifest Control GATT characteristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EffectManifestControl {
    /// Read a chunk from the manifest data at the given offset.
    Read { offset: u32 },

    /// Resets the read state machine.
    Reset,

    /// Stat the manifest data.
    Stat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectManifestStatResponse {
    /// Size of the manifest data in bytes.
    size: u32,
}

impl EffectManifestStatResponse {
    #[must_use]
    pub fn new(size: u32) -> Self {
        Self { size }
    }
}
