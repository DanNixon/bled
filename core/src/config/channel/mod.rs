mod config;
mod layouts;
mod section;

pub use config::*;
pub use layouts::*;
pub use section::*;

pub trait ChannelPartOps {
    fn len(&self) -> u32;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
