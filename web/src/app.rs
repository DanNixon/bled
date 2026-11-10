use crate::components::activity_log::ActivityLog;
use crate::components::device_tree::DeviceTree;
use crate::components::header::Header;
use crate::components::main_panel::MainPanel;
use crate::state::{ActivityLogSignal, DeviceRegistry, LogEntry, LogLevel};
use leptodon::toast::Toaster;
use leptos::context::provide_context;
use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    let registry = DeviceRegistry::new();
    let activity_log = ActivityLogSignal(RwSignal::new(vec![LogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        message: "WebAssembly protocol runtime initialized.".into(),
        level: LogLevel::Info,
    }]));

    provide_context(registry);
    provide_context(activity_log);

    view! {
        <Toaster>
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
