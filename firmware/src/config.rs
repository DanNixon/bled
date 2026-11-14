use crate::sdcard::{ReadFileError, SdCardStorage};
use bled_core::{
    ChannelNumber, DeviceName, PathString, RGB8,
    ble::BlePasskey,
    config::{
        BrightnessLimit, ChannelBrightnessLimits, Fixture, Fixtures, LinearArray,
        MAX_FIXTURES_JSON_SIZE, PixelLayout, PixelSpan,
    },
};
use core::{fmt::Write, str::FromStr};
use defmt::{info, warn};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, once_lock::OnceLock,
};
use exfat_slim::blocking::error::ExFatError;

const DEVICE_NAME_PATH: &str = "/device-name.txt";
const DEVICE_NAME_OLD_PATH: &str = "/device-name.txt_old";

const BLE_PASSKEY_PATH: &str = "/ble-passkey.txt";
const BLE_PASSKEY_OLD_PATH: &str = "/ble-passkey.txt_old";

const FIXTURES_PATH: &str = "/fixtures.json";
const FIXTURES_OLD_PATH: &str = "/fixtures.json_old";

const DEFAULT_EFFECT_PATH: &str = "/default-effect.txt";
const DEFAULT_EFFECT_OLD_PATH: &str = "/default-effect.txt_old";

static DEVICE_NAME: OnceLock<DeviceName> = OnceLock::new();
static DEFAULT_DEVICE_NAME: OnceLock<DeviceName> = OnceLock::new();
static FIXTURES: Mutex<CriticalSectionRawMutex, Fixtures> = Mutex::new(Fixtures::empty());

#[inline(never)]
pub(crate) async fn init(sd: &SdCardStorage) {
    let name = load_device_name(sd).await;
    info!("Device name: {}", name);
    DEVICE_NAME
        .init(name)
        .expect("device name initialized more than once");

    let _ = load_ble_passkey(sd).await;

    let fixtures = load_fixtures(sd).await;
    info!("Active fixtures: {}", fixtures);
    crate::leds::reconfigure(&fixtures).await;
    *FIXTURES.lock().await = fixtures;
}

pub(crate) fn device_name() -> &'static DeviceName {
    DEVICE_NAME
        .try_get()
        .unwrap_or_else(|| DEFAULT_DEVICE_NAME.get_or_init(DeviceName::default))
}

pub(crate) async fn with_fixtures<R>(operation: impl FnOnce(&Fixtures) -> R) -> R {
    let fixtures = FIXTURES.lock().await;
    operation(&fixtures)
}

#[inline(never)]
pub(crate) async fn reload_fixtures(sd: &SdCardStorage) {
    let fixtures = load_fixtures(sd).await;
    info!("Reloaded fixtures: {}", fixtures);
    crate::leds::reconfigure(&fixtures).await;
    *FIXTURES.lock().await = fixtures;
    crate::leds::draw().await;
    crate::effect::reload_active_effect(sd).await;
}

#[inline(never)]
async fn load_device_name(sd: &SdCardStorage) -> DeviceName {
    let mut bytes = [0u8; 64];
    match sd.read_file(DEVICE_NAME_PATH, &mut bytes).await {
        Ok(len) => match parse_device_name(&bytes[..len]) {
            Ok(name) => return name,
            Err(()) => {
                warn!(
                    "Invalid {} on SD card, renaming to {} and writing default",
                    DEVICE_NAME_PATH, DEVICE_NAME_OLD_PATH
                );
                let _ = sd.rename_file(DEVICE_NAME_PATH, DEVICE_NAME_OLD_PATH).await;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            warn!(
                "{} not found on SD card, creating default",
                DEVICE_NAME_PATH
            );
        }
        Err(e) => {
            warn!(
                "Failed to read {} from SD card ({}), renaming to {} and writing default",
                DEVICE_NAME_PATH, e, DEVICE_NAME_OLD_PATH
            );
            let _ = sd.rename_file(DEVICE_NAME_PATH, DEVICE_NAME_OLD_PATH).await;
        }
    }

    let default = DeviceName::default();
    let _ = sd.write_file(DEVICE_NAME_PATH, default.as_bytes()).await;
    default
}

fn parse_device_name(bytes: &[u8]) -> Result<DeviceName, ()> {
    let text = core::str::from_utf8(bytes).map_err(|_| ())?.trim();
    DeviceName::from_str(text).map_err(|_| ())
}

#[inline(never)]
pub(crate) async fn load_ble_passkey(sd: &SdCardStorage) -> BlePasskey {
    let mut bytes = [0u8; 32];
    match sd.read_file(BLE_PASSKEY_PATH, &mut bytes).await {
        Ok(len) => match parse_ble_passkey(&bytes[..len]) {
            Ok(passkey) => return passkey,
            Err(()) => {
                warn!(
                    "Invalid {} on SD card, renaming to {} and writing default",
                    BLE_PASSKEY_PATH, BLE_PASSKEY_OLD_PATH
                );
                let _ = sd.rename_file(BLE_PASSKEY_PATH, BLE_PASSKEY_OLD_PATH).await;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            warn!(
                "{} not found on SD card, creating default",
                BLE_PASSKEY_PATH
            );
        }
        Err(e) => {
            warn!(
                "Failed to read {} from SD card ({}), renaming to {} and writing default",
                BLE_PASSKEY_PATH, e, BLE_PASSKEY_OLD_PATH
            );
            let _ = sd.rename_file(BLE_PASSKEY_PATH, BLE_PASSKEY_OLD_PATH).await;
        }
    }

    let default = BlePasskey::default();
    let mut formatted = bled_core::NameString::new();
    let _ = write!(formatted, "{:06}", *default);
    let _ = sd.write_file(BLE_PASSKEY_PATH, formatted.as_bytes()).await;
    default
}

fn parse_ble_passkey(bytes: &[u8]) -> Result<BlePasskey, ()> {
    let text = core::str::from_utf8(bytes).map_err(|_| ())?.trim();
    BlePasskey::from_str(text).map_err(|_| ())
}

#[inline(never)]
pub(crate) async fn load_default_effect_path(sd: &SdCardStorage) -> Option<PathString> {
    let mut bytes = [0u8; 128];
    match sd.read_file(DEFAULT_EFFECT_PATH, &mut bytes).await {
        Ok(len) => match parse_default_effect_path(&bytes[..len]) {
            Ok(path) => return path,
            Err(()) => {
                warn!(
                    "Invalid {} on SD card, renaming to {} and writing default",
                    DEFAULT_EFFECT_PATH, DEFAULT_EFFECT_OLD_PATH
                );
                let _ = sd
                    .rename_file(DEFAULT_EFFECT_PATH, DEFAULT_EFFECT_OLD_PATH)
                    .await;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            warn!(
                "{} not found on SD card, creating default",
                DEFAULT_EFFECT_PATH
            );
        }
        Err(e) => {
            warn!(
                "Failed to read {} from SD card ({}), renaming to {} and writing default",
                DEFAULT_EFFECT_PATH, e, DEFAULT_EFFECT_OLD_PATH
            );
            let _ = sd
                .rename_file(DEFAULT_EFFECT_PATH, DEFAULT_EFFECT_OLD_PATH)
                .await;
        }
    }

    let _ = sd.write_file(DEFAULT_EFFECT_PATH, b"").await;
    None
}

fn parse_default_effect_path(bytes: &[u8]) -> Result<Option<PathString>, ()> {
    let text = core::str::from_utf8(bytes).map_err(|_| ())?.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        PathString::try_from(text).map(Some).map_err(|_| ())
    }
}

#[inline(never)]
async fn load_fixtures(sd: &SdCardStorage) -> Fixtures {
    let mut bytes = [0u8; MAX_FIXTURES_JSON_SIZE];
    match sd.read_file(FIXTURES_PATH, &mut bytes).await {
        Ok(len) => match parse_and_validate_fixtures(&bytes[..len]) {
            Ok(fixtures) => return fixtures,
            Err(()) => {
                warn!(
                    "Invalid {} on SD card, renaming to {} and writing default",
                    FIXTURES_PATH, FIXTURES_OLD_PATH
                );
                let _ = sd.rename_file(FIXTURES_PATH, FIXTURES_OLD_PATH).await;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            warn!("{} not found on SD card, creating default", FIXTURES_PATH);
        }
        Err(e) => {
            warn!(
                "Failed to read {} from SD card ({}), renaming to {} and writing default",
                FIXTURES_PATH, e, FIXTURES_OLD_PATH
            );
            let _ = sd.rename_file(FIXTURES_PATH, FIXTURES_OLD_PATH).await;
        }
    }

    let default = default_fixtures();
    save_fixtures(sd, &default, &mut bytes).await;
    default
}

fn parse_and_validate_fixtures(bytes: &[u8]) -> Result<Fixtures, ()> {
    let fixtures = bled_core::io::decode_json::<Fixtures>(bytes).map_err(|e| {
        warn!("Failed to parse fixtures from SD card: {}", e);
    })?;

    fixtures
        .validate::<{ crate::leds::CHANNEL_COUNT }>()
        .map_err(|e| {
            warn!("Invalid fixtures loaded from SD card: {}", e);
        })?;

    let required_bytes = fixtures
        .total_length::<{ crate::leds::CHANNEL_COUNT }>()
        .checked_mul(core::mem::size_of::<RGB8>())
        .ok_or_else(|| {
            warn!("Fixtures total pixel byte length overflows");
        })?;

    if required_bytes > crate::leds::LED_MEMORY {
        warn!(
            "Fixtures require {} bytes, exceeding LED buffer capacity of {} bytes",
            required_bytes,
            crate::leds::LED_MEMORY
        );
        return Err(());
    }

    if ChannelBrightnessLimits::<{ crate::leds::CHANNEL_COUNT }>::from_fixtures(&fixtures).is_none()
    {
        warn!("Failed to build brightness limits from fixtures");
        return Err(());
    }

    Ok(fixtures)
}

#[inline(never)]
async fn save_fixtures(
    sd: &SdCardStorage,
    fixtures: &Fixtures,
    bytes: &mut [u8; MAX_FIXTURES_JSON_SIZE],
) {
    info!("Saving default fixtures");
    match bled_core::io::encode_json(fixtures, bytes) {
        Ok(len) => {
            let _ = sd.write_file(FIXTURES_PATH, &bytes[..len]).await;
        }
        Err(e) => {
            warn!("Failed to serialize fixtures: {}", e);
        }
    }
}

#[inline(never)]
fn default_fixtures() -> Fixtures {
    let onboard_leds = Fixture::new(
        "onboard".try_into().unwrap(),
        PixelLayout::LinearArray(LinearArray::default()),
        [
            PixelSpan::new(
                "led".try_into().unwrap(),
                ChannelNumber::new(0),
                0,
                1,
                BrightnessLimit::default(),
            ),
            PixelSpan::new(
                "led".try_into().unwrap(),
                ChannelNumber::new(1),
                0,
                1,
                BrightnessLimit::default(),
            ),
            PixelSpan::new(
                "led".try_into().unwrap(),
                ChannelNumber::new(2),
                0,
                1,
                BrightnessLimit::default(),
            ),
            PixelSpan::new(
                "led".try_into().unwrap(),
                ChannelNumber::new(3),
                0,
                1,
                BrightnessLimit::default(),
            ),
        ]
        .into(),
    );

    Fixtures::new([onboard_leds].into())
}
