use super::{EffectError, EffectInstance};
use alloc::vec::Vec;
use bled_core::{
    PathString,
    api::{ActivePlaylistInfo, Chunk, MAX_CHUNK_SIZE},
    effect::{
        EffectPlaylist, EffectPreset, MAX_EFFECT_PRESET_JSON_SIZE, NextPlaylistEntryAction,
        abi::export::StepResult,
    },
    util::{has_extension, resolve_relative_path},
};
use defmt::{debug, error, info, warn};
use embassy_futures::select::{Either, select};
use embassy_rp::clocks::RoscRng;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex, signal::Signal,
};
use embassy_time::{Instant, Timer};
use portable_atomic::{AtomicU32, Ordering};

const MAX_WASM_MODULE_BYTES: usize = 64 * 1024;

struct ActiveEffect {
    instance_id: u32,
    module_path: PathString,
    instance: EffectInstance,
}

struct ActivePlaylist {
    path: PathString,
    playlist: EffectPlaylist,
    position: Option<usize>,
    entry_started_at: Instant,
}

enum ControlRequest {
    ActivateEffect(Option<PathString>),
    ActivatePlaylist(Option<PathString>),
    ReloadFixtures,
}

static ACTIVE_EFFECT: Mutex<CriticalSectionRawMutex, Option<ActiveEffect>> = Mutex::new(None);
static ACTIVE_PLAYLIST: Mutex<CriticalSectionRawMutex, Option<ActivePlaylist>> = Mutex::new(None);
static NEXT_INSTANCE_ID: AtomicU32 = AtomicU32::new(0);

static CONTROL_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
static CONTROL_REQUEST: Channel<CriticalSectionRawMutex, ControlRequest, 1> = Channel::new();
static CONTROL_COMPLETE: Signal<CriticalSectionRawMutex, Result<(), ()>> = Signal::new();

#[inline(never)]
async fn load_default_playlist(sd: &crate::sdcard::SdCardStorage) {
    let Ok(path) = PathString::try_from(crate::config::DEFAULT_PLAYLIST_PATH) else {
        return;
    };
    *ACTIVE_EFFECT.lock().await = None;
    let has_entries = {
        let mut guard = ACTIVE_PLAYLIST.lock().await;
        let playlist = crate::config::load_default_playlist(sd).await;
        set_active_playlist(&mut guard, path, playlist)
    };
    if has_entries {
        load_playlist_entry_at(sd, 0).await;
    }
}

#[inline(never)]
async fn activate_playlist_direct(
    sd: &crate::sdcard::SdCardStorage,
    path: Option<PathString>,
) -> Result<(), ()> {
    let Some(path) = path else {
        *ACTIVE_PLAYLIST.lock().await = None;
        *ACTIVE_EFFECT.lock().await = None;
        return Ok(());
    };

    let has_entries = {
        let mut guard = ACTIVE_PLAYLIST.lock().await;
        let playlist = crate::config::load_playlist_from_path(sd, path.as_str()).await?;
        set_active_playlist(&mut guard, path, playlist)
    };
    *ACTIVE_EFFECT.lock().await = None;
    if has_entries {
        load_playlist_entry_at(sd, 0).await;
    }
    Ok(())
}

fn set_active_playlist(
    active_playlist: &mut Option<ActivePlaylist>,
    path: PathString,
    playlist: EffectPlaylist,
) -> bool {
    let has_entries = !playlist.is_empty();
    if has_entries {
        info!(
            "Loaded playlist '{}' with {} entries",
            path.as_str(),
            playlist.len()
        );
    } else {
        info!("Loaded empty playlist: {}", path.as_str());
    }
    *active_playlist = Some(ActivePlaylist {
        path,
        playlist,
        position: None,
        entry_started_at: Instant::now(),
    });
    has_entries
}

/// Attempts to load the playlist entry at `target_index`.
///
/// If an entry fails to load, it is removed from the in-memory playlist and its `after`
/// action is evaluated to select the next entry. If the playlist becomes empty, the
/// playlist and active effect are unloaded.
#[inline(never)]
async fn load_playlist_entry_at(sd: &crate::sdcard::SdCardStorage, mut target_index: usize) {
    loop {
        let candidate = {
            let mut guard = ACTIVE_PLAYLIST.lock().await;
            let Some(active) = guard.as_mut() else {
                return;
            };
            if active.playlist.is_empty() {
                *guard = None;
                drop(guard);
                *ACTIVE_EFFECT.lock().await = None;
                return;
            }
            let index = target_index % active.playlist.len();
            let entry = &active.playlist[index];
            (index, active.path.clone(), entry.path().clone())
        };

        let (index, playlist_path, entry_path) = candidate;

        let resolved_res = resolve_relative_path(playlist_path.as_str(), entry_path.as_str());
        let load_ok = match resolved_res {
            Ok(resolved_path) => activate_effect_direct(sd, Some(resolved_path))
                .await
                .is_ok(),
            Err(_) => {
                warn!("Playlist entry path '{}' is too long", entry_path.as_str());
                false
            }
        };

        if load_ok {
            let mut guard = ACTIVE_PLAYLIST.lock().await;
            if let Some(active) = guard.as_mut() {
                active.position = Some(index);
                active.entry_started_at = Instant::now();
            }
            return;
        }

        warn!(
            "Failed to load playlist entry {} ('{}'), removing from playlist",
            index,
            entry_path.as_str()
        );

        let Some(next_index) = remove_failed_playlist_entry(index).await else {
            return;
        };

        target_index = next_index;
    }
}

/// Removes the entry at `index` from the active playlist and resolves the next entry index.
///
/// If the playlist becomes empty, the active playlist and active effect are unloaded and
/// `None` is returned.
async fn remove_failed_playlist_entry(index: usize) -> Option<usize> {
    let mut guard = ACTIVE_PLAYLIST.lock().await;
    let active = guard.as_mut()?;
    if index >= active.playlist.len() {
        return None;
    }
    let removed = active.playlist.remove(index);
    let after_action = *removed.after();
    let remaining_len = active.playlist.len();
    match after_action.resolve_index_after_removal(index, remaining_len, RoscRng.next_u64()) {
        Some(next) => Some(next),
        None => {
            info!("Playlist is now empty after removing failed entry, unloading playlist");
            *guard = None;
            drop(guard);
            *ACTIVE_EFFECT.lock().await = None;
            None
        }
    }
}

/// Removes the currently active playlist entry after a runtime failure and advances to the next entry.
#[inline(never)]
pub(crate) async fn handle_playlist_entry_failure(sd: &crate::sdcard::SdCardStorage) {
    let current_index = {
        let guard = ACTIVE_PLAYLIST.lock().await;
        guard.as_ref().and_then(|active| active.position)
    };
    let Some(index) = current_index else {
        return;
    };
    warn!(
        "Active playlist entry {} failed, removing from playlist",
        index
    );
    if let Some(next_index) = remove_failed_playlist_entry(index).await {
        load_playlist_entry_at(sd, next_index).await;
    }
}

/// Advances the active playlist from its current position using the current entry's `after` action.
#[inline(never)]
async fn advance_playlist(sd: &crate::sdcard::SdCardStorage) {
    let next_index = {
        let guard = ACTIVE_PLAYLIST.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let Some(current_index) = active.position else {
            return;
        };
        let Some(entry) = active.playlist.get(current_index) else {
            return;
        };
        let action: NextPlaylistEntryAction = *entry.after();
        match action.resolve_index(current_index, active.playlist.len(), RoscRng.next_u64()) {
            Some(next) => next,
            None => return,
        }
    };

    load_playlist_entry_at(sd, next_index).await;
}

/// Checks if the current playlist entry's duration has elapsed, and if so, advances to the next entry.
async fn check_playlist_duration(sd: &crate::sdcard::SdCardStorage, now: Instant) {
    let expired = {
        let guard = ACTIVE_PLAYLIST.lock().await;
        if let Some(active) = guard.as_ref()
            && let Some(pos) = active.position
            && let Some(entry) = active.playlist.get(pos)
            && let Some(duration) = entry.duration()
        {
            let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            now.duration_since(active.entry_started_at).as_millis() >= duration_ms
        } else {
            false
        }
    };

    if expired {
        debug!("Playlist entry duration elapsed, advancing playlist");
        advance_playlist(sd).await;
    }
}

/// Returns the remaining time in milliseconds for the active playlist entry, if it has a finite duration.
async fn playlist_remaining_ms(now: Instant) -> Option<u64> {
    let guard = ACTIVE_PLAYLIST.lock().await;
    let active = guard.as_ref()?;
    let pos = active.position?;
    let entry = active.playlist.get(pos)?;
    let duration = (*entry.duration())?;
    let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    let elapsed_ms = now.duration_since(active.entry_started_at).as_millis();
    Some(duration_ms.saturating_sub(elapsed_ms))
}

/// Unloads the currently active effect, if any, and returns its path.
#[inline(never)]
pub(crate) async fn unload_active_effect() -> Option<PathString> {
    ACTIVE_EFFECT
        .lock()
        .await
        .take()
        .map(|effect| effect.module_path)
}

/// Requests the effect task to activate an effect given a path to either a WASM module or JSON preset.
pub(crate) async fn activate_effect(path: Option<PathString>) -> Result<(), ()> {
    let _guard = CONTROL_MUTEX.lock().await;
    CONTROL_REQUEST
        .send(ControlRequest::ActivateEffect(path))
        .await;
    CONTROL_COMPLETE.wait().await
}

/// Requests the effect task to activate a playlist given a path to a JSON playlist file, or unload the current playlist.
pub(crate) async fn activate_playlist(path: Option<PathString>) -> Result<(), ()> {
    let _guard = CONTROL_MUTEX.lock().await;
    CONTROL_REQUEST
        .send(ControlRequest::ActivatePlaylist(path))
        .await;
    CONTROL_COMPLETE.wait().await
}

/// Requests the effect task to reload the fixture configuration and active effect from the SD card.
pub(crate) async fn reload_fixtures() {
    let _guard = CONTROL_MUTEX.lock().await;
    CONTROL_REQUEST.send(ControlRequest::ReloadFixtures).await;
    let _ = CONTROL_COMPLETE.wait().await;
}

/// Activates an effect given a path to either a WASM module or JSON preset.
#[inline(never)]
pub(crate) async fn activate_effect_direct(
    sd: &crate::sdcard::SdCardStorage,
    path: Option<PathString>,
) -> Result<(), ()> {
    // Unload the active effect, if any
    *ACTIVE_EFFECT.lock().await = None;

    // Load the effect, if a path is provided
    if let Some(module_path) = path {
        let path_str = module_path.as_str();
        let instance = if has_extension(path_str, "wasm") {
            load_effect_instance(sd, path_str).await?
        } else if has_extension(path_str, "json") {
            load_effect_preset(sd, path_str).await?
        } else {
            error!(
                "Unable to determine effect file type for '{}'; expected .wasm or .json",
                path_str
            );
            return Err(());
        };

        info!("Loaded effect: {}", module_path.as_str());
        *ACTIVE_EFFECT.lock().await = Some(ActiveEffect {
            instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
            module_path,
            instance,
        });
    };

    Ok(())
}

#[inline(never)]
async fn load_effect_preset(
    sd: &crate::sdcard::SdCardStorage,
    preset_path: &str,
) -> Result<EffectInstance, ()> {
    let preset = {
        let mut buf = [0; MAX_EFFECT_PRESET_JSON_SIZE];
        let len = match sd.read_file(preset_path, &mut buf).await {
            Ok(len) => len,
            Err(e) => {
                warn!("Failed to read effect JSON preset: {}", e);
                return Err(());
            }
        };
        let preset_bytes = &buf[..len];

        match bled_core::io::decode_json::<EffectPreset>(preset_bytes) {
            Ok(preset) => preset,
            Err(e) => {
                warn!("Failed to parse effect JSON preset: {}", e);
                return Err(());
            }
        }
    };

    let module_path = match resolve_relative_path(preset_path, preset.module().as_str()) {
        Ok(path) => path,
        Err(_) => {
            warn!("Effect JSON preset module path is too long");
            return Err(());
        }
    };
    let mut instance = load_effect_instance(sd, module_path.as_str()).await?;

    if !preset.command().is_empty()
        && let Err(e) = instance.write_command_data(preset.command())
    {
        warn!(
            "Failed to apply effect preset command: {}",
            defmt::Debug2Format(&e)
        );
        return Err(());
    }
    if !preset.parameters().is_empty()
        && let Err(e) = instance.write_parameter_data(preset.parameters())
    {
        warn!(
            "Failed to apply effect preset parameters: {}",
            defmt::Debug2Format(&e)
        );
        return Err(());
    }

    Ok(instance)
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

    let load_result =
        crate::config::with_fixtures(|fixtures| EffectInstance::load(&wasm_bytes, fixtures)).await;
    match load_result {
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

pub(crate) async fn active_playlist_info() -> Option<ActivePlaylistInfo> {
    ACTIVE_PLAYLIST.lock().await.as_ref().map(|active| {
        ActivePlaylistInfo::new(
            active.path.clone(),
            active.position.and_then(|pos| u8::try_from(pos).ok()),
        )
    })
}

pub(crate) async fn with_active_playlist<T>(
    operation: impl FnOnce(Option<&EffectPlaylist>) -> T,
) -> T {
    let active = ACTIVE_PLAYLIST.lock().await;
    operation(active.as_ref().map(|p| &p.playlist))
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

pub(crate) async fn read_effect_command() -> Result<Chunk, ()> {
    with_active_effect(|effect| {
        let mut chunk = Chunk::new();
        chunk
            .resize(MAX_CHUNK_SIZE, 0)
            .map_err(|_| EffectError::InvalidBuffer)?;
        let length = effect.read_command_data(&mut chunk)?;
        chunk.truncate(length);
        Ok(chunk)
    })
    .await
}

pub(crate) async fn write_effect_command(buf: &[u8]) -> Result<(), ()> {
    with_active_effect(|effect| effect.write_command_data(buf)).await
}

pub(crate) async fn effect_parameter_size() -> Result<usize, ()> {
    with_active_effect(|effect| effect.parameter_size()).await
}

pub(crate) async fn read_effect_parameters() -> Result<Chunk, ()> {
    with_active_effect(|effect| {
        let mut chunk = Chunk::new();
        chunk
            .resize(MAX_CHUNK_SIZE, 0)
            .map_err(|_| EffectError::InvalidBuffer)?;
        let length = effect.read_parameter_data(&mut chunk)?;
        chunk.truncate(length);
        Ok(chunk)
    })
    .await
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

async fn handle_control_request(sd: &crate::sdcard::SdCardStorage, request: ControlRequest) {
    let result = match request {
        ControlRequest::ActivateEffect(path) => {
            *ACTIVE_PLAYLIST.lock().await = None;
            activate_effect_direct(sd, path).await
        }
        ControlRequest::ActivatePlaylist(path) => activate_playlist_direct(sd, path).await,
        ControlRequest::ReloadFixtures => {
            crate::config::reload_fixtures(sd).await;
            Ok(())
        }
    };
    CONTROL_COMPLETE.signal(result);
}

#[embassy_executor::task]
pub(crate) async fn task(sd: crate::sdcard::SdCardStorage) -> ! {
    crate::config::reload_fixtures(&sd).await;
    load_default_playlist(&sd).await;

    let start = Instant::now();

    loop {
        while let Ok(request) = CONTROL_REQUEST.try_receive() {
            handle_control_request(&sd, request).await;
        }

        check_playlist_duration(&sd, Instant::now()).await;

        let now = Instant::now();
        let elapsed_ms = now.duration_since(start).as_millis().min(i32::MAX as u64) as i32;

        let mut step_failed = false;
        let mut active_effect = ACTIVE_EFFECT.lock().await;

        if let Some(effect) = active_effect.as_mut() {
            debug!("Stepping effect");
            match effect.instance.step(elapsed_ms) {
                Ok(StepResult::Changed) => {
                    debug!("Pixels changed after step, rendering");

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
                                    step_failed = true;
                                }
                                Err(error) => {
                                    warn!(
                                        "Failed to read effect frame: {}",
                                        defmt::Debug2Format(&error)
                                    );
                                    *active_effect = None;
                                    step_failed = true;
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
                            step_failed = true;
                        }
                        Err(error) => {
                            warn!(
                                "Failed to read effect frame length: {}",
                                defmt::Debug2Format(&error)
                            );
                            *active_effect = None;
                            step_failed = true;
                        }
                    }
                }
                Ok(StepResult::Unchanged) => {
                    debug!("Pixels unchanged after step");
                }
                Ok(StepResult::Error) => {
                    warn!("Active effect reported an error while stepping");
                    *active_effect = None;
                    step_failed = true;
                }
                Err(error) => {
                    warn!(
                        "Active effect failed while stepping: {}",
                        defmt::Debug2Format(&error)
                    );
                    *active_effect = None;
                    step_failed = true;
                }
            }
        }

        const DEFAULT_STEP_TIME: u64 = 100;
        let mut delay_ms = match active_effect.as_mut() {
            Some(effect) => match effect.instance.step_time() {
                Ok(time_to_next) => time_to_next as u64,
                Err(error) => {
                    warn!(
                        "Active effect failed while reading its step time: {}",
                        defmt::Debug2Format(&error)
                    );
                    *active_effect = None;
                    step_failed = true;
                    DEFAULT_STEP_TIME
                }
            },
            None => DEFAULT_STEP_TIME,
        };

        drop(active_effect);

        if step_failed {
            handle_playlist_entry_failure(&sd).await;
            continue;
        }

        if let Some(remaining_ms) = playlist_remaining_ms(Instant::now()).await {
            delay_ms = delay_ms.min(remaining_ms);
        }
        debug!("Next step in: {}ms", delay_ms);

        if let Either::First(request) =
            select(CONTROL_REQUEST.receive(), Timer::after_millis(delay_ms)).await
        {
            handle_control_request(&sd, request).await;
        }
    }
}
