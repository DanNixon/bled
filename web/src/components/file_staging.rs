use crate::state::{ConnectedDevice, LogLevel, log};
use crate::transport::bridge;
use crate::utils::formatters::{format_bytes, format_hex_rows};
use bled_core::api::{DirEntry, DirEntryType};
use leptodon::button::{Button, ButtonAppearance};
use leptodon::input::TextInput;
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, HtmlInputElement};

struct FileTreeRenderParams<'a, F1, F2, F3>
where
    F1: Fn(String) + Clone + 'static,
    F2: Fn(String) + Clone + 'static,
    F3: Fn(String) + Clone + 'static,
{
    tree_data: &'a HashMap<String, Vec<DirEntry>>,
    expanded: &'a HashSet<String>,
    loading: &'a HashSet<String>,
    selected_path: &'a str,
    toggle_dir: &'a F1,
    select_path: &'a F2,
    read_file: &'a F3,
}

fn file_icon(name: &str) -> &'static str {
    if name.ends_with(".json") {
        "📋"
    } else if name.ends_with(".wasm") {
        "⚙️"
    } else if name.ends_with(".bin") || name.ends_with(".dat") {
        "💾"
    } else if name.ends_with(".txt") || name.ends_with(".md") {
        "📝"
    } else {
        "📄"
    }
}

fn render_tree_dir<F1, F2, F3>(
    dir_path: &str,
    depth: usize,
    params: &FileTreeRenderParams<'_, F1, F2, F3>,
) -> Vec<AnyView>
where
    F1: Fn(String) + Clone + 'static,
    F2: Fn(String) + Clone + 'static,
    F3: Fn(String) + Clone + 'static,
{
    let mut views = Vec::new();

    if let Some(entries) = params.tree_data.get(dir_path) {
        if entries.is_empty() {
            let pad = format!("padding-left: {}rem", 0.75 + depth as f32 * 1.0);
            views.push(
                view! {
                    <div class="text-[11px] text-gray-500 italic py-1" style=pad>
                        "(empty folder)"
                    </div>
                }
                .into_any(),
            );
        } else {
            for entry in entries {
                let name = entry.name().to_string();
                let full_path = if dir_path == "/" {
                    format!("/{name}")
                } else {
                    format!("{}/{}", dir_path.trim_end_matches('/'), name)
                };

                let is_dir = matches!(entry.kind(), DirEntryType::Directory);
                let is_selected = params.selected_path == full_path;

                if is_dir {
                    let is_expanded = params.expanded.contains(&full_path);
                    let is_loading = params.loading.contains(&full_path);
                    let full_path_toggle = full_path.clone();
                    let full_path_select = full_path.clone();
                    let toggle = params.toggle_dir.clone();
                    let select = params.select_path.clone();

                    let pad = format!("padding-left: {}rem", 0.35 + depth as f32 * 0.85);

                    views.push(
                        view! {
                            <div
                                class=leptodon::class_list![
                                    "flex items-center justify-between py-1 px-1.5 rounded-lg cursor-pointer text-xs select-none group transition-all",
                                    ("bg-oa-blue/20 text-oa-blue font-semibold border-l-2 border-oa-blue pl-1.5", move || is_selected),
                                    ("hover:bg-gray-800/80 text-gray-300", move || !is_selected)
                                ]
                                style=pad
                                on:click=move |_| {
                                    select(full_path_select.clone());
                                }
                            >
                                <div class="flex items-center gap-1.5 min-w-0 flex-1">
                                    <button
                                        type="button"
                                        class="text-gray-400 hover:text-white w-4 text-center text-xs shrink-0"
                                        on:click={
                                            let full_path_toggle = full_path_toggle.clone();
                                            let toggle = toggle.clone();
                                            move |e: web_sys::MouseEvent| {
                                                e.stop_propagation();
                                                toggle(full_path_toggle.clone());
                                            }
                                        }
                                    >
                                        {if is_loading {
                                            "⏳"
                                        } else if is_expanded {
                                            "▾"
                                        } else {
                                            "▸"
                                        }}
                                    </button>
                                    <span class="text-xs shrink-0">{if is_expanded { "📂" } else { "📁" }}</span>
                                    <span class="truncate font-medium">{name}</span>
                                </div>
                            </div>
                        }
                        .into_any(),
                    );

                    if is_expanded {
                        let children = render_tree_dir(&full_path, depth + 1, params);
                        views.extend(children);
                    }
                } else {
                    let size = match entry.kind() {
                        DirEntryType::File { size } => *size,
                        _ => 0,
                    };
                    let size_str = format_bytes(size);
                    let full_path_select = full_path.clone();
                    let full_path_read = full_path.clone();
                    let select = params.select_path.clone();
                    let read = params.read_file.clone();
                    let icon = file_icon(&name);

                    let pad = format!("padding-left: {}rem", 0.35 + depth as f32 * 0.85 + 1.1);

                    views.push(
                        view! {
                            <div
                                class=leptodon::class_list![
                                    "flex items-center justify-between py-1 px-1.5 rounded-lg cursor-pointer text-xs group transition-all",
                                    ("bg-oa-blue/20 text-oa-blue font-semibold border-l-2 border-oa-blue pl-1.5", move || is_selected),
                                    ("hover:bg-gray-800/80 text-gray-300", move || !is_selected)
                                ]
                                style=pad
                                on:click=move |_| {
                                    select(full_path_select.clone());
                                }
                                on:dblclick={
                                    let full_path_read = full_path_read.clone();
                                    let read = read.clone();
                                    move |_| {
                                        read(full_path_read.clone());
                                    }
                                }
                            >
                                <div class="flex items-center gap-1.5 min-w-0 flex-1">
                                    <span class="text-xs shrink-0">{icon}</span>
                                    <span class="truncate font-mono">{name}</span>
                                </div>
                                <div class="flex items-center gap-1.5 shrink-0">
                                    <span class="text-[10px] text-gray-500 font-mono">{size_str}</span>
                                    <button
                                        type="button"
                                        class="hidden group-hover:inline-block px-1.5 py-0.5 text-[10px] rounded bg-gray-800 hover:bg-gray-700 text-gray-200 border border-gray-700"
                                        title="Read into Editor"
                                        on:click={
                                            let full_path_read = full_path_read.clone();
                                            let read = read.clone();
                                            move |e: web_sys::MouseEvent| {
                                                e.stop_propagation();
                                                read(full_path_read.clone());
                                            }
                                        }
                                    >
                                        "Read"
                                    </button>
                                </div>
                            </div>
                        }
                        .into_any(),
                    );
                }
            }
        }
    } else if params.loading.contains(dir_path) {
        let pad = format!("padding-left: {}rem", 0.75 + depth as f32 * 0.85);
        views.push(
            view! {
                <div class="text-[11px] text-gray-400 italic py-1 flex items-center gap-1" style=pad>
                    <span>"Loading..."</span>
                </div>
            }
            .into_any(),
        );
    }

    views
}

#[component]
pub fn FileStaging(dev: ConnectedDevice) -> impl IntoView {
    let remote_path_sig = RwSignal::new("/config.json".to_string());
    let progress_sig = RwSignal::new(None::<u8>);
    let editor_text_sig = RwSignal::new(String::new());
    let copied_sig = RwSignal::new(false);

    let tree_data = RwSignal::new(HashMap::<String, Vec<DirEntry>>::new());
    let expanded_dirs = RwSignal::new({
        let mut set = HashSet::new();
        set.insert("/".to_string());
        set
    });
    let loading_dirs = RwSignal::new(HashSet::<String>::new());

    let d_fetch = dev.clone();
    let fetch_dir = move |path: String| {
        let dev = d_fetch.clone();
        loading_dirs.update(|set| {
            set.insert(path.clone());
        });

        let path_clone = path.clone();
        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.list_directory(&path_clone).await {
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
                    tree_data.update(|map| {
                        map.insert(path_clone.clone(), entries);
                    });
                }
                Err(e) => {
                    log(
                        format!("Failed to list directory {path_clone}: {e}"),
                        LogLevel::Error,
                    );
                    tree_data.update(|map| {
                        map.remove(&path_clone);
                    });
                }
            }
            loading_dirs.update(|set| {
                set.remove(&path_clone);
            });
        });
    };

    // Initial load of root directory
    {
        let fetch_dir = fetch_dir.clone();
        Effect::new(move || {
            fetch_dir("/".to_string());
        });
    }

    let toggle_dir = {
        let fetch_dir = fetch_dir.clone();
        move |path: String| {
            let is_open = expanded_dirs.get_untracked().contains(&path);
            if is_open {
                expanded_dirs.update(|set| {
                    set.remove(&path);
                });
            } else {
                expanded_dirs.update(|set| {
                    set.insert(path.clone());
                });
                if !tree_data.get_untracked().contains_key(&path) {
                    fetch_dir(path);
                }
            }
        }
    };

    let refresh_tree = {
        let fetch_dir = fetch_dir.clone();
        move || {
            let expanded = expanded_dirs.get_untracked();
            fetch_dir("/".to_string());
            for dir in expanded {
                if dir != "/" {
                    fetch_dir(dir);
                }
            }
        }
    };

    let d_read = dev.clone();
    let trigger_read_file = {
        let dev = d_read.clone();
        move |path: String| {
            let dev = dev.clone();
            if path.is_empty() {
                log("Please specify a remote path", LogLevel::Error);
                return;
            }

            remote_path_sig.set(path.clone());

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
        }
    };

    let d_write = dev.clone();
    let on_write_file = {
        let refresh_tree = refresh_tree.clone();
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

            let refresh = refresh_tree.clone();
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
                        refresh();
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
        let refresh_tree = refresh_tree.clone();
        move || {
            let dev = d_del.clone();
            let path = remote_path_sig.get_untracked();
            let refresh = refresh_tree.clone();

            spawn_local(async move {
                let mut rpc = dev.rpc.lock().await;
                match rpc.file_delete(&path).await {
                    Ok(()) => {
                        log(format!("[{}] Deleted {path}", dev.name), LogLevel::Success);
                        refresh();
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

    let on_copy_clipboard = {
        let dev = dev.clone();
        move |_| {
            let is_hex = dev.staged_is_hex.get_untracked();
            let text = if is_hex {
                let data = dev.staged_data.get_untracked();
                crate::utils::formatters::render_hex_dump(&data, 8192)
            } else {
                editor_text_sig.get_untracked()
            };

            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = clipboard.write_text(&text);
                copied_sig.set(true);
                log("Copied contents to clipboard", LogLevel::Info);

                let cb = wasm_bindgen::closure::Closure::once(move || {
                    copied_sig.set(false);
                });
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    1800,
                );
                cb.forget();
            }
        }
    };

    let select_path = move |path: String| {
        remote_path_sig.set(path);
    };

    view! {
        <div class="space-y-4">
            // Section Header
            <div class="flex flex-wrap items-center justify-between gap-3 bg-gray-900 border border-gray-800 rounded-xl p-4 shadow-sm">
                <div>
                    <h2 class="text-base font-semibold text-white flex items-center gap-2">
                        <span>"File Manager"</span>
                        <span class="text-xs text-gray-400 font-mono font-normal">"• SD Storage & Staging"</span>
                    </h2>
                    <p class="text-xs text-gray-400">"Browse device storage, inspect, edit, and stage configuration and binaries"</p>
                </div>
                <div class="flex items-center gap-2">
                    <label class="cursor-pointer bg-gray-800 hover:bg-gray-700 text-gray-200 border border-gray-700 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors shadow-sm">
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

            // Main Split: Storage Tree (Left) and Modern File Viewer / Editor (Right)
            <div class="grid grid-cols-1 lg:grid-cols-12 gap-5 items-start">
                // Left Column: Tree File Explorer
                <div class="lg:col-span-5 bg-gray-900 border border-gray-800 rounded-xl p-4 shadow-sm flex flex-col space-y-3">
                    <div class="flex items-center justify-between border-b border-gray-800 pb-2.5">
                        <div class="flex items-center gap-2">
                            <span class="text-xs font-semibold uppercase tracking-wider text-gray-300">
                                "Storage Tree"
                            </span>
                            <span class="text-[10px] px-1.5 py-0.5 rounded bg-gray-800 text-gray-400 font-mono">
                                {move || format!("{} dirs", tree_data.get().len())}
                            </span>
                        </div>
                        <Button appearance=ButtonAppearance::Minimal on_click={
                            let refresh = refresh_tree.clone();
                            move |_| refresh()
                        }>
                            "Refresh"
                        </Button>
                    </div>

                    <div class="h-[460px] overflow-y-auto font-mono py-1 space-y-0.5 border border-gray-800/80 rounded-xl bg-gray-950 p-2.5">
                        // Root /
                        {
                            let toggle_dir = toggle_dir.clone();
                            let trigger_read = trigger_read_file.clone();

                            move || {
                                let data = tree_data.get();
                                let expanded = expanded_dirs.get();
                                let loading = loading_dirs.get();
                                let cur_sel = remote_path_sig.get();

                                let is_root_expanded = expanded.contains("/");
                                let is_root_loading = loading.contains("/");
                                let is_root_sel = cur_sel == "/";

                                let toggle_root = toggle_dir.clone();

                                let params = FileTreeRenderParams {
                                    tree_data: &data,
                                    expanded: &expanded,
                                    loading: &loading,
                                    selected_path: &cur_sel,
                                    toggle_dir: &toggle_dir,
                                    select_path: &select_path,
                                    read_file: &trigger_read,
                                };

                                view! {
                                    <div>
                                        <div
                                            class=leptodon::class_list![
                                                "flex items-center justify-between py-1 px-1.5 rounded-lg cursor-pointer text-xs select-none transition-all",
                                                ("bg-oa-blue/20 text-oa-blue font-semibold border-l-2 border-oa-blue pl-1.5", move || is_root_sel),
                                                ("hover:bg-gray-800/80 text-gray-200", move || !is_root_sel)
                                            ]
                                            on:click=move |_| {
                                                select_path("/".to_string());
                                            }
                                        >
                                            <div class="flex items-center gap-1.5 min-w-0 flex-1">
                                                <button
                                                    type="button"
                                                    class="text-gray-400 hover:text-white w-4 text-center text-xs shrink-0"
                                                    on:click={
                                                        let toggle = toggle_root.clone();
                                                        move |e: web_sys::MouseEvent| {
                                                            e.stop_propagation();
                                                            toggle("/".to_string());
                                                        }
                                                    }
                                                >
                                                    {if is_root_loading {
                                                        "⏳"
                                                    } else if is_root_expanded {
                                                        "▾"
                                                    } else {
                                                        "▸"
                                                    }}
                                                </button>
                                                <span class="text-xs shrink-0">"💾"</span>
                                                <span class="font-bold truncate">"/ (SD Card Root)"</span>
                                            </div>
                                        </div>

                                        {if is_root_expanded {
                                            view! {
                                                <div class="border-l border-gray-800 ml-2.5 space-y-0.5 mt-0.5">
                                                    {render_tree_dir("/", 0, &params)}
                                                </div>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                }
                            }
                        }
                    </div>

                    <p class="text-[11px] text-gray-500 italic">
                        "Double-click any file to read it directly into the viewer."
                    </p>
                </div>

                // Right Column: Modern File Viewer / Staging Editor
                <div class="lg:col-span-7 bg-gray-900 border border-gray-800 rounded-xl p-4 shadow-sm flex flex-col space-y-3.5">
                    // Viewer Header Bar
                    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-gray-800 pb-3">
                        <div class="flex items-center gap-2.5 min-w-0">
                            <span class="text-lg">
                                {move || {
                                    let f = dev.staged_filename.get();
                                    file_icon(&f)
                                }}
                            </span>
                            <div class="min-w-0">
                                <div class="flex items-center gap-2">
                                    <span class="text-sm font-semibold text-white font-mono truncate">
                                        {move || {
                                            let f = dev.staged_filename.get();
                                            if f.is_empty() { "<In-Memory Staging Buffer>".to_string() } else { f }
                                        }}
                                    </span>
                                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-gray-800 border border-gray-700 text-gray-300 font-mono shrink-0">
                                        {move || {
                                            let is_hex = dev.staged_is_hex.get();
                                            let len = if is_hex {
                                                dev.staged_data.get().len()
                                            } else {
                                                editor_text_sig.get().len()
                                            };
                                            format_bytes(len as u64)
                                        }}
                                    </span>
                                </div>
                            </div>
                        </div>

                        <div class="flex items-center gap-2 shrink-0">
                            <Button appearance=ButtonAppearance::Minimal on_click=on_copy_clipboard>
                                {move || if copied_sig.get() { "Copied ✓" } else { "Copy" }}
                            </Button>
                            <Button appearance=ButtonAppearance::Secondary on_click={
                                let dev = dev.clone();
                                move |_| {
                                    let cur = dev.staged_is_hex.get_untracked();
                                    dev.staged_is_hex.set(!cur);
                                }
                            }>
                                {move || if dev.staged_is_hex.get() { "Switch to Text" } else { "Switch to Hex" }}
                            </Button>
                        </div>
                    </div>

                    // Remote Actions Toolbar
                    <div class="grid grid-cols-1 sm:grid-cols-12 gap-2.5 items-end">
                        <div class="sm:col-span-6">
                            <TextInput label="Device File Path" value=remote_path_sig />
                        </div>
                        <div class="sm:col-span-6 flex items-center justify-end gap-1.5">
                            <Button appearance=ButtonAppearance::Primary on_click={
                                let trigger_read = trigger_read_file.clone();
                                move |_| {
                                    let path = remote_path_sig.get_untracked();
                                    trigger_read(path);
                                }
                            }>
                                "Read"
                            </Button>
                            <Button appearance=ButtonAppearance::Primary on_click=move |_| on_write_file()>
                                "Write"
                            </Button>
                            <Button appearance=ButtonAppearance::Danger on_click=move |_| on_delete_file()>
                                "Delete"
                            </Button>
                        </div>
                    </div>

                    // Progress indicator
                    {move || {
                        if let Some(pct) = progress_sig.get() {
                            view! {
                                <div class="w-full bg-gray-800 rounded-full h-1.5 overflow-hidden">
                                    <div class="bg-oa-blue h-1.5 transition-all duration-150" style=format!("width: {pct}%")></div>
                                </div>
                            }.into_any()
                        } else {
                            ().into_any()
                        }
                    }}

                    // Modern Content Viewer Container
                    <div class="border border-gray-800 rounded-xl overflow-hidden bg-gray-950 flex flex-col">
                        // Viewer Mode Bar
                        <div class="flex items-center justify-between px-3 py-1.5 bg-gray-900/90 border-b border-gray-800 text-[11px] text-gray-400 font-mono">
                            <div class="flex items-center gap-2">
                                <span class="w-2 h-2 rounded-full bg-emerald-500 inline-block"></span>
                                <span>{move || if dev.staged_is_hex.get() { "BINARY HEX DUMP" } else { "UTF-8 CODE EDITOR" }}</span>
                            </div>
                            <div class="flex items-center gap-3">
                                <span>{move || {
                                    if dev.staged_is_hex.get() {
                                        let data = dev.staged_data.get();
                                        format!("{} bytes", data.len())
                                    } else {
                                        let text = editor_text_sig.get();
                                        let lines = text.lines().count().max(1);
                                        format!("{lines} lines • {} chars", text.len())
                                    }
                                }}</span>
                            </div>
                        </div>

                        // Main Content Area
                        {move || {
                            let is_hex = dev.staged_is_hex.get();
                            let data = dev.staged_data.get();
                            let text = editor_text_sig.get();

                            if is_hex && data.is_empty() && text.is_empty() || !is_hex && text.is_empty() && data.is_empty() {
                                view! {
                                    <div class="h-80 flex flex-col items-center justify-center p-6 text-center space-y-2">
                                        <span class="text-3xl text-gray-600">"📄"</span>
                                        <p class="text-sm font-medium text-gray-400">"No file staged in buffer"</p>
                                        <p class="text-xs text-gray-600 max-w-sm">
                                            "Select a file from the Storage Tree and click Read, load from disk, or type directly into the editor."
                                        </p>
                                    </div>
                                }.into_any()
                            } else if is_hex {
                                let (rows, truncated_bytes) = format_hex_rows(&data, 8192);
                                view! {
                                    <div class="h-96 overflow-auto font-mono text-[11px] p-2.5 space-y-0.5 select-text">
                                        <div class="grid grid-cols-12 gap-2 text-gray-500 border-b border-gray-800/80 pb-1 mb-1 font-semibold text-[10px] uppercase tracking-wider">
                                            <div class="col-span-2 text-cyan-400/90">"Offset"</div>
                                            <div class="col-span-4 text-gray-400">"00 01 02 03 04 05 06 07"</div>
                                            <div class="col-span-4 text-gray-400">"08 09 0A 0B 0C 0D 0E 0F"</div>
                                            <div class="col-span-2 text-emerald-400/90 border-l border-gray-800 pl-2">"ASCII"</div>
                                        </div>
                                        {rows.into_iter().map(|row| {
                                            view! {
                                                <div class="grid grid-cols-12 gap-2 py-0.5 hover:bg-gray-900/60 rounded px-1 transition-colors">
                                                    <div class="col-span-2 text-cyan-400/80 font-semibold select-none">{row.offset_str}</div>
                                                    <div class="col-span-4 text-gray-300 font-mono tracking-wider">{row.hex_left}</div>
                                                    <div class="col-span-4 text-gray-400 font-mono tracking-wider">{row.hex_right}</div>
                                                    <div class="col-span-2 text-emerald-400 font-mono border-l border-gray-800 pl-2 truncate">{row.ascii}</div>
                                                </div>
                                            }
                                        }).collect_view()}

                                        {if truncated_bytes > 0 {
                                            view! {
                                                <div class="text-center py-2 text-amber-500/80 text-xs italic border-t border-gray-800 mt-2">
                                                    {format!("... ({} more bytes truncated in preview)", truncated_bytes)}
                                                </div>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                }.into_any()
                            } else {
                                let lines_count = text.lines().count().max(1);
                                let line_numbers = (1..=lines_count)
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                view! {
                                    <div class="flex h-96 relative bg-gray-950 font-mono text-xs">
                                        // Line numbers gutter
                                        <div class="select-none py-3 px-2.5 text-right text-gray-600 bg-gray-950 border-r border-gray-800/80 font-mono text-[11px] leading-relaxed w-10 shrink-0 overflow-hidden">
                                            <pre>{line_numbers}</pre>
                                        </div>
                                        // Text editor
                                        <textarea
                                            class="flex-1 bg-transparent p-3 text-gray-200 text-xs font-mono leading-relaxed focus:outline-none resize-none overflow-auto"
                                            bind:value=editor_text_sig
                                            placeholder="File contents will appear here. Type directly, read from device, or load from disk..."
                                            spellcheck="false"
                                        ></textarea>
                                    </div>
                                }.into_any()
                            }
                        }}
                    </div>
                </div>
            </div>
        </div>
    }
}