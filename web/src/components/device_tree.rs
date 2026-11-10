use crate::state::{DeviceRegistry, TreeSelection};
use leptodon::class_list;
use leptos::context::use_context;
use leptos::prelude::*;

#[component]
pub fn DeviceTree() -> impl IntoView {
    let registry = use_context::<DeviceRegistry>().expect("DeviceRegistry");
    let collapsed_keys = RwSignal::new(std::collections::HashSet::<String>::new());

    let toggle_key = move |key: String| {
        collapsed_keys.update(|set| {
            if !set.remove(&key) {
                set.insert(key);
            }
        });
    };

    view! {
        <aside class="w-full lg:w-72 bg-gray-900 border-r border-gray-800 p-4 flex flex-col shrink-0 min-h-[400px]">
            <h2 class="text-xs font-semibold uppercase tracking-wider text-gray-400 mb-3">
                "Devices"
            </h2>

            {move || {
                let dev_list = registry.devices.get();
                let current_sel = registry.selection.get();
                let collapsed = collapsed_keys.get();

                if dev_list.is_empty() {
                    view! {
                        <p class="text-sm text-gray-500 italic py-4">"No devices connected."</p>
                    }.into_any()
                } else {
                    view! {
                        <ul class="space-y-1 font-medium text-sm">
                            {dev_list.into_iter().map(|dev| {
                                let dev_id = dev.id.clone();
                                let dev_id_clone1 = dev_id.clone();
                                let dev_id_clone2 = dev_id.clone();
                                let dev_id_clone3 = dev_id.clone();
                                let dev_id_clone4 = dev_id.clone();
                                let dev_key = format!("d:{dev_id}");
                                let is_dev_collapsed = collapsed.contains(&dev_key);

                                let is_dev_selected = matches!(&current_sel, Some(TreeSelection::Device { device_id }) if device_id == &dev_id);
                                let is_sd_selected = matches!(&current_sel, Some(TreeSelection::SdCard { device_id }) if device_id == &dev_id);
                                let is_fx_selected = matches!(&current_sel, Some(TreeSelection::Effect { device_id }) if device_id == &dev_id);

                                let cfg_opt = dev.config.get();
                                let dev_name = dev.display_name();

                                view! {
                                    <li class="space-y-0.5">
                                        <div class=class_list![
                                            "flex items-center justify-between rounded-lg px-2 py-1.5 transition-colors group",
                                            ("bg-gray-800 text-white font-semibold", move || is_dev_selected),
                                            ("hover:bg-gray-800/60 text-gray-300", move || !is_dev_selected)
                                        ]>
                                            <div class="flex items-center gap-1.5 flex-1 min-w-0 cursor-pointer"
                                                on:click=move |_| {
                                                    registry.selection.set(Some(TreeSelection::Device { device_id: dev_id_clone1.clone() }));
                                                }
                                            >
                                                <button
                                                    type="button"
                                                    class="text-gray-400 hover:text-white px-1 text-xs"
                                                    on:click={
                                                        let dev_key = dev_key.clone();
                                                        move |e: leptos::ev::MouseEvent| {
                                                            e.stop_propagation();
                                                            toggle_key(dev_key.clone());
                                                        }
                                                    }
                                                >
                                                    {if is_dev_collapsed { "▸" } else { "▾" }}
                                                </button>
                                                <span class="truncate text-sm">{dev_name}</span>
                                            </div>
                                            <button
                                                type="button"
                                                class="text-gray-500 hover:text-red-400 text-xs px-1.5 py-0.5 rounded"
                                                title="Disconnect"
                                                on:click=move |_| registry.disconnect_device(&dev_id_clone2)
                                            >
                                                "✕"
                                            </button>
                                        </div>

                                        {if !is_dev_collapsed {
                                            view! {
                                                <ul class="pl-5 space-y-0.5 border-l border-gray-800 ml-3 mt-1">
                                                    <li>
                                                        <button
                                                            type="button"
                                                            class=class_list![
                                                                "w-full text-left flex items-center gap-2 px-2 py-1 rounded text-xs transition-colors",
                                                                ("bg-oa-blue/20 text-oa-blue font-semibold", move || is_sd_selected),
                                                                ("hover:bg-gray-800 text-gray-400 hover:text-gray-200", move || !is_sd_selected)
                                                            ]
                                                            on:click=move |_| {
                                                                registry.selection.set(Some(TreeSelection::SdCard { device_id: dev_id_clone3.clone() }));
                                                            }
                                                        >
                                                            <span>"📁"</span>"SD Card"
                                                        </button>
                                                    </li>
                                                    <li>
                                                        <button
                                                            type="button"
                                                            class=class_list![
                                                                "w-full text-left flex items-center gap-2 px-2 py-1 rounded text-xs transition-colors",
                                                                ("bg-oa-blue/20 text-oa-blue font-semibold", move || is_fx_selected),
                                                                ("hover:bg-gray-800 text-gray-400 hover:text-gray-200", move || !is_fx_selected)
                                                            ]
                                                            on:click=move |_| {
                                                                registry.selection.set(Some(TreeSelection::Effect { device_id: dev_id_clone4.clone() }));
                                                            }
                                                        >
                                                            <span>"✨"</span>"Effects"
                                                        </button>
                                                    </li>

                                                    {if let Some(cfg) = cfg_opt {
                                                        view! {
                                                            <>
                                                                {cfg.fixtures.into_iter().enumerate().map(|(f_idx, fixture)| {
                                                                    let dev_id_f = dev_id.clone();
                                                                    let dev_id_f2 = dev_id.clone();
                                                                    let f_key = format!("d:{dev_id}:f:{f_idx}");
                                                                    let is_f_collapsed = collapsed.contains(&f_key);
                                                                    let is_f_sel = matches!(&current_sel, Some(TreeSelection::Fixture { device_id, fixture_idx }) if device_id == &dev_id_f && *fixture_idx == f_idx);

                                                                    view! {
                                                                        <li class="space-y-0.5">
                                                                            <div class=class_list![
                                                                                "flex items-center justify-between rounded px-2 py-1 text-xs cursor-pointer transition-colors",
                                                                                ("bg-oa-blue/20 text-oa-blue font-semibold", move || is_f_sel),
                                                                                ("hover:bg-gray-800 text-gray-400 hover:text-gray-200", move || !is_f_sel)
                                                                            ]
                                                                                on:click=move |_| {
                                                                                    registry.selection.set(Some(TreeSelection::Fixture { device_id: dev_id_f.clone(), fixture_idx: f_idx }));
                                                                                }
                                                                            >
                                                                                <div class="flex items-center gap-1.5 truncate">
                                                                                    {if !fixture.spans.is_empty() {
                                                                                        let f_key = f_key.clone();
                                                                                        view! {
                                                                                            <button
                                                                                                type="button"
                                                                                                class="text-gray-500 hover:text-white px-0.5"
                                                                                                on:click=move |e: leptos::ev::MouseEvent| {
                                                                                                    e.stop_propagation();
                                                                                                    toggle_key(f_key.clone());
                                                                                                }
                                                                                            >
                                                                                                {if is_f_collapsed { "▸" } else { "▾" }}
                                                                                            </button>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        ().into_any()
                                                                                    }}
                                                                                    <span>"💡"</span>
                                                                                    <span class="truncate">{fixture.name} {format!("({} px)", fixture.total_pixels)}</span>
                                                                                </div>
                                                                            </div>

                                                                            {if !is_f_collapsed {
                                                                                view! {
                                                                                    <ul class="pl-4 space-y-0.5 border-l border-gray-800 ml-2">
                                                                                        {fixture.spans.into_iter().enumerate().map(|(s_idx, span)| {
                                                                                            let dev_id_s = dev_id_f2.clone();
                                                                                            let is_s_sel = matches!(&current_sel, Some(TreeSelection::Span { device_id, fixture_idx, span_idx }) if device_id == &dev_id_s && *fixture_idx == f_idx && *span_idx == s_idx);
                                                                                            view! {
                                                                                                <li>
                                                                                                    <button
                                                                                                        type="button"
                                                                                                        class=class_list![
                                                                                                            "w-full text-left truncate px-2 py-0.5 rounded text-[11px] transition-colors",
                                                                                                            ("bg-oa-blue/20 text-oa-blue font-semibold", move || is_s_sel),
                                                                                                            ("hover:bg-gray-800 text-gray-500 hover:text-gray-300", move || !is_s_sel)
                                                                                                        ]
                                                                                                        on:click=move |_| {
                                                                                                            registry.selection.set(Some(TreeSelection::Span { device_id: dev_id_s.clone(), fixture_idx: f_idx, span_idx: s_idx }));
                                                                                                        }
                                                                                                    >
                                                                                                        {format!("▪ {} (Ch {}, {}..{})", span.name, span.channel, span.start, span.start + span.length)}
                                                                                                    </button>
                                                                                                </li>
                                                                                            }
                                                                                        }).collect_view()}
                                                                                    </ul>
                                                                                }.into_any()
                                                                            } else {
                                                                                ().into_any()
                                                                            }}
                                                                        </li>
                                                                    }
                                                                }).collect_view()}
                                                            </>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </ul>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            }}
        </aside>
    }
}
