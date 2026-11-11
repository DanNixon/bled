use chrono::Local;
use leptodon::toast::{Toast, ToastAppearance, ToasterContext};
use leptos::context::use_context;
use leptos::prelude::*;
use std::cell::RefCell;

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

thread_local! {
    static GLOBAL_LOG_SIGNAL: RefCell<Option<RwSignal<Vec<LogEntry>>>> = const { RefCell::new(None) };
    static GLOBAL_TOASTER_CTX: RefCell<Option<ToasterContext>> = const { RefCell::new(None) };
}

pub fn set_global_log_signal(sig: RwSignal<Vec<LogEntry>>) {
    GLOBAL_LOG_SIGNAL.with(|cell| {
        *cell.borrow_mut() = Some(sig);
    });
}

pub fn set_global_toaster_context(ctx: ToasterContext) {
    GLOBAL_TOASTER_CTX.with(|cell| {
        *cell.borrow_mut() = Some(ctx);
    });
}

pub fn log(msg: impl Into<String>, level: LogLevel) {
    let msg_str = msg.into();
    let now = Local::now().format("%H:%M:%S%.3f").to_string();

    let log_sig = GLOBAL_LOG_SIGNAL.with(|cell| *cell.borrow())
        .or_else(|| use_context::<ActivityLogSignal>().map(|ActivityLogSignal(s)| s));

    if let Some(sig) = log_sig {
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

    let toast_ctx = GLOBAL_TOASTER_CTX.with(|cell| cell.borrow().clone())
        .or_else(use_context::<ToasterContext>);

    if let Some(toast_ctx) = toast_ctx {
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
