use super::TextConfigFile;
use crate::sdcard::SdCardStorage;
use bled_core::PathString;
use core::{
    fmt::{self, Display},
    str::FromStr,
};

#[inline(never)]
pub(crate) async fn load_default_effect_path(sd: &SdCardStorage) -> Option<PathString> {
    let DefaultEffect(path) = super::load(sd).await;
    path
}

#[derive(Default)]
struct DefaultEffect(Option<PathString>);

impl FromStr for DefaultEffect {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Self(None))
        } else {
            PathString::try_from(s)
                .map(|p| Self(Some(p)))
                .map_err(|_| ())
        }
    }
}

impl Display for DefaultEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.0 {
            f.write_str(path.as_str())
        } else {
            Ok(())
        }
    }
}

impl TextConfigFile<128> for DefaultEffect {
    const PATH: &'static str = "/default-effect.txt";
    const OLD_PATH: &'static str = "/default-effect-OLD.txt";
}
