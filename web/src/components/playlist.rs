use crate::state::{ConnectedDevice, LogLevel, log};
use bled_core::{api::DirEntryType, effect::NextPlaylistEntryAction};
use core::time::Duration;
use leptodon::button::{Button, ButtonAppearance};
use leptodon::input::TextInput;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

fn format_duration(duration: Option<Duration>) -> String {
    match duration {
        None => "Indefinite".to_string(),
        Some(d) => {
            let ms = d.as_millis();
            if ms >= 1000 && ms.is_multiple_of(1000) {
                format!("{}s ({ms} ms)", ms / 1000)
            } else {
                format!("{ms} ms")
            }
        }
    }
}

fn format_after_action(after: NextPlaylistEntryAction) -> String {
    match after {
        NextPlaylistEntryAction::Shift(offset) => {
            if offset >= 0 {
                format!("Shift (+{offset})")
            } else {
                format!("Shift ({offset})")
            }
        }
        NextPlaylistEntryAction::Random => "Random".to_string(),
    }
}

#[component]
pub fn PlaylistPanel(dev: ConnectedDevice) -> impl IntoView {
    let initial_path = dev
        .active_playlist_info
        .get_untracked()
        .map(|info| info.path().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "/playlist.json".to_string());

    let path_sig = RwSignal::new(initial_path);
    let dir_sig = RwSignal::new("/".to_string());
    let json_files_sig = RwSignal::new(Vec::<String>::new());
    let is_browsing_sig = RwSignal::new(false);

    // Sync path input when active playlist info updates
    Effect::watch(
        {
            let dev = dev.clone();
            move || dev.active_playlist_info.get()
        },
        move |info_opt, _, _| {
            if let Some(info) = info_opt
                && !info.path().is_empty()
            {
                path_sig.set(info.path().to_string());
            }
        },
        false,
    );

    // Refresh playlist state when panel mounts
    {
        let dev = dev.clone();
        Effect::new(move || {
            let dev = dev.clone();
            spawn_local(async move {
                dev.refresh_playlist().await;
            });
        });
    }

    let d_refresh = dev.clone();
    let on_refresh = move |_| {
        let dev = d_refresh.clone();
        spawn_local(async move {
            dev.refresh_playlist().await;
        });
    };

    let d_load = dev.clone();
    let on_load_playlist = move |_| {
        let dev = d_load.clone();
        let path = path_sig.get_untracked();
        let path = path.trim().to_string();
        if path.is_empty() {
            log(
                "Enter a playlist file path before loading a playlist.",
                LogLevel::Error,
            );
            return;
        }

        spawn_local(async move {
            let res = {
                let mut rpc = dev.rpc.lock().await;
                rpc.set_active_playlist(Some(&path)).await
            };
            match res {
                Ok(()) => {
                    log(format!("Loaded playlist \"{path}\"."), LogLevel::Success);
                    dev.refresh_playlist().await;
                }
                Err(e) => log(
                    format!("Failed to load playlist \"{path}\": {e}"),
                    LogLevel::Error,
                ),
            }
        });
    };

    let d_unload = dev.clone();
    let on_unload_playlist = move |_| {
        let dev = d_unload.clone();
        spawn_local(async move {
            let res = {
                let mut rpc = dev.rpc.lock().await;
                rpc.set_active_playlist(None).await
            };
            match res {
                Ok(()) => {
                    log("Unloaded active playlist.", LogLevel::Success);
                    dev.refresh_playlist().await;
                }
                Err(e) => log(format!("Failed to unload playlist: {e}"), LogLevel::Error),
            }
        });
    };

    let d_browse = dev.clone();
    let on_browse_playlists = move |_| {
        let dev = d_browse.clone();
        let dir = {
            let raw = dir_sig.get_untracked();
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                "/".to_string()
            } else {
                trimmed.to_string()
            }
        };
        is_browsing_sig.set(true);

        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.list_directory(&dir).await {
                Ok(entries) => {
                    let files: Vec<String> = entries
                        .into_iter()
                        .filter(|e| {
                            matches!(e.kind(), DirEntryType::File { .. })
                                && bled_core::util::has_extension(e.name().as_str(), "json")
                        })
                        .map(|e| {
                            if dir == "/" {
                                format!("/{}", e.name())
                            } else {
                                format!("{}/{}", dir.trim_end_matches('/'), e.name())
                            }
                        })
                        .collect();
                    json_files_sig.set(files);
                }
                Err(e) => log(
                    format!("Failed to list JSON files in \"{dir}\": {e}"),
                    LogLevel::Error,
                ),
            }
            is_browsing_sig.set(false);
        });
    };

    let d_active_label = dev.clone();
    let d_entries = dev.clone();

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="flex items-center justify-between border-b border-gray-800 pb-3">
                <div>
                    <h3 class="text-base font-semibold text-white">"Playlist"</h3>
                    <p class="text-xs text-gray-400">
                        "View the active effect playlist or load a playlist JSON file from the device SD card."
                    </p>
                </div>
                <Button appearance=ButtonAppearance::Secondary on_click=on_refresh>
                    "Refresh"
                </Button>
            </div>

            // Playlist file path bar
            <div class="flex flex-wrap items-end gap-3">
                <div class="flex-1 min-w-[240px]">
                    <TextInput label="Playlist file path" placeholder="/playlist.json" value=path_sig />
                </div>
                <Button appearance=ButtonAppearance::Primary on_click=on_load_playlist>
                    "Load playlist"
                </Button>
                <Button appearance=ButtonAppearance::Secondary on_click=on_unload_playlist>
                    "Unload"
                </Button>
            </div>

            // Browse directory
            <div class="space-y-2 bg-gray-950/60 p-3.5 rounded-xl border border-gray-800">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <div class="flex items-center gap-2 flex-1 min-w-[200px]">
                        <span class="text-xs font-medium text-gray-400">"Browse directory:"</span>
                        <div class="w-48">
                            <TextInput value=dir_sig />
                        </div>
                        <Button appearance=ButtonAppearance::Secondary on_click=on_browse_playlists>
                            "List JSON files"
                        </Button>
                    </div>
                    <span class="text-xs text-oa-blue font-mono">
                        {move || {
                            match d_active_label.active_playlist_info.get() {
                                Some(info) => {
                                    let pos_label = match info.position() {
                                        Some(pos) => format!("entry #{pos}"),
                                        None => "empty".to_string(),
                                    };
                                    format!("Active: {} ({pos_label})", info.path())
                                }
                                None => "No active playlist loaded.".into(),
                            }
                        }}
                    </span>
                </div>

                <div class="flex flex-wrap gap-2 pt-1">
                    {move || {
                        let files = json_files_sig.get();
                        if files.is_empty() {
                            view! {
                                <span class="text-xs text-gray-500 italic">
                                    {if is_browsing_sig.get() { "Searching..." } else { "Browse a directory to find playlist JSON files." }}
                                </span>
                            }.into_any()
                        } else {
                            files.into_iter().map(|f_path| {
                                let f_click = f_path.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="px-2.5 py-1 rounded bg-gray-800 hover:bg-gray-700 text-xs font-mono text-gray-300 border border-gray-700 transition-colors"
                                        on:click=move |_| path_sig.set(f_click.clone())
                                    >
                                        {f_path}
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </div>
            </div>

            // Active in-memory playlist table
            {move || {
                let info_opt = d_entries.active_playlist_info.get();
                let playlist_opt = d_entries.active_playlist.get();
                let count = playlist_opt.as_ref().map(|p| p.len()).unwrap_or(0);
                let active_pos = info_opt.as_ref().and_then(|i| *i.position()).map(usize::from);

                view! {
                    <div class="space-y-2 pt-2 border-t border-gray-800">
                        <span class="text-xs font-semibold text-gray-300 block">
                            {format!("Active in-memory playlist ({count} entries)")}
                        </span>
                        <div class="bg-gray-950/40 rounded-lg border border-gray-800 overflow-hidden text-xs">
                            {match playlist_opt {
                                None => view! {
                                    <div class="p-4 text-center text-gray-500 italic">
                                        "No active playlist loaded."
                                    </div>
                                }.into_any(),
                                Some(playlist) if playlist.is_empty() => view! {
                                    <div class="p-4 text-center text-gray-500 italic">
                                        "The loaded playlist has no entries."
                                    </div>
                                }.into_any(),
                                Some(playlist) => view! {
                                    <table class="w-full text-left font-mono">
                                        <thead class="text-gray-500 uppercase text-[10px] border-b border-gray-800 bg-gray-900/60">
                                            <tr>
                                                <th class="py-2 px-3">"#"</th>
                                                <th class="py-2 px-3">"Path"</th>
                                                <th class="py-2 px-3">"Duration"</th>
                                                <th class="py-2 px-3">"After"</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-gray-800/40">
                                            {playlist.as_slice().iter().enumerate().map(|(index, entry)| {
                                                let is_active = active_pos == Some(index);
                                                let idx_label = if is_active {
                                                    format!("{index} ▶")
                                                } else {
                                                    index.to_string()
                                                };
                                                let path = entry.path().to_string();
                                                let duration = format_duration(*entry.duration());
                                                let after = format_after_action(*entry.after());

                                                view! {
                                                    <tr class=leptodon::class_list![
                                                        ("bg-oa-blue/10 text-white font-semibold", move || is_active),
                                                        ("text-gray-300", move || !is_active)
                                                    ]>
                                                        <td class="py-2 px-3 text-oa-blue">{idx_label}</td>
                                                        <td class="py-2 px-3 text-white">{path}</td>
                                                        <td class="py-2 px-3 text-gray-400">{duration}</td>
                                                        <td class="py-2 px-3 text-gray-400">{after}</td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                }.into_any(),
                            }}
                        </div>
                    </div>
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(None), "Indefinite");
        assert_eq!(format_duration(Some(Duration::from_millis(500))), "500 ms");
        assert_eq!(
            format_duration(Some(Duration::from_millis(15000))),
            "15s (15000 ms)"
        );
        assert_eq!(
            format_duration(Some(Duration::from_millis(1500))),
            "1500 ms"
        );
    }

    #[test]
    fn test_format_after_action() {
        assert_eq!(
            format_after_action(NextPlaylistEntryAction::Shift(1)),
            "Shift (+1)"
        );
        assert_eq!(
            format_after_action(NextPlaylistEntryAction::Shift(0)),
            "Shift (+0)"
        );
        assert_eq!(
            format_after_action(NextPlaylistEntryAction::Shift(-2)),
            "Shift (-2)"
        );
        assert_eq!(
            format_after_action(NextPlaylistEntryAction::Random),
            "Random"
        );
    }
}
