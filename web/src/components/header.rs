use crate::state::DeviceRegistry;
use crate::transport::bridge;
use leptodon::alert::{ErrorAlert, WarnAlert};
use leptodon::button::{Button, ButtonAppearance};
use leptos::context::use_context;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn Header() -> impl IntoView {
    let registry = use_context::<DeviceRegistry>().expect("DeviceRegistry");
    let is_ble_avail = bridge::is_ble_supported();
    let is_serial_avail = bridge::is_serial_supported();
    let is_secure = bridge::is_secure_context();
    let git_rev = git_version::git_version!();

    let on_connect_ble = move |_| {
        spawn_local(async move {
            registry.add_ble_device().await;
        });
    };

    let on_connect_serial = move |_| {
        spawn_local(async move {
            registry.add_serial_device().await;
        });
    };

    view! {
        <header class="bg-gray-900 border-b border-gray-800 px-6 py-4 flex flex-wrap items-center justify-between gap-4">
            <div class="flex flex-col">
                <div class="flex items-center gap-3">
                    <h1 class="text-2xl font-bold tracking-tight text-white flex items-center gap-2">
                        <span class="text-oa-blue">"⚡"</span>"BLED"
                    </h1>
                    <span class="text-xs px-2 py-0.5 rounded bg-gray-800 text-gray-400 font-mono">
                        <a
                            href=format!("https://github.com/DanNixon/bled/tree/{}", git_rev.replace("-modified", ""))
                            target="_blank"
                            class="hover:text-oa-blue transition-colors"
                        >
                            {git_rev}
                        </a>
                    </span>
                </div>
                <p class="text-xs text-gray-400 mt-0.5">
                    "Configuration and control of BLED LED controllers ["
                    <a href="https://github.com/DanNixon/bled" target="_blank" class="text-oa-blue hover:underline">
                        "GitHub"
                    </a>
                    "]"
                </p>
            </div>

            <div class="flex items-center gap-2">
                <Button
                    appearance=ButtonAppearance::Primary
                    on_click=on_connect_ble
                    loading=Signal::derive(move || registry.pending.get() > 0)
                >
                    "Connect BLE"
                </Button>
                <Button
                    appearance=ButtonAppearance::Primary
                    on_click=on_connect_serial
                    loading=Signal::derive(move || registry.pending.get() > 0)
                >
                    "Connect USB Serial"
                </Button>
            </div>
        </header>

        {if !is_ble_avail && !is_serial_avail {
            view! {
                <div class="px-6 pt-3">
                    <ErrorAlert border=true>
                        <strong>"No supported connection transport is available. "</strong>
                        "Use a browser with Web Bluetooth or Web Serial over HTTPS or localhost."
                    </ErrorAlert>
                </div>
            }.into_any()
        } else if !is_serial_avail {
            view! {
                <div class="px-6 pt-3">
                    <WarnAlert border=true>
                        {if !is_secure {
                            "Web Serial requires a secure context. Open this page over HTTPS or localhost."
                        } else {
                            "Web Serial is not supported by this browser. Use Chrome/Edge/Opera for USB Serial."
                        }}
                    </WarnAlert>
                </div>
            }.into_any()
        } else {
            ().into_any()
        }}
    }
}
