use crate::state::{ConnectedDevice, LogLevel, log};
use crate::transport::bridge;
use crate::utils::formatters::{format_bytes, render_hex_dump};
use bled_core::api::{DirEntry, DirEntryType};
use leptodon::button::{Button, ButtonAppearance};
use leptodon::input::TextInput;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, HtmlInputElement};

#[component]
pub fn FileStaging(dev: ConnectedDevice) -> impl IntoView {
    let dir_path_sig = RwSignal::new("/".to_string());
    let remote_path_sig = RwSignal::new("/config.json".to_string());
    let is_listing_sig = RwSignal::new(false);
    let progress_sig = RwSignal::new(None::<u8>);

    let editor_text_sig = RwSignal::new(String::new());

    let dir_entries_sig = RwSignal::new(Vec::<DirEntry>::new());

    let d_list = dev.clone();
    let load_directory = move || {
        let dev = d_list.clone();
        let path = dir_path_sig.get_untracked();
        is_listing_sig.set(true);

        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.list_directory(&path).await {
                Ok(mut entries) => {
                    entries.sort_by(|a, b| {
                        let a_is_dir = matches!(a.kind(), DirEntryType::Directory);
                        let b_is_dir = matches!(b.kind(), DirEntryType::Directory);
                        match (a_is_dir, b_is_dir) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name().cmp(b.name()),
                        }
                    });
                    dir_entries_sig.set(entries);
                }
                Err(e) => {
                    log(format!("Failed to list directory: {e}"), LogLevel::Error);
                    dir_entries_sig.set(Vec::new());
                }
            }
            is_listing_sig.set(false);
        });
    };

    Effect::new({
        let load_directory = load_directory.clone();
        move || {
            let _ = dir_path_sig.get();
            load_directory();
        }
    });

    let on_dir_up = move |_| {
        let cur = dir_path_sig.get_untracked();
        let norm = cur.trim_end_matches('/');
        let parent = match norm.rfind('/') {
            Some(idx) if idx > 0 => &norm[..idx],
            _ => "/",
        };
        dir_path_sig.set(parent.to_string());
    };

    let d_read = dev.clone();
    let trigger_read_file = move || {
        let dev = d_read.clone();
        let path = remote_path_sig.get_untracked();
        if path.is_empty() {
            log("Please specify a remote path", LogLevel::Error);
            return;
        }

        spawn_local(async move {
            progress_sig.set(Some(5));
            let mut rpc = dev.rpc.lock().await;
            match rpc
                .read_file_chunked(&path, Some(|pct| progress_sig.set(Some(pct))))
                .await
            {
                Ok(bytes) => {
                    let filename = path.split('/').next_back().unwrap_or("file").to_string();
                    dev.staged_filename.set(filename);
                    dev.staged_data.set(bytes.clone());

                    if let Ok(text) = String::from_utf8(bytes) {
                        editor_text_sig.set(text);
                        dev.staged_is_hex.set(false);
                    } else {
                        dev.staged_is_hex.set(true);
                    }
                    log(format!("[{}] Read {path}", dev.name), LogLevel::Success);
                }
                Err(e) => log(
                    format!("[{}] Failed reading file: {e}", dev.name),
                    LogLevel::Error,
                ),
            }
            progress_sig.set(None);
        });
    };

    let d_write = dev.clone();
    let on_write_file = {
        let load_directory = load_directory.clone();
        move || {
            let dev = d_write.clone();
            let path = remote_path_sig.get_untracked();
            if path.is_empty() {
                log("Please specify a remote path to write", LogLevel::Error);
                return;
            }

            let is_hex = dev.staged_is_hex.get_untracked();
            let bytes = if is_hex {
                dev.staged_data.get_untracked()
            } else {
                editor_text_sig.get_untracked().into_bytes()
            };

            if bytes.is_empty() {
                log("Staging buffer is empty", LogLevel::Error);
                return;
            }

            let load_dir = load_directory.clone();
            spawn_local(async move {
                progress_sig.set(Some(5));
                let mut rpc = dev.rpc.lock().await;
                match rpc
                    .write_file_chunked(&path, &bytes, Some(|pct| progress_sig.set(Some(pct))))
                    .await
                {
                    Ok(()) => {
                        log(
                            format!("[{}] Successfully wrote {path}", dev.name),
                            LogLevel::Success,
                        );
                        load_dir();
                    }
                    Err(e) => log(
                        format!("[{}] Failed writing file: {e}", dev.name),
                        LogLevel::Error,
                    ),
                }
                progress_sig.set(None);
            });
        }
    };

    let d_del = dev.clone();
    let on_delete_file = {
        let load_directory = load_directory.clone();
        move || {
            let dev = d_del.clone();
            let path = remote_path_sig.get_untracked();
            let load_dir = load_directory.clone();

            spawn_local(async move {
                let mut rpc = dev.rpc.lock().await;
                match rpc.file_delete(&path).await {
                    Ok(()) => {
                        log(format!("[{}] Deleted {path}", dev.name), LogLevel::Success);
                        load_dir();
                    }
                    Err(e) => log(
                        format!("[{}] Failed deleting file: {e}", dev.name),
                        LogLevel::Error,
                    ),
                }
            });
        }
    };

    let on_disk_load = {
        let dev = dev.clone();
        move |e: Event| {
            if let Some(input) = e
                .target()
                .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                && let Some(files) = input.files()
                && let Some(file) = files.get(0)
            {
                let fname = file.name();
                let promise = file.array_buffer();
                let dev = dev.clone();

                spawn_local(async move {
                    if let Ok(ab) = JsFuture::from(promise).await {
                        let u8_arr = js_sys::Uint8Array::new(&ab);
                        let bytes = u8_arr.to_vec();
                        dev.staged_filename.set(fname.clone());
                        remote_path_sig.set(format!("/{fname}"));
                        dev.staged_data.set(bytes.clone());

                        if let Ok(text) = String::from_utf8(bytes) {
                            editor_text_sig.set(text);
                            dev.staged_is_hex.set(false);
                        } else {
                            dev.staged_is_hex.set(true);
                        }
                        log(format!("Loaded \"{fname}\" from disk"), LogLevel::Success);
                    }
                });
            }
        }
    };

    let d_save = dev.clone();
    let on_save_disk = move |_| {
        let is_hex = d_save.staged_is_hex.get_untracked();
        let bytes = if is_hex {
            d_save.staged_data.get_untracked()
        } else {
            editor_text_sig.get_untracked().into_bytes()
        };

        if !bytes.is_empty() {
            let fname = d_save.staged_filename.get_untracked();
            let default_name = if !fname.is_empty() {
                fname
            } else {
                "staged_file.bin".into()
            };
            bridge::save_file_to_disk(&bytes, &default_name);
        }
    };

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-gray-800 pb-3">
                <div>
                    <h3 class="text-base font-semibold text-white">"File Management"</h3>
                    <p class="text-xs text-gray-400">"Buffer between files on disk and files on device"</p>
                </div>
                <div class="flex items-center gap-2">
                    <label class="cursor-pointer bg-gray-800 hover:bg-gray-700 text-gray-200 border border-gray-700 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors">
                        "Load from Disk"
                        <input type="file" on:change=on_disk_load class="hidden" />
                    </label>
                    <Button appearance=ButtonAppearance::Secondary on_click=on_save_disk>
                        "Save to Disk"
                    </Button>
                    <Button appearance=ButtonAppearance::Secondary on_click={
                        let dev = dev.clone();
                        move |_| {
                            dev.staged_data.set(Vec::new());
                            dev.staged_filename.set(String::new());
                            editor_text_sig.set(String::new());
                        }
                    }>
                        "Clear Buffer"
                    </Button>
                </div>
            </div>

            // Directory Browser
            <div class="space-y-2 bg-gray-950/60 p-3.5 rounded-xl border border-gray-800">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <div class="flex items-center gap-2 flex-1 min-w-[240px]">
                        <span class="text-xs font-medium text-gray-400">"Directory:"</span>
                        <div class="w-48">
                            <TextInput value=dir_path_sig />
                        </div>
                        <Button appearance=ButtonAppearance::Secondary on_click={
                            let load_dir = load_directory.clone();
                            move |_| load_dir()
                        }>
                            "Refresh"
                        </Button>
                        <Button appearance=ButtonAppearance::Secondary on_click=on_dir_up>
                            "⬆ Up"
                        </Button>
                    </div>
                </div>

                <div class="max-h-56 overflow-auto border border-gray-800 rounded-lg">
                    <table class="w-full text-left text-xs text-gray-300">
                        <thead class="bg-gray-800/80 text-gray-400 uppercase text-[10px] sticky top-0">
                            <tr>
                                <th class="p-2 w-8"></th>
                                <th class="p-2">"Name"</th>
                                <th class="p-2 w-24">"Size"</th>
                                <th class="p-2 w-32 text-right">"Actions"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-gray-800/60 font-mono">
                            {
                                let trigger_read = trigger_read_file.clone();
                                move || {
                                    let entries = dir_entries_sig.get();
                                    let cur_dir = dir_path_sig.get();

                                    if entries.is_empty() {
                                        view! {
                                            <tr>
                                                <td colspan="4" class="p-4 text-center text-gray-500 italic">
                                                    {if is_listing_sig.get() { "Listing directory..." } else { "Directory is empty." }}
                                                </td>
                                            </tr>
                                        }.into_any()
                                    } else {
                                        entries.into_iter().map(|entry| {
                                            let name = entry.name().to_string();
                                            let is_dir = matches!(entry.kind(), DirEntryType::Directory);
                                            let size_str = match entry.kind() {
                                                DirEntryType::File { size } => format_bytes(*size),
                                                DirEntryType::Directory => "—".into(),
                                            };

                                            let full_path = if cur_dir == "/" {
                                                format!("/{name}")
                                            } else {
                                                format!("{}/{}", cur_dir.trim_end_matches('/'), name)
                                            };
                                            let full_path_sel = full_path.clone();
                                            let full_path_read = full_path.clone();
                                            let trigger_read = trigger_read.clone();

                                            view! {
                                                <tr class="hover:bg-gray-800/40 transition-colors">
                                                    <td class="p-2 text-center">{if is_dir { "📁" } else { "📄" }}</td>
                                                    <td class="p-2 font-medium text-white">{name}</td>
                                                    <td class="p-2 text-gray-400">{size_str}</td>
                                                    <td class="p-2 text-right">
                                                        <div class="flex items-center justify-end gap-1">
                                                            {if is_dir {
                                                                let full_path = full_path.clone();
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="px-2 py-0.5 text-[11px] rounded bg-gray-800 hover:bg-gray-700 text-gray-200"
                                                                        on:click=move |_| {
                                                                            dir_path_sig.set(full_path.clone());
                                                                        }
                                                                    >
                                                                        "Open"
                                                                    </button>
                                                                }.into_any()
                                                            } else {
                                                                view! {
                                                                    <>
                                                                        <button
                                                                            type="button"
                                                                            class="px-2 py-0.5 text-[11px] rounded bg-gray-800 hover:bg-gray-700 text-gray-200"
                                                                            on:click=move |_| {
                                                                                remote_path_sig.set(full_path_sel.clone());
                                                                            }
                                                                        >
                                                                            "Select"
                                                                        </button>
                                                                        <button
                                                                            type="button"
                                                                            class="px-2 py-0.5 text-[11px] rounded bg-oa-blue/20 hover:bg-oa-blue/30 text-oa-blue font-semibold"
                                                                            on:click=move |_| {
                                                                                remote_path_sig.set(full_path_read.clone());
                                                                                trigger_read();
                                                                            }
                                                                        >
                                                                            "Read"
                                                                        </button>
                                                                    </>
                                                                }.into_any()
                                                            }}
                                                        </div>
                                                    </td>
                                                </tr>
                                            }
                                        }).collect_view().into_any()
                                    }
                                }
                            }
                        </tbody>
                    </table>
                </div>
            </div>

            // Remote Path Actions Toolbar
            <div class="flex flex-wrap items-end justify-between gap-3 pt-2 border-t border-gray-800">
                <div class="flex-1 min-w-[260px]">
                    <TextInput label="Remote Path on Device" value=remote_path_sig />
                </div>
                <div class="flex items-center gap-2">
                    <Button appearance=ButtonAppearance::Primary on_click=move |_| trigger_read_file()>
                        "Read from Device"
                    </Button>
                    <Button appearance=ButtonAppearance::Primary on_click=move |_| on_write_file()>
                        "Write to Device"
                    </Button>
                    <Button appearance=ButtonAppearance::Danger on_click=move |_| on_delete_file()>
                        "Delete"
                    </Button>
                </div>
            </div>

            {move || {
                if let Some(pct) = progress_sig.get() {
                    view! {
                        <div class="w-full bg-gray-800 rounded-full h-2 overflow-hidden">
                            <div class="bg-oa-blue h-2 transition-all duration-200" style=format!("width: {pct}%")></div>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            // Editor / Staging Viewer
            <div class="space-y-2 pt-2 border-t border-gray-800">
                <div class="flex items-center justify-between text-xs text-gray-400">
                    <div class="flex items-center gap-3">
                        <span>"Staged: " <strong class="text-white">{move || {
                            let f = dev.staged_filename.get();
                            if f.is_empty() { "<In-Memory Buffer>".into() } else { f }
                        }}</strong></span>
                        <span>"Size: " <strong class="text-white">{move || {
                            let d = dev.staged_data.get();
                            format_bytes(d.len() as u64)
                        }}</strong></span>
                        <span>"Mode: " <strong class="text-oa-blue">{move || if dev.staged_is_hex.get() { "Binary (Hex View)" } else { "Text (UTF-8)" }}</strong></span>
                    </div>
                    <Button appearance=ButtonAppearance::Secondary on_click={
                        let dev = dev.clone();
                        move |_| {
                            let cur = dev.staged_is_hex.get_untracked();
                            dev.staged_is_hex.set(!cur);
                        }
                    }>
                        {move || if dev.staged_is_hex.get() { "View Text" } else { "View Hex" }}
                    </Button>
                </div>

                {move || {
                    let is_hex = dev.staged_is_hex.get();
                    if is_hex {
                        let data = dev.staged_data.get();
                        view! {
                            <pre class="w-full bg-gray-950 p-3 rounded-lg border border-gray-800 text-xs font-mono text-gray-300 max-h-80 overflow-auto">
                                {render_hex_dump(&data, 4096)}
                            </pre>
                        }.into_any()
                    } else {
                        view! {
                            <textarea
                                class="w-full bg-gray-950 p-3 rounded-lg border border-gray-800 text-xs font-mono text-gray-200 h-64 focus:outline-none focus:ring-1 focus:ring-oa-blue"
                                bind:value=editor_text_sig
                                placeholder="File contents will appear here. Load from disk, read from device, or type directly..."
                            ></textarea>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
