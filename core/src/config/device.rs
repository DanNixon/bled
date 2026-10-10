use crate::{
    api::MAX_CHANNEL_COUNT,
    config::{Fixture, MAX_FIXTURE_COUNT, MAX_NAME_LEN},
};
use getset::Getters;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: String<{ MAX_NAME_LEN }>,

    pub fixtures: Vec<Fixture, { MAX_FIXTURE_COUNT }>,
}

impl DeviceConfig {
    pub fn channel_lengths(&self) -> Vec<usize, MAX_CHANNEL_COUNT> {
        todo!()
    }

    pub fn validate(&self) -> Result<(), ()> {
        todo!()
    }
}
