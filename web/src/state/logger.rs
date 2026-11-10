use chrono::Local;
use leptodon::toast::{Toast, ToastAppearance, ToasterContext};
use leptos::context::use_context;
use leptos::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: LogLevel,
}

#[derive(Clone, Copy)]
pub struct ActivityLogSignal(pub RwSignal<Vec<LogEntry>>);

pub fn log(msg: impl Into<String>, level: LogLevel) {
    let msg_str = msg.into();
    let now = Local::now().format("%H:%M:%S%.3f").to_string();

    if let Some(ActivityLogSignal(sig)) = use_context::<ActivityLogSignal>() {
        sig.update(|entries| {
            entries.push(LogEntry {
                timestamp: now.clone(),
                message: msg_str.clone(),
                level: level.clone(),
            });
            if entries.len() > 300 {
                entries.remove(0);
            }
        });
    }

    if let Some(toast_ctx) = use_context::<ToasterContext>() {
        let (show_toast, dismiss_toast) = toast_ctx.use_toast();
        let toast_appearance = match level {
            LogLevel::Info => ToastAppearance::Inform,
            LogLevel::Success => ToastAppearance::Success,
            LogLevel::Error => ToastAppearance::Danger,
        };
        let toast_title = match level {
            LogLevel::Info => "Info",
            LogLevel::Success => "Success",
            LogLevel::Error => "Error",
        };
        let m = msg_str.clone();

        show_toast(
            (move || {
                let m = m.clone();
                view! {
                    <Toast
                        title=toast_title
                        message=m
                        dismiss=dismiss_toast
                        appearance=toast_appearance
                    />
                }
            })
            .into(),
        );
    }
}
