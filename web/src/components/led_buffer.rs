use crate::state::{ConnectedDevice, LogLevel, log};
use crate::transport::bridge;
use crate::utils::formatters::{format_bytes, render_hex_dump};
use leptodon::button::{Button, ButtonAppearance};
use leptodon::input::NumberInput;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, HtmlInputElement};

#[component]
pub fn LedBuffer(dev: ConnectedDevice) -> impl IntoView {
    let d_info = dev.clone();
    let d_read = dev.clone();
    let d_write = dev.clone();
    let d_dl = dev.clone();

    let offset_sig = RwSignal::new(0u32);
    let progress_sig = RwSignal::new(None::<u8>);

    let on_check_info = move || {
        let dev = d_info.clone();
        spawn_local(async move {
            dev.refresh_led_buf_info().await;
        });
    };

    let on_read_buffer = move || {
        let dev = d_read.clone();
        spawn_local(async move {
            dev.refresh_led_buf_info().await;
            let size = dev.led_buf_size.get_untracked().unwrap_or(0);
            if size == 0 {
                log("LED buffer is empty or size is 0", LogLevel::Error);
                return;
            }

            progress_sig.set(Some(0));
            let mut rpc = dev.rpc.lock().await;
            match rpc
                .read_led_buffer_chunked(0, size, Some(|pct| progress_sig.set(Some(pct))))
                .await
            {
                Ok(bytes) => {
                    dev.led_buf_dump.set(bytes.clone());
                    log(
                        format!("Read {} bytes from LED buffer", bytes.len()),
                        LogLevel::Success,
                    );
                }
                Err(e) => log(format!("Failed reading LED buffer: {e}"), LogLevel::Error),
            }
            progress_sig.set(None);
        });
    };

    let on_download = move || {
        let data = d_dl.led_buf_dump.get();
        if !data.is_empty() {
            bridge::save_file_to_disk(&data, "led_buffer_dump.bin");
        }
    };

    let staged_file_bytes = RwSignal::new(Vec::<u8>::new());
    let on_file_selected = move |e: Event| {
        if let Some(input) = e
            .target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
            && let Some(files) = input.files()
            && let Some(file) = files.get(0)
        {
            let promise = file.array_buffer();
            spawn_local(async move {
                if let Ok(ab) = JsFuture::from(promise).await {
                    let u8_arr = js_sys::Uint8Array::new(&ab);
                    staged_file_bytes.set(u8_arr.to_vec());
                }
            });
        }
    };

    let on_upload_buffer = move || {
        let dev = d_write.clone();
        let bytes = staged_file_bytes.get_untracked();
        let offset = offset_sig.get_untracked();
        if bytes.is_empty() {
            log("Please select a .bin file to upload", LogLevel::Error);
            return;
        }

        spawn_local(async move {
            progress_sig.set(Some(5));
            let mut rpc = dev.rpc.lock().await;
            match rpc
                .write_led_buffer_chunked(offset, &bytes, Some(|pct| progress_sig.set(Some(pct))))
                .await
            {
                Ok(()) => {
                    log(
                        format!("Uploaded {} bytes to LED buffer", bytes.len()),
                        LogLevel::Success,
                    );
                    let _ = dev.refresh_led_buf_info().await;
                }
                Err(e) => log(
                    format!("Failed writing to LED buffer: {e}"),
                    LogLevel::Error,
                ),
            }
            progress_sig.set(None);
        });
    };

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="flex items-center justify-between border-b border-gray-800 pb-3">
                <div>
                    <h3 class="text-base font-semibold text-white">"LED Data Buffer"</h3>
                    <p class="text-xs text-gray-400">"Low-level frame buffer memory access"</p>
                </div>
                <Button appearance=ButtonAppearance::Secondary on_click=move |_| on_check_info()>
                    "Check Info"
                </Button>
            </div>

            <div class="grid grid-cols-2 gap-3">
                <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                    <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Buffer Capacity"</span>
                    <span class="text-sm font-mono text-white block">
                        {move || dev.led_buf_capacity.get().map(|c| format_bytes(c as u64)).unwrap_or_else(|| "—".into())}
                    </span>
                </div>
                <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                    <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Current Size"</span>
                    <span class="text-sm font-mono text-white block">
                        {move || dev.led_buf_size.get().map(|s| format_bytes(s as u64)).unwrap_or_else(|| "—".into())}
                    </span>
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

            <div class="pt-3 border-t border-gray-800 space-y-3">
                <span class="text-xs font-semibold text-gray-300 block">"Read Buffer"</span>
                <div class="flex items-center gap-2">
                    <Button appearance=ButtonAppearance::Secondary on_click=move |_| on_read_buffer()>
                        "Read Entire Buffer"
                    </Button>
                    {move || {
                        if !dev.led_buf_dump.get().is_empty() {
                            view! {
                                <Button appearance=ButtonAppearance::Secondary on_click=move |_| on_download()>
                                    "Download .bin"
                                </Button>
                            }.into_any()
                        } else {
                            ().into_any()
                        }
                    }}
                </div>

                {move || {
                    let dump = dev.led_buf_dump.get();
                    if !dump.is_empty() {
                        view! {
                            <pre class="bg-gray-950 p-3 rounded-lg border border-gray-800 text-xs font-mono text-gray-300 max-h-48 overflow-auto">
                                {render_hex_dump(&dump, 512)}
                            </pre>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </div>

            <div class="pt-3 border-t border-gray-800 space-y-3">
                <span class="text-xs font-semibold text-gray-300 block">"Write Buffer"</span>
                <div class="flex flex-wrap items-end gap-3">
                    <div class="w-32">
                        <NumberInput<u32> label="Offset (bytes)" value=offset_sig />
                    </div>
                    <div class="flex-1 min-w-[200px]">
                        <label class="block mb-1 text-xs font-medium text-gray-400">"Select .bin File"</label>
                        <input
                            type="file"
                            on:change=on_file_selected
                            class="block w-full text-xs text-gray-300 bg-gray-800 rounded-lg border border-gray-700 cursor-pointer p-2 focus:outline-none"
                        />
                    </div>
                </div>
                <Button appearance=ButtonAppearance::Primary on_click=move |_| on_upload_buffer()>
                    "Upload to LED Buffer"
                </Button>
            </div>
        </div>
    }
}
