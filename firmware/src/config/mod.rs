#[cfg(feature = "_wireless")]
mod ble_passkey;
mod default_effect;
mod device_name;
mod fixtures;

#[cfg(feature = "_wireless")]
pub(crate) use ble_passkey::load_ble_passkey;
pub(crate) use default_effect::load_default_effect_path;
pub(crate) use device_name::device_name;
pub(crate) use fixtures::{reload_fixtures, with_fixtures};

use crate::sdcard::{ReadFileError, SdCardStorage};
use core::{
    fmt::{self, Display, Write},
    str::FromStr,
};
use defmt::warn;
use exfat_slim::blocking::error::ExFatError;

#[inline(never)]
pub(crate) async fn init(sd: &SdCardStorage) {
    device_name::init(sd).await;
}

#[inline(never)]
async fn load<T: ConfigFile<N>, const N: usize>(sd: &SdCardStorage) -> T {
    let mut bytes = [0u8; N];
    load_with_buf(sd, &mut bytes).await
}

#[inline(never)]
async fn load_with_buf<T: ConfigFile<N>, const N: usize>(
    sd: &SdCardStorage,
    bytes: &mut [u8; N],
) -> T {
    match sd.read_file(T::PATH, bytes).await {
        Ok(len) => match T::decode(&bytes[..len]) {
            Ok(value) => return value,
            Err(()) => {
                warn!(
                    "Invalid {} on SD card, renaming to {} and writing default",
                    T::PATH,
                    T::OLD_PATH
                );
                let _ = sd.rename_file(T::PATH, T::OLD_PATH).await;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            warn!("{} not found on SD card, creating default", T::PATH);
        }
        Err(e) => {
            warn!(
                "Failed to read {} from SD card ({}), renaming to {} and writing default",
                T::PATH,
                e,
                T::OLD_PATH
            );
            let _ = sd.rename_file(T::PATH, T::OLD_PATH).await;
        }
    }

    let default = T::default_value();
    if let Ok(len) = default.encode(bytes) {
        let _ = sd.write_file(T::PATH, &bytes[..len]).await;
    }
    default
}

trait ConfigFile<const MAX_SIZE: usize>: Sized {
    const PATH: &'static str;
    const OLD_PATH: &'static str;

    fn default_value() -> Self;
    fn decode(bytes: &[u8]) -> Result<Self, ()>;
    fn encode(&self, buf: &mut [u8; MAX_SIZE]) -> Result<usize, ()>;
}

trait TextConfigFile<const MAX_SIZE: usize>: Default + FromStr + Display {
    const PATH: &'static str;
    const OLD_PATH: &'static str;
}

impl<T: TextConfigFile<MAX_SIZE>, const MAX_SIZE: usize> ConfigFile<MAX_SIZE> for T {
    const PATH: &'static str = T::PATH;
    const OLD_PATH: &'static str = T::OLD_PATH;

    fn default_value() -> Self {
        Self::default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ()> {
        let text = core::str::from_utf8(bytes).map_err(|_| ())?.trim();
        Self::from_str(text).map_err(|_| ())
    }

    fn encode(&self, buf: &mut [u8; MAX_SIZE]) -> Result<usize, ()> {
        let mut cursor = SliceWriter::new(buf);
        write!(cursor, "{self}").map_err(|_| ())?;
        Ok(cursor.written())
    }
}

struct SliceWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> SliceWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn written(&self) -> usize {
        self.pos
    }
}

impl Write for SliceWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let end = self.pos.checked_add(bytes.len()).ok_or(fmt::Error)?;
        let target = self.buf.get_mut(self.pos..end).ok_or(fmt::Error)?;
        target.copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
}
