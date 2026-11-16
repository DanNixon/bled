use super::active::{activate_effect_direct, step_active_effect, unload_active_effect};
use bled_core::{
    PathString,
    api::ActivePlaylistInfo,
    effect::{EffectPlaylist, NextPlaylistEntryAction},
    util::resolve_relative_path,
};
use defmt::{debug, info, warn};
use embassy_futures::select::{Either, select};
use embassy_rp::clocks::RoscRng;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex, signal::Signal,
};
use embassy_time::{Instant, Timer};

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

static ACTIVE_PLAYLIST: Mutex<CriticalSectionRawMutex, Option<ActivePlaylist>> = Mutex::new(None);

static CONTROL_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
static CONTROL_REQUEST: Channel<CriticalSectionRawMutex, ControlRequest, 1> = Channel::new();
static CONTROL_COMPLETE: Signal<CriticalSectionRawMutex, Result<(), ()>> = Signal::new();

#[inline(never)]
async fn load_default_playlist(sd: &crate::sdcard::SdCardStorage) {
    let Ok(path) = PathString::try_from(crate::config::DEFAULT_PLAYLIST_PATH) else {
        return;
    };
    let _ = unload_active_effect().await;
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
        let _ = unload_active_effect().await;
        return Ok(());
    };

    let has_entries = {
        let mut guard = ACTIVE_PLAYLIST.lock().await;
        let playlist = crate::config::load_playlist_from_path(sd, path.as_str()).await?;
        set_active_playlist(&mut guard, path, playlist)
    };
    let _ = unload_active_effect().await;
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
                let _ = unload_active_effect().await;
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
            let _ = unload_active_effect().await;
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

async fn unload_active_playlist() {
    *ACTIVE_PLAYLIST.lock().await = None;
}

async fn handle_control_request(sd: &crate::sdcard::SdCardStorage, request: ControlRequest) {
    let result = match request {
        ControlRequest::ActivateEffect(path) => {
            unload_active_playlist().await;
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

        let Ok(mut delay_ms) = step_active_effect(elapsed_ms).await else {
            handle_playlist_entry_failure(&sd).await;
            continue;
        };

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
