use crate::config::ChannelSpan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SinglePixel {}

impl ChannelSpan for SinglePixel {
    fn len(&self) -> u32 {
        1
    }
}
