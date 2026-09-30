mod config;
mod modes;
mod section;

pub use config::*;
pub use modes::*;
pub use section::*;

pub trait ChannelPartOps {
    fn len(&self) -> u32;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
