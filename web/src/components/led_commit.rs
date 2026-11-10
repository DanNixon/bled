use crate::state::ConnectedDevice;
use leptodon::button::{Button, ButtonAppearance};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn LedCommit(dev: ConnectedDevice) -> impl IntoView {
    let d = dev.clone();
    let on_commit = move || {
        let dev = d.clone();
        spawn_local(async move {
            dev.commit().await;
        });
    };

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-3">
            <div class="flex items-center justify-between border-b border-gray-800 pb-3">
                <div>
                    <h3 class="text-base font-semibold text-white">"Commit / Render"</h3>
                    <p class="text-xs text-gray-400">"Render staged buffer to physical LEDs"</p>
                </div>
            </div>
            <p class="text-sm text-gray-300">
                "Flushes the staged LED data buffer to all physical LED outputs."
            </p>
            <Button appearance=ButtonAppearance::Primary on_click=move |_| on_commit()>
                "Commit & Render"
            </Button>
        </div>
    }
}
