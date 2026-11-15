use super::ConfigFile;
use crate::sdcard::SdCardStorage;
use bled_core::{
    ChannelNumber, RGB8,
    config::{
        BrightnessLimit, ChannelBrightnessLimits, Fixture, Fixtures, LinearArray,
        MAX_FIXTURES_JSON_SIZE, PixelLayout, PixelSpan,
    },
};
use defmt::{info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

static FIXTURES: Mutex<CriticalSectionRawMutex, Fixtures> = Mutex::new(Fixtures::empty());
static FIXTURES_BUF: Mutex<CriticalSectionRawMutex, [u8; MAX_FIXTURES_JSON_SIZE]> =
    Mutex::new([0u8; MAX_FIXTURES_JSON_SIZE]);

#[inline(never)]
pub(crate) async fn reload_fixtures(sd: &SdCardStorage) {
    let fixtures: Fixtures = {
        let mut buf = FIXTURES_BUF.lock().await;
        super::load_with_buf(sd, &mut buf).await
    };
    info!("Reloaded fixtures: {}", fixtures);
    let active_effect = crate::effect::unload_active_effect().await;
    crate::leds::reconfigure(&fixtures).await;
    *FIXTURES.lock().await = fixtures;
    if let Some(path) = active_effect
        && crate::effect::activate_effect(sd, Some(path))
            .await
            .is_err()
    {
        warn!("Failed to reload active effect after fixture reload");
    }
}

pub(crate) async fn with_fixtures<R>(operation: impl FnOnce(&Fixtures) -> R) -> R {
    let fixtures = FIXTURES.lock().await;
    operation(&fixtures)
}

impl ConfigFile<MAX_FIXTURES_JSON_SIZE> for Fixtures {
    const PATH: &'static str = "/fixtures.json";
    const OLD_PATH: &'static str = "/fixtures-OLD.json";

    #[inline(never)]
    fn default_value() -> Self {
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

    fn decode(bytes: &[u8]) -> Result<Self, ()> {
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

        if ChannelBrightnessLimits::<{ crate::leds::CHANNEL_COUNT }>::from_fixtures(&fixtures)
            .is_none()
        {
            warn!("Failed to build brightness limits from fixtures");
            return Err(());
        }

        Ok(fixtures)
    }

    fn encode(&self, buf: &mut [u8; MAX_FIXTURES_JSON_SIZE]) -> Result<usize, ()> {
        bled_core::io::encode_json(self, buf).map_err(|e| {
            warn!("Failed to serialize fixtures: {}", e);
        })
    }
}
