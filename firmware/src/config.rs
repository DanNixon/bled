use crate::sdcard::{ReadFileError, SdCardStorage};
use bled_core::{
    ChannelNumber,
    config::{DeviceConfig, Fixture, LinearArray, MAX_CONFIG_JSON_SIZE, PixelLayout, PixelSpan},
};
use defmt::{error, info, warn};
use exfat_slim::blocking::error::ExFatError;

/// File name of the config file on the SD card.
const CONFIG_PATH: &str = "/config.json";

#[inline(never)]
pub(super) async fn boot_time_load(sd: &SdCardStorage) -> DeviceConfig {
    let config = match load_config(sd).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            warn!("Config file not found on SD card, creating default");
            let default = default_config();
            save_config(sd, &default).await;
            default
        }
        Err(_) => {
            error!(
                "Config failed to load or parse; using default config without overwriting SD file"
            );
            default_config()
        }
    };
    info!("Active config: {}", config);
    config
}

#[inline(never)]
async fn load_config(sd: &SdCardStorage) -> Result<Option<DeviceConfig>, ()> {
    let mut bytes = [0u8; MAX_CONFIG_JSON_SIZE];
    match sd.read_file(CONFIG_PATH, &mut bytes).await {
        Ok(len) => match bled_core::io::decode_json::<DeviceConfig>(&bytes[..len]) {
            Ok(config) => Ok(Some(config)),
            Err(e) => {
                warn!("Failed to parse config loaded from SD card: {}", e);
                Err(())
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => Ok(None),
        Err(e) => {
            warn!("Failed to read config file from SD card: {}", e);
            Err(())
        }
    }
}

#[inline(never)]
pub(crate) async fn save_config(sd: &SdCardStorage, config: &DeviceConfig) -> bool {
    info!("Saving config");
    let mut bytes = [0u8; MAX_CONFIG_JSON_SIZE];
    match bled_core::io::encode_json(config, &mut bytes) {
        Ok(len) => sd.write_file(CONFIG_PATH, &bytes[..len]).await.is_ok(),
        Err(e) => {
            warn!("Failed to serialize config: {}", e);
            false
        }
    }
}

#[inline(never)]
fn default_config() -> DeviceConfig {
    let onboard_leds = Fixture::new(
        "onboard".try_into().unwrap(),
        PixelLayout::LinearArray(LinearArray::default()),
        [
            PixelSpan::new(ChannelNumber::new(0), 0, 1),
            PixelSpan::new(ChannelNumber::new(1), 0, 1),
            PixelSpan::new(ChannelNumber::new(2), 0, 1),
            PixelSpan::new(ChannelNumber::new(3), 0, 1),
        ]
        .try_into()
        .unwrap(),
    );

    DeviceConfig {
        name: "bled-default".try_into().unwrap(),
        fixtures: [onboard_leds].into(),
    }
}
