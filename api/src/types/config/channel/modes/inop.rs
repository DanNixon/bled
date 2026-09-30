use crate::ChannelPartOps;
use getset::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Getters, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct Inop {
    length: u32,
}

impl Inop {
    #[must_use]
    pub fn new(length: u32) -> Self {
        Self { length }
    }
}

impl ChannelPartOps for Inop {
    fn len(&self) -> u32 {
        self.length
    }
}
