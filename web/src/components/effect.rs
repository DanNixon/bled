use crate::state::{ConnectedDevice, LogLevel, log};
use crate::utils::formatters::{bytes_to_hex, parse_hex};
use bled_core::api::DirEntryType;
use leptodon::button::{Button, ButtonAppearance};
use leptodon::input::TextInput;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn EffectPanel(dev: ConnectedDevice) -> impl IntoView {
    let path_sig = RwSignal::new(dev.active_effect_path.get_untracked());
    let dir_sig = RwSignal::new("/".to_string());
    let wasm_modules_sig = RwSignal::new(Vec::<String>::new());
    let is_browsing_sig = RwSignal::new(false);

    let cmd_text_sig = RwSignal::new(bytes_to_hex(&dev.effect_command_bytes.get_untracked()));
    let param_text_sig = RwSignal::new(bytes_to_hex(&dev.effect_param_bytes.get_untracked()));

    // Sync when device active effect and buffers update
    Effect::watch(
        {
            let dev = dev.clone();
            move || (
                dev.active_effect_path.get(),
                dev.effect_command_bytes.get(),
                dev.effect_param_bytes.get(),
            )
        },
        move |(path, cmd_bytes, param_bytes), _, _| {
            path_sig.set(path.clone());
            cmd_text_sig.set(bytes_to_hex(cmd_bytes));
            param_text_sig.set(bytes_to_hex(param_bytes));
        },
        false,
    );

    let d_refresh = dev.clone();
    let on_refresh = move |_| {
        let dev = d_refresh.clone();
        spawn_local(async move {
            dev.refresh_effect().await;
        });
    };

    let d_load = dev.clone();
    let on_load_effect = move |_| {
        let dev = d_load.clone();
        let path = path_sig.get_untracked();
        if path.is_empty() {
            log(
                "Enter a Wasm module path before loading an effect.",
                LogLevel::Error,
            );
            return;
        }

        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.set_active_effect(Some(&path)).await {
                Ok(()) => {
                    log(format!("Loaded effect \"{path}\""), LogLevel::Success);
                    dev.refresh_effect().await;
                }
                Err(e) => log(format!("Failed loading effect: {e}"), LogLevel::Error),
            }
        });
    };

    let d_unload = dev.clone();
    let on_unload_effect = move |_| {
        let dev = d_unload.clone();
        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.set_active_effect(None).await {
                Ok(()) => {
                    log("Unloaded active effect", LogLevel::Success);
                    dev.refresh_effect().await;
                }
                Err(e) => log(format!("Failed unloading effect: {e}"), LogLevel::Error),
            }
        });
    };

    let d_browse = dev.clone();
    let on_browse_modules = move |_| {
        let dev = d_browse.clone();
        let dir = dir_sig.get_untracked();
        is_browsing_sig.set(true);

        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.list_directory(&dir).await {
                Ok(entries) => {
                    let wasms: Vec<String> = entries
                        .into_iter()
                        .filter(|e| {
                            matches!(e.kind(), DirEntryType::File { .. })
                                && e.name().ends_with(".wasm")
                        })
                        .map(|e| {
                            if dir == "/" {
                                format!("/{}", e.name())
                            } else {
                                format!("{}/{}", dir.trim_end_matches('/'), e.name())
                            }
                        })
                        .collect();
                    wasm_modules_sig.set(wasms);
                }
                Err(e) => log(
                    format!("Failed browsing Wasm modules: {e}"),
                    LogLevel::Error,
                ),
            }
            is_browsing_sig.set(false);
        });
    };

    let d_read_cmd = dev.clone();
    let on_read_cmd = move |_| {
        let dev = d_read_cmd.clone();
        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.effect_command_read().await {
                Ok(bytes) => {
                    dev.effect_command_bytes.set(bytes.clone());
                    log(
                        format!("Read {} command byte(s)", bytes.len()),
                        LogLevel::Success,
                    );
                }
                Err(e) => log(
                    format!("Failed reading command buffer: {e}"),
                    LogLevel::Error,
                ),
            }
        });
    };

    let d_write_cmd = dev.clone();
    let on_write_cmd = move |_| {
        let dev = d_write_cmd.clone();
        let text = cmd_text_sig.get_untracked();
        match parse_hex(&text) {
            Ok(bytes) => {
                spawn_local(async move {
                    let mut rpc = dev.rpc.lock().await;
                    match rpc.effect_command_write(&bytes).await {
                        Ok(()) => log(
                            format!("Wrote {} command byte(s)", bytes.len()),
                            LogLevel::Success,
                        ),
                        Err(e) => log(
                            format!("Failed writing command buffer: {e}"),
                            LogLevel::Error,
                        ),
                    }
                });
            }
            Err(e) => log(e, LogLevel::Error),
        }
    };

    let d_read_param = dev.clone();
    let on_read_param = move |_| {
        let dev = d_read_param.clone();
        spawn_local(async move {
            let mut rpc = dev.rpc.lock().await;
            match rpc.effect_parameter_read().await {
                Ok(bytes) => {
                    dev.effect_param_bytes.set(bytes.clone());
                    log(
                        format!("Read {} parameter byte(s)", bytes.len()),
                        LogLevel::Success,
                    );
                }
                Err(e) => log(
                    format!("Failed reading parameter buffer: {e}"),
                    LogLevel::Error,
                ),
            }
        });
    };

    let d_write_param = dev.clone();
    let on_write_param = move |_| {
        let dev = d_write_param.clone();
        let text = param_text_sig.get_untracked();
        match parse_hex(&text) {
            Ok(bytes) => {
                spawn_local(async move {
                    let mut rpc = dev.rpc.lock().await;
                    match rpc.effect_parameter_write(&bytes).await {
                        Ok(()) => log(
                            format!("Wrote {} parameter byte(s)", bytes.len()),
                            LogLevel::Success,
                        ),
                        Err(e) => log(
                            format!("Failed writing parameter buffer: {e}"),
                            LogLevel::Error,
                        ),
                    }
                });
            }
            Err(e) => log(e, LogLevel::Error),
        }
    };

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="flex items-center justify-between border-b border-gray-800 pb-3">
                <div>
                    <h3 class="text-base font-semibold text-white">"Effects"</h3>
                    <p class="text-xs text-gray-400">"Load a Wasm effect module and exchange ABI buffers"</p>
                </div>
                <Button appearance=ButtonAppearance::Secondary on_click=on_refresh>
                    "Refresh"
                </Button>
            </div>

            // Wasm path bar
            <div class="flex flex-wrap items-end gap-3">
                <div class="flex-1 min-w-[240px]">
                    <TextInput label="Wasm module path" placeholder="/effects/demo/demo.wasm" value=path_sig />
                </div>
                <Button appearance=ButtonAppearance::Primary on_click=on_load_effect>
                    "Load effect"
                </Button>
                <Button appearance=ButtonAppearance::Secondary on_click=on_unload_effect>
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
                        <Button appearance=ButtonAppearance::Secondary on_click=on_browse_modules>
                            "List Wasm modules"
                        </Button>
                    </div>
                    <span class="text-xs text-oa-blue font-mono">
                        {move || {
                            let act = dev.active_effect_path.get();
                            if act.is_empty() {
                                "No active effect loaded.".into()
                            } else if let Some(m) = dev.active_effect_manifest.get() {
                                format!("{} — Active: {}", m.name().as_str(), act)
                            } else {
                                format!("Active: {act}")
                            }
                        }}
                    </span>
                </div>

                <div class="flex flex-wrap gap-2 pt-1">
                    {move || {
                        let modules = wasm_modules_sig.get();
                        if modules.is_empty() {
                            view! {
                                <span class="text-xs text-gray-500 italic">
                                    {if is_browsing_sig.get() { "Searching..." } else { "Browse a directory to find Wasm modules." }}
                                </span>
                            }.into_any()
                        } else {
                            modules.into_iter().map(|m_path| {
                                let m_click = m_path.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="px-2.5 py-1 rounded bg-gray-800 hover:bg-gray-700 text-xs font-mono text-gray-300 border border-gray-700 transition-colors"
                                        on:click=move |_| path_sig.set(m_click.clone())
                                    >
                                        {m_path}
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </div>
            </div>

            // Manifest descriptor tables
            {move || {
                if let Some(manifest) = dev.active_effect_manifest.get() {
                    view! {
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2">
                            <div class="bg-gray-950/40 p-3 rounded-lg border border-gray-800 text-xs">
                                <span class="font-semibold text-gray-300 block mb-2">"Command Fields"</span>
                                <table class="w-full text-left font-mono">
                                    <thead class="text-gray-500 uppercase text-[10px] border-b border-gray-800">
                                        <tr><th class="pb-1">"Field"</th><th class="pb-1">"Start"</th><th class="pb-1">"End"</th></tr>
                                    </thead>
                                    <tbody class="divide-y divide-gray-800/40">
                                        {manifest.command_descriptors().iter().map(|d| {
                                            let name = d.name().to_string();
                                            let start = *d.start();
                                            let end = *d.end();
                                            view! {
                                                <tr>
                                                    <td class="py-1 text-white">{name}</td>
                                                    <td class="py-1 text-gray-400">{start}</td>
                                                    <td class="py-1 text-gray-400">{end}</td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            </div>

                            <div class="bg-gray-950/40 p-3 rounded-lg border border-gray-800 text-xs">
                                <span class="font-semibold text-gray-300 block mb-2">"Parameter Fields"</span>
                                <table class="w-full text-left font-mono">
                                    <thead class="text-gray-500 uppercase text-[10px] border-b border-gray-800">
                                        <tr><th class="pb-1">"Field"</th><th class="pb-1">"Start"</th><th class="pb-1">"End"</th></tr>
                                    </thead>
                                    <tbody class="divide-y divide-gray-800/40">
                                        {manifest.parameter_descriptors().iter().map(|d| {
                                            let name = d.name().to_string();
                                            let start = *d.start();
                                            let end = *d.end();
                                            view! {
                                                <tr>
                                                    <td class="py-1 text-white">{name}</td>
                                                    <td class="py-1 text-gray-400">{start}</td>
                                                    <td class="py-1 text-gray-400">{end}</td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            // Buffers
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2 border-t border-gray-800">
                <div class="space-y-2">
                    <div class="flex items-center justify-between">
                        <label class="text-xs font-semibold text-gray-300">
                            {move || format!("Command Buffer ({} B)", dev.effect_command_len.get().map(|l| l.to_string()).unwrap_or_else(|| "—".into()))}
                        </label>
                    </div>
                    <textarea
                        class="w-full bg-gray-950 p-2.5 rounded-lg border border-gray-800 text-xs font-mono text-gray-200 h-28 focus:outline-none focus:ring-1 focus:ring-oa-blue"
                        bind:value=cmd_text_sig
                        placeholder="Hex bytes, e.g. 01 02 ff"
                    ></textarea>
                    <div class="flex items-center gap-2">
                        <Button appearance=ButtonAppearance::Secondary on_click=on_read_cmd>
                            "Read Command"
                        </Button>
                        <Button appearance=ButtonAppearance::Primary on_click=on_write_cmd>
                            "Write Command"
                        </Button>
                    </div>
                </div>

                <div class="space-y-2">
                    <div class="flex items-center justify-between">
                        <label class="text-xs font-semibold text-gray-300">
                            {move || format!("Parameter Buffer ({} B)", dev.effect_param_len.get().map(|l| l.to_string()).unwrap_or_else(|| "—".into()))}
                        </label>
                    </div>
                    <textarea
                        class="w-full bg-gray-950 p-2.5 rounded-lg border border-gray-800 text-xs font-mono text-gray-200 h-28 focus:outline-none focus:ring-1 focus:ring-oa-blue"
                        bind:value=param_text_sig
                        placeholder="Hex bytes, e.g. 01 02 ff"
                    ></textarea>
                    <div class="flex items-center gap-2">
                        <Button appearance=ButtonAppearance::Secondary on_click=on_read_param>
                            "Read Parameters"
                        </Button>
                        <Button appearance=ButtonAppearance::Primary on_click=on_write_param>
                            "Write Parameters"
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    }
}
