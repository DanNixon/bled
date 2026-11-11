use crate::state::ConnectedDevice;
use crate::utils::formatters::{format_bytes, format_uptime};
use leptodon::alert::WarnAlert;
use leptodon::button::{Button, ButtonAppearance, DialogButton, DialogButtonChildren};
use leptodon::util::callback::BoxCallback;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn DeviceInfoPanel(dev: ConnectedDevice) -> impl IntoView {
    let d1 = dev.clone();
    let d3 = dev.clone();
    let d_cfg = dev.clone();
    let d_info = dev.clone();

    let on_refresh = move |_| {
        let dev = d1.clone();
        spawn_local(async move {
            dev.refresh_info().await;
            dev.load_config().await;
            dev.refresh_led_buf_info().await;
        });
    };

    let registry = leptos::context::use_context::<crate::state::DeviceRegistry>();
    let reboot_dialog_visible = RwSignal::new(false);

    let on_reboot_cancel = BoxCallback::new(move || {
        reboot_dialog_visible.set(false);
    });

    let on_reboot = BoxCallback::new(move || {
        reboot_dialog_visible.set(false);
        let dev = d3.clone();
        let reg = registry;
        spawn_local(async move {
            let dev_id = dev.id.clone();
            dev.reboot().await;
            if let Some(r) = reg {
                r.remove_device(&dev_id);
            }
        });
    });

    let reboot_msg = format!(
        "Are you sure you want to reboot \"{}\"?",
        dev.display_name()
    );
    let dev_name = dev.display_name();

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="flex flex-wrap items-center justify-between gap-3 border-b border-gray-800 pb-3">
                <div class="flex items-center gap-2">
                    <span class="text-base font-semibold text-white">"Device"</span>
                    <span class="text-xs text-gray-400 font-mono">{format!("({dev_name})")}</span>
                </div>
                <div class="flex items-center gap-2">
                    <Button appearance=ButtonAppearance::Secondary on_click=on_refresh>
                        "Refresh Info"
                    </Button>
                    <DialogButton
                        appearance=Signal::derive(|| ButtonAppearance::Danger)
                        dialog_title="Reboot Device"
                        dialog_visible=reboot_dialog_visible
                        primary_text="Reboot"
                        on_click_primary=on_reboot
                        secondary_text="Cancel"
                        on_click_secondary=on_reboot_cancel
                        button_children=DialogButtonChildren::builder().children(Box::new(|| view! { "Reboot Device" }.into_any())).build()
                    >
                        <p class="text-sm text-gray-300">
                            {reboot_msg}
                        </p>
                    </DialogButton>
                </div>
            </div>

            {move || {
                if let Some(err) = d_cfg.config_error.get() {
                    view! {
                        <WarnAlert border=true>
                            {format!("Could not read active config: {err}. No fixtures are available until it can be read.")}
                        </WarnAlert>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}

            {move || {
                let info_opt = d_info.device_info.get();
                view! {
                    <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3 pt-2">
                        <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                            <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Git Revision"</span>
                            <span class="text-sm font-mono text-white truncate block">
                                {info_opt.as_ref().map(|i| i.git_revision().to_string()).unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                            <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Boot Reason"</span>
                            <span class="text-sm font-mono text-white block">
                                {info_opt.as_ref().map(|i| format!("{:?}", i.boot_reason())).unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                            <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Uptime"</span>
                            <span class="text-sm text-white block">
                                {info_opt.as_ref().map(|i| format_uptime(*i.uptime_ms())).unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                            <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Output Channels"</span>
                            <span class="text-sm font-mono text-white block">
                                {info_opt.as_ref().map(|i| i.channel_count().to_string()).unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                        <div class="bg-gray-800/60 rounded-lg p-3 border border-gray-750">
                            <span class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 block mb-1">"Buffer Capacity"</span>
                            <span class="text-sm font-mono text-white block">
                                {info_opt.as_ref().map(|i| format_bytes(*i.led_buffer_capacity())).unwrap_or_else(|| "—".into())}
                            </span>
                        </div>
                    </div>
                }
            }}
        </div>
    }
}
