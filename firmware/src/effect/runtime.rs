use super::{EffectInstance, StepResult};
use alloc::vec::Vec;
use bled_core::{RGB8, effect::EffectManifest};
use defmt::{error, info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Instant, Timer};

const MAX_EFFECT_MANIFEST_BYTES: usize = 1024;
const MAX_WASM_MODULE_BYTES: usize = 64 * 1024;

static ACTIVE_EFFECT: Mutex<CriticalSectionRawMutex, Option<EffectInstance>> = Mutex::new(None);

/// Loads the effect selected by the device configuration, replacing any active effect.
pub(crate) async fn load_active_effect(sd: &crate::sdcard::SdCardStorage) {
    unload_active_effect().await;

    let Some(manifest_path) = crate::config::get().default_effect.as_ref() else {
        info!("No default effect configured");
        return;
    };

    let mut manifest_bytes = [0; MAX_EFFECT_MANIFEST_BYTES];
    let manifest_len = match sd
        .read_file(manifest_path.as_str(), &mut manifest_bytes)
        .await
    {
        Ok(len) => len,
        Err(e) => {
            warn!("Failed to read effect manifest: {}", e);
            return;
        }
    };
    let manifest =
        match bled_core::io::decode_json::<EffectManifest>(&manifest_bytes[..manifest_len]) {
            Ok(manifest) => manifest,
            Err(e) => {
                warn!("Failed to parse effect manifest: {}", e);
                return;
            }
        };

    let module_path = manifest.wasm_module();
    let module_size = match sd.stat_file(module_path.as_str()).await {
        Ok(size) => match usize::try_from(size) {
            Ok(size) if size <= MAX_WASM_MODULE_BYTES => size,
            _ => {
                warn!(
                    "Effect Wasm module exceeds the {} byte limit",
                    MAX_WASM_MODULE_BYTES
                );
                return;
            }
        },
        Err(e) => {
            warn!("Failed to stat effect Wasm module: {}", e);
            return;
        }
    };

    let mut wasm_bytes = Vec::new();
    if wasm_bytes.try_reserve_exact(module_size).is_err() {
        error!("Failed to allocate memory for effect Wasm module");
        return;
    }
    wasm_bytes.resize(module_size, 0);
    match sd.read_file(module_path.as_str(), &mut wasm_bytes).await {
        Ok(len) => wasm_bytes.truncate(len),
        Err(e) => {
            warn!("Failed to read effect Wasm module: {}", e);
            return;
        }
    }

    match EffectInstance::load(&wasm_bytes, crate::config::get()) {
        Ok(effect) => {
            *ACTIVE_EFFECT.lock().await = Some(effect);
        }
        Err(e) => {
            error!(
                "Failed to initialize configured effect: {}",
                defmt::Debug2Format(&e)
            )
        }
    }
}

/// Drops the active effect, if one is loaded.
pub(crate) async fn unload_active_effect() {
    *ACTIVE_EFFECT.lock().await = None;
}

/// Advances the loaded effect and publishes each changed frame to the LEDs.
#[embassy_executor::task]
pub(crate) async fn task() -> ! {
    const DEFAULT_STEP_DELAY_MS: u64 = 16;

    let start = Instant::now();

    if render_active_frame().await {
        crate::leds::draw().await;
    }

    let initial_delay = {
        let mut active_effect = ACTIVE_EFFECT.lock().await;
        match active_effect.as_mut() {
            Some(effect) => match effect.step_time() {
                Ok(time_ms) => Some(time_ms as u64),
                Err(error) => {
                    warn!(
                        "Active effect failed while reading its step time: {}",
                        defmt::Debug2Format(&error)
                    );
                    *active_effect = None;
                    None
                }
            },
            None => None,
        }
    };
    let mut delay_ms = initial_delay.unwrap_or(DEFAULT_STEP_DELAY_MS);

    loop {
        Timer::after_millis(delay_ms).await;
        let now = Instant::now();
        let elapsed_ms = now.duration_since(start).as_millis().min(i32::MAX as u64) as i32;

        let step_result = {
            let mut active_effect = ACTIVE_EFFECT.lock().await;
            match active_effect.as_mut() {
                Some(effect) => match effect.step(elapsed_ms) {
                    Ok(result) => match effect.step_time() {
                        Ok(time_to_next) => Some((result, time_to_next as u64)),
                        Err(error) => {
                            warn!(
                                "Active effect failed while reading its step time: {}",
                                defmt::Debug2Format(&error)
                            );
                            *active_effect = None;
                            None
                        }
                    },
                    Err(error) => {
                        warn!(
                            "Active effect failed while stepping: {}",
                            defmt::Debug2Format(&error)
                        );
                        *active_effect = None;
                        None
                    }
                },
                None => None,
            }
        };

        if let Some((result, time_to_next)) = step_result {
            delay_ms = time_to_next;
            if result == StepResult::Changed && render_active_frame().await {
                crate::leds::draw().await;
            }
        }
    }
}

async fn render_active_frame() -> bool {
    let active_effect = ACTIVE_EFFECT.lock().await;
    let Some(effect) = active_effect.as_ref() else {
        return false;
    };
    let frame = effect.frame();
    let mut channel_data = crate::leds::data().await;
    let mut frame_offset = 0usize;

    for fixture in crate::config::get().fixtures() {
        for span in fixture.spans() {
            let channel = usize::from(*span.channel().as_ref());
            let start = *span.start() as usize;
            let length = *span.length() as usize;
            let Some(end) = start.checked_add(length) else {
                warn!("Effect span index overflow while rendering frame");
                return false;
            };
            let Some(frame_byte_count) = length.checked_mul(3) else {
                warn!("Effect span length overflow while rendering frame");
                return false;
            };
            let Some(frame_end) = frame_offset.checked_add(frame_byte_count) else {
                warn!("Effect frame offset overflow while rendering frame");
                return false;
            };
            let Some(source) = frame.get(frame_offset..frame_end) else {
                warn!("Effect frame is shorter than the configured pixel layout");
                return false;
            };

            let Ok(channel_pixels) = channel_data.try_channel_mut::<RGB8>(channel) else {
                warn!("Effect references an unavailable LED output channel");
                return false;
            };
            let Some(destination) = channel_pixels.get_mut(start..end) else {
                warn!("Effect span exceeds the LED output channel buffer");
                return false;
            };

            let (rgb_pixels, _) = source.as_chunks::<3>();
            for (pixel, rgb) in destination.iter_mut().zip(rgb_pixels) {
                *pixel = RGB8::new(rgb[0], rgb[1], rgb[2]);
            }
            frame_offset = frame_end;
        }
    }

    if frame_offset != frame.len() {
        warn!("Effect frame length does not match the configured pixel layout");
        return false;
    }
    true
}
