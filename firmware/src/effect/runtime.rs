use super::{EffectError, EffectInstance};
use alloc::vec::Vec;
use bled_core::{PathString, effect::abi::export::StepResult};
use defmt::{debug, error, info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Instant, Timer};
use portable_atomic::{AtomicU32, Ordering};

const MAX_WASM_MODULE_BYTES: usize = 64 * 1024;

struct ActiveEffect {
    instance_id: u32,
    module_path: PathString,
    instance: EffectInstance,
}

static ACTIVE_EFFECT: Mutex<CriticalSectionRawMutex, Option<ActiveEffect>> = Mutex::new(None);
static NEXT_INSTANCE_ID: AtomicU32 = AtomicU32::new(0);

#[inline(never)]
pub(crate) async fn load_default_effect(sd: &crate::sdcard::SdCardStorage) {
    let Some(module_path) = crate::config::get().default_effect.as_ref() else {
        info!("No default effect configured");
        return;
    };

    if activate_effect(sd, Some(module_path.clone()))
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
    if let Some(module_path) = path {
        let instance = load_effect_instance(sd, module_path.as_str()).await?;
        debug!("Effect manifest: {} bytes", instance.manifest_bytes().len());
        info!("Loaded effect module: {}", module_path.as_str());
        *ACTIVE_EFFECT.lock().await = Some(ActiveEffect {
            instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
            module_path,
            instance,
        });
    };

    Ok(())
}

#[inline(never)]
async fn load_effect_instance(
    sd: &crate::sdcard::SdCardStorage,
    module_path: &str,
) -> Result<EffectInstance, ()> {
    let module_size = match sd.stat_file(module_path).await {
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
    match sd.read_file(module_path, &mut wasm_bytes).await {
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
        .map(|effect| effect.module_path.clone())
}

/// Access manifest bytes only while holding the active effect lock, without copying the manifest.
pub(crate) async fn with_active_effect_manifest<T>(
    operation: impl FnOnce(Option<(u32, &[u8])>) -> T,
) -> T {
    let active = ACTIVE_EFFECT.lock().await;
    operation(
        active
            .as_ref()
            .map(|effect| (effect.instance_id, effect.instance.manifest_bytes())),
    )
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
            debug!("Stepping effect");
            match effect.instance.step(elapsed_ms) {
                Ok(StepResult::Changed) => {
                    info!("Pixels changed after step, rendering");

                    let mut channel_data = crate::leds::data().await;
                    match effect.instance.frame_size() {
                        Ok(guest_buffer_len) if guest_buffer_len == channel_data.len() => {
                            match effect.instance.read_frame_data(channel_data.inner_mut()) {
                                Ok(read_len) if read_len == channel_data.len() => {
                                    drop(channel_data);
                                    crate::leds::draw().await;
                                }
                                Ok(read_len) => {
                                    warn!(
                                        "Effect returned an incomplete frame: guest={} host={}",
                                        read_len,
                                        channel_data.len()
                                    );
                                    *active_effect = None;
                                }
                                Err(error) => {
                                    warn!(
                                        "Failed to read effect frame: {}",
                                        defmt::Debug2Format(&error)
                                    );
                                    *active_effect = None;
                                }
                            }
                        }
                        Ok(guest_buffer_len) => {
                            warn!(
                                "Invalid effect frame length: guest={} host={}",
                                guest_buffer_len,
                                channel_data.len()
                            );
                            *active_effect = None;
                        }
                        Err(error) => {
                            warn!(
                                "Failed to read effect frame length: {}",
                                defmt::Debug2Format(&error)
                            );
                            *active_effect = None;
                        }
                    }
                }
                Ok(StepResult::Unchanged) => {
                    debug!("Pixels unchanged after step");
                }
                Ok(StepResult::Error) => {
                    warn!("Active effect reported an error while stepping");
                    *active_effect = None;
                }
                Err(error) => {
                    warn!(
                        "Active effect failed while stepping: {}",
                        defmt::Debug2Format(&error)
                    );
                    *active_effect = None;
                }
            }
        }

        const DEFAULT_STEP_TIME: u64 = 100;
        let delay_ms = match active_effect.as_mut() {
            Some(effect) => match effect.instance.step_time() {
                Ok(time_to_next) => time_to_next as u64,
                Err(error) => {
                    warn!(
                        "Active effect failed while reading its step time: {}",
                        defmt::Debug2Format(&error)
                    );
                    *active_effect = None;
                    DEFAULT_STEP_TIME
                }
            },
            None => DEFAULT_STEP_TIME,
        };
        debug!("Next step in: {}ms", delay_ms);

        drop(active_effect);

        Timer::after_millis(delay_ms).await;
    }
}
