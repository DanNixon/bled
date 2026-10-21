use crate::NameString;
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

/// Description of an effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct EffectManifest {
    /// Human-readable effect name.
    name: NameString,

    /// Descriptors for the commands supported by this effect.
    command_descriptors: Vec<AbiDescriptor, 16>,

    /// Descriptors for the parameters supported by this effect.
    parameter_descriptors: Vec<AbiDescriptor, 32>,
}

/// Describes a command or parameter for a specific effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct AbiDescriptor {
    /// Human-readable field name.
    name: String<64>,

    /// Bit index of the start of the field.
    start: u32,

    /// Bit index of the end of the field (inclusive).
    end: u32,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<EffectManifest>(), 3880);
        assert_eq!(core::mem::size_of::<AbiDescriptor>(), 80);
    }
}
