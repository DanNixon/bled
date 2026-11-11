use super::config::{NormalizedConfig, normalize_config};
use super::logger::{LogLevel, log};
use crate::transport::RpcClient;
use bled_core::{api::DeviceInfo, effect::EffectManifest};
use futures::lock::Mutex;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use std::rc::Rc;

#[derive(Clone)]
pub struct ConnectedDevice {
    pub id: String,
    pub name: String,
    pub is_ble: bool,
    pub rpc: SendWrapper<Rc<Mutex<RpcClient>>>,
    pub config: RwSignal<Option<NormalizedConfig>>,
    pub config_error: RwSignal<Option<String>>,
    pub device_info: RwSignal<Option<DeviceInfo>>,
    // Staging and effect state
    pub staged_data: RwSignal<Vec<u8>>,
    pub staged_filename: RwSignal<String>,
    pub staged_is_hex: RwSignal<bool>,
    pub active_effect_path: RwSignal<String>,
    pub active_effect_manifest: RwSignal<Option<EffectManifest>>,
    pub effect_command_bytes: RwSignal<Vec<u8>>,
    pub effect_param_bytes: RwSignal<Vec<u8>>,
    pub effect_command_len: RwSignal<Option<u64>>,
    pub effect_param_len: RwSignal<Option<u64>>,
    pub led_buf_capacity: RwSignal<Option<u32>>,
    pub led_buf_size: RwSignal<Option<u32>>,
}

// Implement Send & Sync for ConnectedDevice since it is used exclusively on the browser main thread
unsafe impl Send for ConnectedDevice {}
unsafe impl Sync for ConnectedDevice {}

pub struct SpanPixelsRequest {
    pub channel: usize,
    pub start: usize,
    pub count: usize,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub auto_commit: bool,
}

impl ConnectedDevice {
    pub fn new(rpc: RpcClient) -> Self {
        let id = rpc.id();
        let name = rpc.name();
        let is_ble = rpc.is_ble();

        Self {
            id,
            name,
            is_ble,
            rpc: SendWrapper::new(Rc::new(Mutex::new(rpc))),
            config: RwSignal::new(None),
            config_error: RwSignal::new(None),
            device_info: RwSignal::new(None),
            staged_data: RwSignal::new(Vec::new()),
            staged_filename: RwSignal::new(String::new()),
            staged_is_hex: RwSignal::new(false),
            active_effect_path: RwSignal::new(String::new()),
            active_effect_manifest: RwSignal::new(None),
            effect_command_bytes: RwSignal::new(Vec::new()),
            effect_param_bytes: RwSignal::new(Vec::new()),
            effect_command_len: RwSignal::new(None),
            effect_param_len: RwSignal::new(None),
            led_buf_capacity: RwSignal::new(None),
            led_buf_size: RwSignal::new(None),
        }
    }

    pub fn display_name(&self) -> String {
        if let Some(cfg) = self.config.get()
            && !cfg.name.is_empty()
        {
            return cfg.name;
        }
        self.name.clone()
    }

    pub async fn load_config(&self) {
        let mut rpc = self.rpc.lock().await;
        log(
            format!("[{}] Reading configuration...", self.name),
            LogLevel::Info,
        );
        match rpc.read_config::<fn(u8)>(None).await {
            Ok(cfg) => {
                let norm = normalize_config(&cfg);
                self.config.set(Some(norm));
                self.config_error.set(None);
                log(
                    format!("[{}] Loaded config \"{}\"", self.name, cfg.name().as_str()),
                    LogLevel::Success,
                );
            }
            Err(e) => {
                self.config.set(None);
                self.config_error.set(Some(e.clone()));
                log(
                    format!("[{}] Could not read config: {e}", self.name),
                    LogLevel::Error,
                );
            }
        }
    }

    pub async fn refresh_info(&self) {
        let mut rpc = self.rpc.lock().await;
        match rpc.read_device_info().await {
            Ok(info) => {
                self.device_info.set(Some(info.clone()));
                log(
                    format!(
                        "[{}] Device Info: rev={}, boot={:?}, uptime={} ms",
                        self.name,
                        info.git_revision().as_str(),
                        info.boot_reason(),
                        info.uptime_ms()
                    ),
                    LogLevel::Success,
                );
            }
            Err(e) => {
                log(
                    format!("[{}] Failed to read device info: {e}", self.name),
                    LogLevel::Error,
                );
            }
        }
    }

    pub async fn reboot(&self) {
        let mut rpc = self.rpc.lock().await;
        log(
            format!("[{}] Sending reboot command...", self.name),
            LogLevel::Info,
        );
        let _ = rpc.reboot().await;
        rpc.disconnect();
    }

    pub async fn commit(&self) {
        let mut rpc = self.rpc.lock().await;
        match rpc.commit().await {
            Ok(()) => log(
                format!("[{}] Committed LED buffer", self.name),
                LogLevel::Success,
            ),
            Err(e) => log(
                format!("[{}] Commit failed: {e}", self.name),
                LogLevel::Error,
            ),
        }
    }

    pub async fn refresh_led_buf_info(&self) {
        let mut rpc = self.rpc.lock().await;
        match rpc.led_buffer_info().await {
            Ok(info) => {
                self.led_buf_capacity.set(Some(*info.capacity()));
                self.led_buf_size.set(Some(*info.size()));
            }
            Err(e) => {
                log(
                    format!("[{}] Failed reading LED buffer info: {e}", self.name),
                    LogLevel::Error,
                );
            }
        }
    }

    pub async fn refresh_effect(&self) {
        let mut rpc = self.rpc.lock().await;
        match rpc.get_active_effect().await {
            Ok(Some(path)) => {
                self.active_effect_path.set(path.clone());
                let cmd_len = rpc.effect_command_stat().await.ok();
                let param_len = rpc.effect_parameter_stat().await.ok();
                let cmd_bytes = rpc.effect_command_read().await.unwrap_or_default();
                let param_bytes = rpc.effect_parameter_read().await.unwrap_or_default();
                let manifest = rpc.read_effect_manifest().await.ok();

                self.effect_command_len.set(cmd_len);
                self.effect_param_len.set(param_len);
                self.effect_command_bytes.set(cmd_bytes);
                self.effect_param_bytes.set(param_bytes);
                self.active_effect_manifest.set(manifest);
            }
            Ok(None) => {
                self.active_effect_path.set(String::new());
                self.active_effect_manifest.set(None);
                self.effect_command_len.set(None);
                self.effect_param_len.set(None);
                self.effect_command_bytes.set(Vec::new());
                self.effect_param_bytes.set(Vec::new());
            }
            Err(e) => {
                log(
                    format!("[{}] Failed to read active effect: {e}", self.name),
                    LogLevel::Error,
                );
            }
        }
    }

    pub async fn set_span_pixels(&self, req: SpanPixelsRequest) {
        let offset = self
            .config
            .get_untracked()
            .and_then(|c| c.channel_offsets.get(req.channel).copied())
            .unwrap_or(0)
            + req.start * 3;

        let mut data = vec![0u8; req.count * 3];
        for i in 0..req.count {
            data[i * 3] = req.r;
            data[i * 3 + 1] = req.g;
            data[i * 3 + 2] = req.b;
        }

        let mut rpc = self.rpc.lock().await;
        if let Err(e) = rpc
            .write_led_buffer_chunked::<fn(u8)>(offset as u32, &data, None)
            .await
        {
            log(
                format!("[{}] Failed writing span pixels: {e}", self.name),
                LogLevel::Error,
            );
            return;
        }

        if req.auto_commit {
            let _ = rpc.commit().await;
        }
        log(
            format!(
                "[{}] Set {} pixels on Ch {}",
                self.name, req.count, req.channel
            ),
            LogLevel::Success,
        );
    }
}
