use super::{EffectError, EffectInstance};
use alloc::vec::Vec;
use bled_core::{
    PathString,
    effect::{EffectManifest, MAX_EFFECT_MANIFEST_JSON_SIZE, abi::export::StepResult},
};
use defmt::{error, info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Instant, Timer};

const MAX_WASM_MODULE_BYTES: usize = 64 * 1024;

struct ActiveEffect {
    manifest_path: PathString,
    instance: EffectInstance,
}

static ACTIVE_EFFECT: Mutex<CriticalSectionRawMutex, Option<ActiveEffect>> = Mutex::new(None);

#[inline(never)]
pub(crate) async fn load_default_effect(sd: &crate::sdcard::SdCardStorage) {
    let Some(manifest_path) = crate::config::get().default_effect.as_ref() else {
        info!("No default effect configured");
        return;
    };

    if activate_effect(sd, Some(manifest_path.clone()))
        .await
        .is_err()
    {
        warn!("Failed to load configured default effect");
    }
}

#[inline(never)]
pub(crate) async fn activate_effect(
    sd: &crate::sdcard::SdCardStorage,
    path: Option<PathString>,
) -> Result<(), ()> {
    // Unload the active effect, if any
    *ACTIVE_EFFECT.lock().await = None;

    // Load the effect, if a path is provided
    if let Some(path) = path {
        let Some(manifest) = load_effect_manifest(sd, path.as_str()).await else {
            return Err(());
        };
        let instance = load_effect_instance(sd, manifest).await?;
        *ACTIVE_EFFECT.lock().await = Some(ActiveEffect {
            manifest_path: path,
            instance,
        });
    };

    Ok(())
}

#[inline(never)]
async fn load_effect_manifest(
    sd: &crate::sdcard::SdCardStorage,
    manifest_path: &str,
) -> Option<EffectManifest> {
    let mut manifest_bytes = [0; MAX_EFFECT_MANIFEST_JSON_SIZE];
    let manifest_len = match sd.read_file(manifest_path, &mut manifest_bytes).await {
        Ok(len) => len,
        Err(e) => {
            warn!("Failed to read effect manifest: {}", e);
            return None;
        }
    };

    match bled_core::io::decode_json::<EffectManifest>(&manifest_bytes[..manifest_len]) {
        Ok(manifest) => Some(manifest),
        Err(e) => {
            warn!("Failed to parse effect manifest: {}", e);
            None
        }
    }
}

#[inline(never)]
async fn load_effect_instance(
    sd: &crate::sdcard::SdCardStorage,
    manifest: EffectManifest,
) -> Result<EffectInstance, ()> {
    let module_path = manifest.wasm_module();
    let module_size = match sd.stat_file(module_path.as_str()).await {
        Ok(size) => match usize::try_from(size) {
            Ok(size) if size <= MAX_WASM_MODULE_BYTES => size,
            _ => {
                warn!(
                    "Effect Wasm module exceeds the {} byte limit",
                    MAX_WASM_MODULE_BYTES
                );
                return Err(());
            }
        },
        Err(e) => {
            warn!("Failed to stat effect Wasm module: {}", e);
            return Err(());
        }
    };

    let mut wasm_bytes = Vec::new();
    if wasm_bytes.try_reserve_exact(module_size).is_err() {
        error!("Failed to allocate memory for effect Wasm module");
        return Err(());
    }
    wasm_bytes.resize(module_size, 0);
    match sd.read_file(module_path.as_str(), &mut wasm_bytes).await {
        Ok(len) => wasm_bytes.truncate(len),
        Err(e) => {
            warn!("Failed to read effect Wasm module: {}", e);
            return Err(());
        }
    }

    match EffectInstance::load(&wasm_bytes, crate::config::get()) {
        Ok(effect) => Ok(effect),
        Err(e) => {
            error!("Failed to initialize effect: {}", defmt::Debug2Format(&e));
            Err(())
        }
    }
}

pub(crate) async fn active_effect_path() -> Option<PathString> {
    ACTIVE_EFFECT
        .lock()
        .await
        .as_ref()
        .map(|effect| effect.manifest_path.clone())
}

pub(crate) async fn effect_command_size() -> Result<usize, ()> {
    with_active_effect(|effect| effect.command_size()).await
}

pub(crate) async fn read_effect_command(buf: &mut [u8]) -> Result<usize, ()> {
    with_active_effect(|effect| effect.read_command_data(buf)).await
}

pub(crate) async fn write_effect_command(buf: &[u8]) -> Result<(), ()> {
    with_active_effect(|effect| effect.write_command_data(buf)).await
}

pub(crate) async fn effect_parameter_size() -> Result<usize, ()> {
    with_active_effect(|effect| effect.parameter_size()).await
}

pub(crate) async fn read_effect_parameters(buf: &mut [u8]) -> Result<usize, ()> {
    with_active_effect(|effect| effect.read_parameter_data(buf)).await
}

pub(crate) async fn write_effect_parameters(buf: &[u8]) -> Result<(), ()> {
    with_active_effect(|effect| effect.write_parameter_data(buf)).await
}

async fn with_active_effect<T>(
    operation: impl FnOnce(&mut EffectInstance) -> Result<T, EffectError>,
) -> Result<T, ()> {
    let mut active = ACTIVE_EFFECT.lock().await;
    let Some(effect) = active.as_mut() else {
        return Err(());
    };
    operation(&mut effect.instance).map_err(|error| {
        warn!(
            "Effect data exchange failed: {}",
            defmt::Debug2Format(&error)
        );
    })
}

#[embassy_executor::task]
pub(crate) async fn task() -> ! {
    let start = Instant::now();

    loop {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(start).as_millis().min(i32::MAX as u64) as i32;

        let mut active_effect = ACTIVE_EFFECT.lock().await;

        if let Some(effect) = active_effect.as_mut() {
            info!("Stepping effect");
            match effect.instance.step(elapsed_ms).unwrap() {
                StepResult::Changed => {
                    info!("Pixels changed after step, rendering");

                    let mut channel_data = crate::leds::data().await;

                    let guest_buffer_len = effect.instance.frame_size().unwrap();
                    if guest_buffer_len != channel_data.len() {
                        panic!(
                            "Invalid buffer length: guest={} host={}",
                            guest_buffer_len,
                            channel_data.len()
                        );
                    }

                    effect
                        .instance
                        .read_frame_data(channel_data.inner_mut())
                        .unwrap();

                    drop(channel_data);

                    crate::leds::draw().await;
                }
                StepResult::Unchanged => {
                    info!("Pixels unchanged after step");
                }
                StepResult::Error => {
                    warn!("Active effect failed while stepping");
                    *active_effect = None;
                }
            }
        }

        let delay_ms = match active_effect.as_mut() {
            Some(effect) => match effect.instance.step_time() {
                Ok(time_to_next) => time_to_next as u64,
                Err(error) => {
                    warn!(
                        "Active effect failed while reading its step time: {}",
                        defmt::Debug2Format(&error)
                    );
                    100
                }
            },
            None => 100,
        };
        info!("Next step in: {}ms", delay_ms);

        drop(active_effect);

        Timer::after_millis(delay_ms).await;
    }
}
