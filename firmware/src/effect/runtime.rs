use super::{
    active::{activate_effect_direct, step_active_effect},
    playlist::{
        activate_playlist_direct, check_playlist_duration, handle_playlist_entry_failure,
        load_default_playlist, playlist_remaining_ms, unload_active_playlist,
    },
};
use bled_core::PathString;
use defmt::debug;
use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, mutex::Mutex, signal::Signal,
};
use embassy_time::{Instant, Timer};

enum ControlRequest {
    ActivateEffect(Option<PathString>),
    ActivatePlaylist(Option<PathString>),
    ReloadFixtures,
}

static CONTROL_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
static CONTROL_REQUEST: Channel<CriticalSectionRawMutex, ControlRequest, 1> = Channel::new();
static CONTROL_COMPLETE: Signal<CriticalSectionRawMutex, Result<(), ()>> = Signal::new();

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
