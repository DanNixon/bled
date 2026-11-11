use crate::state::{ConnectedDevice, SpanPixelsRequest};
use leptodon::button::{Button, ButtonAppearance};
use leptodon::checkbox::Checkbox;
use leptodon::input::{NumberInput, TextInput};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn LedPixels(
    dev: ConnectedDevice,
    fixture_idx: Option<usize>,
    span_idx: Option<usize>,
    channel_idx: Option<usize>,
) -> impl IntoView {
    let initial_count = {
        let cfg = dev.config.get_untracked();
        if let Some(cfg) = cfg {
            if let (Some(f_idx), Some(s_idx)) = (fixture_idx, span_idx) {
                cfg.fixtures
                    .get(f_idx)
                    .and_then(|f| f.spans.get(s_idx))
                    .map(|s| s.length as u32)
                    .unwrap_or(1)
            } else if let Some(f_idx) = fixture_idx {
                cfg.fixtures
                    .get(f_idx)
                    .map(|f| f.total_pixels as u32)
                    .unwrap_or(1)
            } else {
                1
            }
        } else {
            1
        }
    };

    let start_sig = RwSignal::new(0u32);
    let count_sig = RwSignal::new(initial_count);
    let auto_commit_sig = RwSignal::new(true);

    // Sync default pixel count if config updates
    Effect::watch(
        move || dev.config.get(),
        move |cfg_opt, _, _| {
            if let Some(cfg) = cfg_opt {
                if let (Some(f_idx), Some(s_idx)) = (fixture_idx, span_idx) {
                    if let Some(span) = cfg.fixtures.get(f_idx).and_then(|f| f.spans.get(s_idx)) {
                        count_sig.set(span.length as u32);
                    }
                } else if let Some(f_idx) = fixture_idx
                    && let Some(fixture) = cfg.fixtures.get(f_idx)
                {
                    count_sig.set(fixture.total_pixels as u32);
                }
            }
        },
        false,
    );

    let r_sig = RwSignal::new(255u8);
    let g_sig = RwSignal::new(100u8);
    let b_sig = RwSignal::new(0u8);
    let hex_sig = RwSignal::new("FF6400".to_string());

    // Sync Hex -> RGB
    Effect::watch(
        move || hex_sig.get(),
        move |hex, _, _| {
            let clean = hex.trim().replace('#', "");
            if clean.len() == 6
                && let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&clean[0..2], 16),
                    u8::from_str_radix(&clean[2..4], 16),
                    u8::from_str_radix(&clean[4..6], 16),
                )
            {
                r_sig.set(r);
                g_sig.set(g);
                b_sig.set(b);
            }
        },
        false,
    );

    // Sync RGB -> Hex
    Effect::watch(
        move || (r_sig.get(), g_sig.get(), b_sig.get()),
        move |&(r, g, b), _, _| {
            let h = format!("{r:02X}{g:02X}{b:02X}");
            if hex_sig.get_untracked() != h {
                hex_sig.set(h);
            }
        },
        false,
    );

    let swatches = vec![
        ("Red", "#ff0000", 255, 0, 0),
        ("Green", "#00ff00", 0, 255, 0),
        ("Blue", "#0000ff", 0, 0, 255),
        ("Cyan", "#00ffff", 0, 255, 255),
        ("Magenta", "#ff00ff", 255, 0, 255),
        ("Yellow", "#ffff00", 255, 255, 0),
        ("White", "#ffffff", 255, 255, 255),
        ("Warm White", "#ffa757", 255, 167, 87),
        ("Off / Black", "#000000", 0, 0, 0),
    ];

    let d_apply = dev.clone();
    let on_apply = move |_| {
        let dev = d_apply.clone();
        let r = r_sig.get_untracked();
        let g = g_sig.get_untracked();
        let b = b_sig.get_untracked();
        let start = start_sig.get_untracked() as usize;
        let count = count_sig.get_untracked() as usize;
        let auto_commit = auto_commit_sig.get_untracked();

        spawn_local(async move {
            let cfg = dev.config.get_untracked();
            if let Some(cfg) = cfg {
                if let (Some(f_idx), Some(s_idx)) = (fixture_idx, span_idx) {
                    if let Some(span) = cfg.fixtures.get(f_idx).and_then(|f| f.spans.get(s_idx)) {
                        dev.set_span_pixels(SpanPixelsRequest {
                            channel: span.channel,
                            start: span.start + start,
                            count: count.min(span.length.saturating_sub(start)),
                            r,
                            g,
                            b,
                            auto_commit,
                        })
                        .await;
                    }
                } else if let Some(f_idx) = fixture_idx {
                    if let Some(fixture) = cfg.fixtures.get(f_idx) {
                        let mut rem_start = start;
                        let mut rem_count = count;
                        for span in &fixture.spans {
                            if rem_count == 0 {
                                break;
                            }
                            if rem_start >= span.length {
                                rem_start -= span.length;
                                continue;
                            }
                            let s_start = span.start + rem_start;
                            let s_count = rem_count.min(span.length - rem_start);
                            rem_start = 0;
                            rem_count -= s_count;
                            dev.set_span_pixels(SpanPixelsRequest {
                                channel: span.channel,
                                start: s_start,
                                count: s_count,
                                r,
                                g,
                                b,
                                auto_commit: false,
                            })
                            .await;
                        }
                        if auto_commit {
                            dev.commit().await;
                        }
                    }
                } else if let Some(ch_idx) = channel_idx {
                    dev.set_span_pixels(SpanPixelsRequest {
                        channel: ch_idx,
                        start,
                        count,
                        r,
                        g,
                        b,
                        auto_commit,
                    })
                    .await;
                }
            }
        });
    };

    let target_desc = move || {
        let cfg = dev.config.get();
        if let Some(cfg) = cfg {
            if let (Some(f_idx), Some(s_idx)) = (fixture_idx, span_idx) {
                if let Some(span) = cfg.fixtures.get(f_idx).and_then(|f| f.spans.get(s_idx)) {
                    let f_name = &cfg.fixtures[f_idx].name;
                    return format!(
                        "{f_name} / {} — Ch {} [{}..{}] ({} px)",
                        span.name,
                        span.channel,
                        span.start,
                        span.start + span.length,
                        span.length
                    );
                }
            } else if let Some(f_idx) = fixture_idx {
                if let Some(fixture) = cfg.fixtures.get(f_idx) {
                    return format!(
                        "{}: entire fixture ({} px, {})",
                        fixture.name, fixture.total_pixels, fixture.layout_name
                    );
                }
            } else if let Some(ch_idx) = channel_idx {
                return format!("Channel {ch_idx}: direct channel addressing");
            }
        }
        "LED Pixels".into()
    };

    view! {
        <div class="bg-gray-900 border border-gray-800 rounded-xl p-5 shadow-sm space-y-4">
            <div class="border-b border-gray-800 pb-3">
                <h3 class="text-base font-semibold text-white">"LED Pixels"</h3>
                <p class="text-xs text-oa-blue font-mono mt-0.5">{target_desc}</p>
            </div>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="space-y-3">
                    <div class="grid grid-cols-2 gap-3">
                        <NumberInput<u32> label="Start Pixel" value=start_sig />
                        <NumberInput<u32> label="Count" value=count_sig />
                    </div>

                    <div class="pt-2 border-t border-gray-800 space-y-2">
                        <span class="text-xs font-semibold text-gray-400 uppercase tracking-wider block">"Color Swatches"</span>
                        <div class="grid grid-cols-3 gap-2">
                            {swatches.into_iter().map(|(name, hex, r, g, b)| {
                                view! {
                                    <button
                                        type="button"
                                        class="flex items-center gap-2 px-2 py-1.5 rounded-lg bg-gray-800 hover:bg-gray-700 border border-gray-700 text-xs transition-colors"
                                        on:click=move |_| {
                                            r_sig.set(r);
                                            g_sig.set(g);
                                            b_sig.set(b);
                                        }
                                    >
                                        <span class="w-4 h-4 rounded-full border border-gray-600 shrink-0" style=format!("background-color: {hex}")></span>
                                        <span class="truncate text-gray-300">{name}</span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                </div>

                <div class="space-y-3 bg-gray-800/40 p-4 rounded-xl border border-gray-800">
                    <span class="text-xs font-semibold text-gray-400 uppercase tracking-wider block">"Color Values"</span>
                    <div class="flex items-center gap-3">
                        <div
                            class="w-12 h-12 rounded-xl border border-gray-700 shadow-inner shrink-0"
                            style=move || format!("background-color: #{};", hex_sig.get())
                        ></div>
                        <div class="flex-1">
                            <TextInput label="Hex" value=hex_sig />
                        </div>
                    </div>

                    <div class="grid grid-cols-3 gap-2">
                        <NumberInput<u8> label="R (0-255)" value=r_sig />
                        <NumberInput<u8> label="G (0-255)" value=g_sig />
                        <NumberInput<u8> label="B (0-255)" value=b_sig />
                    </div>
                </div>
            </div>

            <div class="flex items-center justify-between pt-4 border-t border-gray-800">
                <Checkbox checked=auto_commit_sig>
                    "Auto-commit on write"
                </Checkbox>
                <Button appearance=ButtonAppearance::Primary on_click=on_apply>
                    "Set Colours"
                </Button>
            </div>
        </div>
    }
}
