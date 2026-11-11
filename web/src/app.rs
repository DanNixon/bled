use crate::components::activity_log::ActivityLog;
use crate::components::device_tree::DeviceTree;
use crate::components::header::Header;
use crate::components::main_panel::MainPanel;
use crate::state::{ActivityLogSignal, DeviceRegistry, LogEntry, LogLevel};
use leptodon::toast::Toaster;
use leptos::context::provide_context;
use leptos::prelude::*;

#[component]
fn ToastContextBridge() -> impl IntoView {
    if let Some(ctx) = leptos::context::use_context::<leptodon::toast::ToasterContext>() {
        crate::state::set_global_toaster_context(ctx);
    }
    ()
}

#[component]
pub fn App() -> impl IntoView {
    let registry = DeviceRegistry::new();
    let raw_log_sig = RwSignal::new(vec![LogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        message: "WebAssembly protocol runtime initialized.".into(),
        level: LogLevel::Info,
    }]);
    crate::state::set_global_log_signal(raw_log_sig);
    let activity_log = ActivityLogSignal(raw_log_sig);

    provide_context(registry);
    provide_context(activity_log);

    view! {
        <Toaster>
            <ToastContextBridge />
            <div class="min-h-screen flex flex-col bg-gray-950 text-gray-100">
                <Header />
                <div class="flex-1 flex flex-col lg:flex-row overflow-hidden">
                    <DeviceTree />
                    <MainPanel />
                </div>
                <ActivityLog />
            </div>
        </Toaster>
    }
}
