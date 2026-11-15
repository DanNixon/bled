use super::ConfigFile;
use crate::sdcard::SdCardStorage;
use bled_core::effect::{EffectPlaylist, MAX_PLAYLIST_JSON_SIZE};
use defmt::warn;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

pub(crate) const DEFAULT_PLAYLIST_PATH: &str = "/playlist.json";

static PLAYLIST_BUF: Mutex<CriticalSectionRawMutex, [u8; MAX_PLAYLIST_JSON_SIZE]> =
    Mutex::new([0u8; MAX_PLAYLIST_JSON_SIZE]);

#[inline(never)]
pub(crate) async fn load_default_playlist(sd: &SdCardStorage) -> EffectPlaylist {
    let mut buf = PLAYLIST_BUF.lock().await;
    super::load_with_buf(sd, &mut buf).await
}

#[inline(never)]
pub(crate) async fn load_playlist_from_path(
    sd: &SdCardStorage,
    path: &str,
) -> Result<EffectPlaylist, ()> {
    let mut buf = PLAYLIST_BUF.lock().await;
    let len = sd.read_file(path, &mut *buf).await.map_err(|e| {
        warn!("Failed to read playlist '{}' from SD card: {}", path, e);
    })?;
    EffectPlaylist::decode(&buf[..len])
}

impl ConfigFile<MAX_PLAYLIST_JSON_SIZE> for EffectPlaylist {
    const PATH: &'static str = DEFAULT_PLAYLIST_PATH;
    const OLD_PATH: &'static str = "/playlist-OLD.json";

    fn default_value() -> Self {
        Self::default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ()> {
        bled_core::io::decode_json::<EffectPlaylist>(bytes).map_err(|e| {
            warn!("Failed to parse playlist from SD card: {}", e);
        })
    }

    fn encode(&self, buf: &mut [u8; MAX_PLAYLIST_JSON_SIZE]) -> Result<usize, ()> {
        bled_core::io::encode_json(self, buf).map_err(|e| {
            warn!("Failed to serialize playlist: {}", e);
        })
    }
}
