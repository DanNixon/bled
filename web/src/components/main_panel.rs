use super::device_info::DeviceInfoPanel;
use super::effect::EffectPanel;
use super::file_staging::FileStaging;
use super::led_pixels::LedPixels;
use crate::state::{DeviceRegistry, TreeSelection};
use leptos::context::use_context;
use leptos::prelude::*;

#[component]
pub fn MainPanel() -> impl IntoView {
    let registry = use_context::<DeviceRegistry>().expect("DeviceRegistry");

    view! {
        <main class="flex-1 p-6 space-y-6 overflow-auto">
            {move || {
                let sel = registry.selection.get();
                let dev_opt = registry.selected_device();

                match (sel, dev_opt) {
                    (Some(TreeSelection::Device { .. }), Some(dev)) => {
                        view! {
                            <div class="space-y-6">
                                <DeviceInfoPanel dev=dev.clone() />
                            </div>
                        }.into_any()
                    }
                    (Some(TreeSelection::SdCard { .. }), Some(dev)) => {
                        view! {
                            <FileStaging dev=dev />
                        }.into_any()
                    }
                    (Some(TreeSelection::Effect { .. }), Some(dev)) => {
                        view! {
                            <EffectPanel dev=dev />
                        }.into_any()
                    }
                    (Some(TreeSelection::Fixture { fixture_idx, .. }), Some(dev)) => {
                        view! {
                            <LedPixels dev=dev fixture_idx=Some(fixture_idx) span_idx=None channel_idx=None />
                        }.into_any()
                    }
                    (Some(TreeSelection::Span { fixture_idx, span_idx, .. }), Some(dev)) => {
                        view! {
                            <LedPixels dev=dev fixture_idx=Some(fixture_idx) span_idx=Some(span_idx) channel_idx=None />
                        }.into_any()
                    }
                    (Some(TreeSelection::Channel { channel_idx, .. }), Some(dev)) => {
                        view! {
                            <LedPixels dev=dev fixture_idx=None span_idx=None channel_idx=Some(channel_idx) />
                        }.into_any()
                    }
                    _ => {
                        view! {
                            <div class="h-96 flex items-center justify-center border-2 border-dashed border-gray-800 rounded-xl">
                                <p class="text-gray-500 text-sm">
                                    "Connect a device, then select it in the tree to inspect and configure."
                                </p>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </main>
    }
}
