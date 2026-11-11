use crate::state::{ActivityLogSignal, LogLevel};
use leptodon::button::{Button, ButtonAppearance};
use leptos::context::use_context;
use leptos::prelude::*;

#[component]
pub fn ActivityLog() -> impl IntoView {
    let ActivityLogSignal(log_sig) = use_context::<ActivityLogSignal>().expect("ActivityLogSignal");
    let is_collapsed = RwSignal::new(false);
    let log_container_ref = NodeRef::<leptos::html::Div>::new();

    Effect::new(move |_| {
        let _ = log_sig.get();
        if let Some(el) = log_container_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    });

    view! {
        <footer class="bg-gray-900 border-t border-gray-800 transition-all duration-200">
            <div class="px-4 py-2 flex items-center justify-between border-b border-gray-800/80 bg-gray-900/90">
                <div class="flex items-center gap-2">
                    <span class="text-xs font-semibold uppercase tracking-wider text-gray-400">"Activity Log"</span>
                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-gray-800 text-gray-400 font-mono">
                        {move || log_sig.get().len()}
                    </span>
                </div>
                <div class="flex items-center gap-2">
                    <Button appearance=ButtonAppearance::Minimal on_click=move |_| {
                        log_sig.set(Vec::new());
                    }>
                        "Clear"
                    </Button>
                    <button
                        type="button"
                        class="text-gray-400 hover:text-white px-1 text-xs"
                        on:click=move |_| is_collapsed.update(|c| *c = !*c)
                    >
                        {move || if is_collapsed.get() { "▲ Expand" } else { "▼ Collapse" }}
                    </button>
                </div>
            </div>

            {move || {
                if !is_collapsed.get() {
                    view! {
                        <div node_ref=log_container_ref class="p-3 bg-gray-950 font-mono text-[11px] h-36 overflow-auto space-y-1">
                            {move || {
                                let entries = log_sig.get();
                                if entries.is_empty() {
                                    view! {
                                        <p class="text-gray-600 italic">"Log initialized."</p>
                                    }.into_any()
                                } else {
                                    entries.into_iter().map(|entry| {
                                        let (color_class, badge, badge_class) = match entry.level {
                                            LogLevel::Info => ("text-gray-300", "INFO", "text-blue-400 bg-blue-950/60 border border-blue-900/40"),
                                            LogLevel::Success => ("text-green-300", "SUCCESS", "text-green-400 bg-green-950/60 border border-green-900/40"),
                                            LogLevel::Error => ("text-red-300", "ERROR", "text-red-400 bg-red-950/60 border border-red-900/40"),
                                        };
                                        view! {
                                            <div class="flex items-start gap-2 leading-relaxed">
                                                <span class="text-gray-600 shrink-0 select-none font-mono">{entry.timestamp}</span>
                                                <span class=format!("text-[9px] px-1 py-0.2 rounded font-semibold shrink-0 select-none {badge_class}")>
                                                    {badge}
                                                </span>
                                                <span class=format!("{color_class} break-all")>{entry.message}</span>
                                            </div>
                                        }
                                    }).collect_view().into_any()
                                }
                            }}
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
        </footer>
    }
}
