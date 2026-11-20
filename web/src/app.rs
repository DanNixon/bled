use crate::state::{
    ConnectedDevice, LogEntry, LogLevel, NormalizedConfig, TreeSelection, config::normalize_config,
    device::now_timestamp,
};
use crate::transport::{RpcClient, bridge};
use crate::utils::formatters::{
    bytes_to_hex, format_bytes, format_hex_rows, format_uptime, parse_hex,
};
use bled_core::{
    api::{ActivePlaylistInfo, DeviceInfo, DirEntry, DirEntryType},
    config::BrightnessLimitMode,
    effect::{EffectManifest, EffectPlaylist, NextPlaylistEntryAction},
};
use iced::widget::{
    Column, Row, button, center, column, container, opaque, progress_bar, responsive, row, rule,
    scrollable, slider, space, stack, text, text_editor, text_input, toggler,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Padding, Size, Task, Theme, border};
use send_wrapper::SendWrapper;
use std::collections::HashSet;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;

pub fn run_app() -> iced::Result {
    let initial_size = web_sys::window()
        .and_then(|w| {
            let width = w.inner_width().ok()?.as_f64()? as f32;
            let height = w.inner_height().ok()?.as_f64()? as f32;
            Some(Size::new(width.max(320.0), height.max(480.0)))
        })
        .unwrap_or_else(|| Size::new(1280.0, 800.0));

    iced::application(BledApp::boot, BledApp::update, BledApp::view)
        .title("BLED — iced Web UI PoC")
        .theme(Theme::Dark)
        .window_size(initial_size)
        .run()
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum AsyncOutcome {
    DeviceConnected(SendWrapper<ConnectedDevice>),
    ConnectFailed(String),
    DeviceBootstrap {
        device_id: String,
        info: Result<DeviceInfo, String>,
        config: Result<NormalizedConfig, String>,
    },
    DirectoryListed {
        device_id: String,
        path: String,
        entries: Result<Vec<DirEntry>, String>,
    },
    FileRead {
        device_id: String,
        path: String,
        bytes: Result<Vec<u8>, String>,
    },
    FileWritten {
        device_id: String,
        path: String,
        result: Result<(), String>,
    },
    FileDeleted {
        device_id: String,
        path: String,
        result: Result<(), String>,
    },
    EffectStateLoaded {
        device_id: String,
        path: String,
        manifest: Option<EffectManifest>,
        cmd_bytes: Vec<u8>,
        param_bytes: Vec<u8>,
        cmd_len: Option<u64>,
        param_len: Option<u64>,
    },
    PlaylistLoaded {
        device_id: String,
        info: Option<ActivePlaylistInfo>,
        playlist: Option<EffectPlaylist>,
    },
    PixelsUpdated {
        device_id: String,
        result: Result<String, String>,
    },
    DeviceRebooted(String),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Message {
    // Connection & Device Tree
    ConnectBle,
    ConnectSerial,
    AddDemoDevice,
    DisconnectDevice(String),
    SelectTree(TreeSelection),
    ToggleTreeCollapse(String),
    ToggleMobileDrawer,
    CloseMobileDrawer,

    // Device Info actions
    RefreshDeviceInfo,
    ReloadFixtures,
    OpenRebootModal,
    CancelRebootModal,
    ConfirmReboot,

    // LED Pixels actions
    PixelStartInput(String),
    PixelCountInput(String),
    ColorRChanged(u8),
    ColorGChanged(u8),
    ColorBChanged(u8),
    ColorHexInput(String),
    SelectSwatch(u8, u8, u8),
    ToggleAutoCommit(bool),
    ApplyPixels,

    // File Manager / Editor actions
    RemotePathInput(String),
    ToggleDirExpand(String),
    SelectFileEntry(String),
    ReadRemoteFile(String),
    WriteRemoteFile,
    DeleteRemoteFile,
    SaveStagedToDisk,
    ClearStagedBuffer,
    ToggleHexView,
    EditorAction(text_editor::Action),

    // Effect actions
    EffectPathInput(String),
    EffectDirInput(String),
    LoadEffect,
    UnloadEffect,
    RefreshEffect,
    BrowseWasmModules,
    CommandHexInput(String),
    ParamHexInput(String),
    ReadCommandBuf,
    WriteCommandBuf,
    ReadParamBuf,
    WriteParamBuf,

    // Playlist actions
    PlaylistPathInput(String),
    PlaylistDirInput(String),
    LoadPlaylist,
    UnloadPlaylist,
    RefreshPlaylist,
    BrowsePlaylistFiles,

    // Activity Log
    ToggleLogOpen,
    ClearLogs,

    // Async RPC completions
    AsyncDone(AsyncOutcome),
}

pub struct BledApp {
    devices: Vec<ConnectedDevice>,
    selection: Option<TreeSelection>,
    collapsed_tree_nodes: HashSet<String>,
    mobile_drawer_open: bool,
    reboot_modal_open: bool,
    demo_counter: usize,
    pending_tasks: usize,

    // LED Pixel controls
    pixel_start_str: String,
    pixel_count_str: String,
    color_r: u8,
    color_g: u8,
    color_b: u8,
    color_hex: String,
    auto_commit: bool,

    // File Staging text editor state
    editor_content: text_editor::Content,

    // Effect & Playlist form state
    effect_path_input: String,
    effect_dir_input: String,
    cmd_hex_input: String,
    param_hex_input: String,
    playlist_path_input: String,
    playlist_dir_input: String,

    // Activity Log
    logs: Vec<LogEntry>,
    log_open: bool,
}

impl BledApp {
    pub fn boot() -> (Self, Task<Message>) {
        let demo = ConnectedDevice::new_demo(1);
        let demo_id = demo.id.clone();
        let initial_editor = String::from_utf8_lossy(&demo.staged_data).to_string();
        let cmd_hex = bytes_to_hex(&demo.effect_command_bytes);
        let param_hex = bytes_to_hex(&demo.effect_param_bytes);

        let mut app = Self {
            devices: vec![demo],
            selection: Some(TreeSelection::Device { device_id: demo_id }),
            collapsed_tree_nodes: HashSet::new(),
            mobile_drawer_open: false,
            reboot_modal_open: false,
            demo_counter: 1,
            pending_tasks: 0,
            pixel_start_str: "0".into(),
            pixel_count_str: "30".into(),
            color_r: 255,
            color_g: 100,
            color_b: 0,
            color_hex: "FF6400".into(),
            auto_commit: true,
            editor_content: text_editor::Content::with_text(&initial_editor),
            effect_path_input: "/effects/span-colour-chase.wasm".into(),
            effect_dir_input: "/effects".into(),
            cmd_hex_input: cmd_hex,
            param_hex_input: param_hex,
            playlist_path_input: "/playlist.json".into(),
            playlist_dir_input: "/".into(),
            logs: Vec::new(),
            log_open: false,
        };

        app.push_log(
            "Initialized iced PoC with demo device \"bled-desk-matrix\". Resize window or open on tablet/mobile to test responsive layout.",
            LogLevel::Info,
        );

        (app, Task::none())
    }

    fn push_log(&mut self, msg: impl Into<String>, level: LogLevel) {
        self.logs.push(LogEntry {
            timestamp: now_timestamp(),
            message: msg.into(),
            level,
        });
    }

    fn selected_device(&self) -> Option<&ConnectedDevice> {
        let sel = self.selection.as_ref()?;
        self.devices.iter().find(|d| d.id == sel.device_id())
    }

    fn selected_device_mut(&mut self) -> Option<&mut ConnectedDevice> {
        let dev_id = self.selection.as_ref()?.device_id().to_string();
        self.devices.iter_mut().find(|d| d.id == dev_id)
    }

    fn sync_inputs_from_selected_device(&mut self) {
        if let Some((text, fx_path, cmd_hex, param_hex, pl_path)) =
            self.selected_device().map(|dev| {
                (
                    String::from_utf8_lossy(&dev.staged_data).to_string(),
                    dev.active_effect_path.clone(),
                    bytes_to_hex(&dev.effect_command_bytes),
                    bytes_to_hex(&dev.effect_param_bytes),
                    dev.active_playlist_info
                        .as_ref()
                        .map(|info| info.path().to_string()),
                )
            })
        {
            self.editor_content = text_editor::Content::with_text(&text);
            self.effect_path_input = fx_path;
            self.cmd_hex_input = cmd_hex;
            self.param_hex_input = param_hex;
            if let Some(p) = pl_path {
                self.playlist_path_input = p;
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleMobileDrawer => {
                self.mobile_drawer_open = !self.mobile_drawer_open;
                Task::none()
            }
            Message::CloseMobileDrawer => {
                self.mobile_drawer_open = false;
                Task::none()
            }
            Message::AddDemoDevice => {
                self.demo_counter += 1;
                let dev = ConnectedDevice::new_demo(self.demo_counter);
                let id = dev.id.clone();
                let name = dev.display_name();
                self.devices.push(dev);
                self.selection = Some(TreeSelection::Device { device_id: id });
                self.sync_inputs_from_selected_device();
                self.push_log(format!("Added demo device \"{name}\""), LogLevel::Success);
                Task::none()
            }
            Message::ConnectBle => {
                self.pending_tasks += 1;
                self.push_log("Requesting Web Bluetooth device...", LogLevel::Info);
                Task::perform(
                    SendWrapper::new(async move {
                        let noop = Closure::wrap(Box::new(move || {}) as Box<dyn Fn()>);
                        let svc_uuid = bled_core::ble::SERIAL_SERVICE_UUID.to_string();
                        let rx_uuid = bled_core::ble::SERIAL_RX_UUID.to_string();
                        let tx_uuid = bled_core::ble::SERIAL_TX_UUID.to_string();
                        let res = bridge::connect_ble_js(
                            &svc_uuid,
                            &rx_uuid,
                            &tx_uuid,
                            noop.as_ref().unchecked_ref(),
                        )
                        .await;
                        noop.forget();
                        match res {
                            Ok(handle) => match RpcClient::new(handle) {
                                Ok(rpc) => AsyncOutcome::DeviceConnected(SendWrapper::new(
                                    ConnectedDevice::new(rpc),
                                )),
                                Err(e) => AsyncOutcome::ConnectFailed(e),
                            },
                            Err(e) => AsyncOutcome::ConnectFailed(format!("{e:?}")),
                        }
                    }),
                    Message::AsyncDone,
                )
            }
            Message::ConnectSerial => {
                self.pending_tasks += 1;
                self.push_log("Requesting Web Serial port...", LogLevel::Info);
                Task::perform(
                    SendWrapper::new(async move {
                        let noop = Closure::wrap(Box::new(move || {}) as Box<dyn Fn()>);
                        let res = bridge::connect_serial_js(
                            0x1209,
                            0x0003,
                            noop.as_ref().unchecked_ref(),
                        )
                        .await;
                        noop.forget();
                        match res {
                            Ok(handle) => match RpcClient::new(handle) {
                                Ok(rpc) => AsyncOutcome::DeviceConnected(SendWrapper::new(
                                    ConnectedDevice::new(rpc),
                                )),
                                Err(e) => AsyncOutcome::ConnectFailed(e),
                            },
                            Err(e) => AsyncOutcome::ConnectFailed(format!("{e:?}")),
                        }
                    }),
                    Message::AsyncDone,
                )
            }
            Message::DisconnectDevice(dev_id) => {
                if let Some(dev) = self.devices.iter().find(|d| d.id == dev_id)
                    && let Some(rpc_wrap) = &dev.rpc
                    && let Some(rpc) = rpc_wrap.try_lock()
                {
                    rpc.disconnect();
                }
                self.devices.retain(|d| d.id != dev_id);
                if self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.device_id() == dev_id)
                {
                    self.selection = self.devices.first().map(|d| TreeSelection::Device {
                        device_id: d.id.clone(),
                    });
                }
                self.push_log(format!("Disconnected device {dev_id}"), LogLevel::Info);
                Task::none()
            }
            Message::SelectTree(sel) => {
                // Update default pixel count if selecting a Fixture or Span
                if let Some(dev) = self.devices.iter().find(|d| d.id == sel.device_id())
                    && let Some(cfg) = &dev.config
                {
                    match &sel {
                        TreeSelection::Fixture { fixture_idx, .. } => {
                            if let Some(f) = cfg.fixtures.get(*fixture_idx) {
                                self.pixel_start_str = "0".into();
                                self.pixel_count_str = f.total_pixels.to_string();
                            }
                        }
                        TreeSelection::Span {
                            fixture_idx,
                            span_idx,
                            ..
                        } => {
                            if let Some(s) = cfg
                                .fixtures
                                .get(*fixture_idx)
                                .and_then(|f| f.spans.get(*span_idx))
                            {
                                self.pixel_start_str = "0".into();
                                self.pixel_count_str = s.length.to_string();
                            }
                        }
                        _ => {}
                    }
                }
                self.selection = Some(sel);
                self.mobile_drawer_open = false;
                self.sync_inputs_from_selected_device();
                Task::none()
            }
            Message::ToggleTreeCollapse(key) => {
                if !self.collapsed_tree_nodes.remove(&key) {
                    self.collapsed_tree_nodes.insert(key);
                }
                Task::none()
            }
            Message::RefreshDeviceInfo => {
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                if dev.is_demo {
                    let name = dev.display_name();
                    if let Some(d) = self.selected_device_mut()
                        && let Some(info) = &d.device_info
                    {
                        let next_uptime = info.uptime_ms() + 5_000;
                        d.device_info = Some(DeviceInfo::new(
                            info.name().clone(),
                            info.git_revision().clone(),
                            *info.boot_reason(),
                            next_uptime,
                            *info.led_buffer_capacity(),
                            *info.channel_count(),
                        ));
                    }
                    self.push_log(
                        format!("[{name}] Refreshed demo device info"),
                        LogLevel::Success,
                    );
                    return Task::none();
                }
                if let Some(rpc) = dev.rpc.clone() {
                    let dev_id = dev.id.clone();
                    let fallback_name = dev.name.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let info = client.read_device_info().await;
                            let name = info
                                .as_ref()
                                .map(|i| i.name().to_string())
                                .unwrap_or(fallback_name);
                            let config = client
                                .read_fixtures::<fn(u8)>(None)
                                .await
                                .map(|f| normalize_config(&f, &name));
                            AsyncOutcome::DeviceBootstrap {
                                device_id: dev_id,
                                info,
                                config,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::ReloadFixtures => {
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                if dev.is_demo {
                    self.push_log(
                        format!(
                            "[{}] Reloaded fixtures from /fixtures.json",
                            dev.display_name()
                        ),
                        LogLevel::Success,
                    );
                    return Task::none();
                }
                if let Some(rpc) = dev.rpc.clone() {
                    let dev_id = dev.id.clone();
                    let fallback_name = dev.name.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let _ = client.fixtures_reload().await;
                            let info = client.read_device_info().await;
                            let name = info
                                .as_ref()
                                .map(|i| i.name().to_string())
                                .unwrap_or(fallback_name);
                            let config = client
                                .read_fixtures::<fn(u8)>(None)
                                .await
                                .map(|f| normalize_config(&f, &name));
                            AsyncOutcome::DeviceBootstrap {
                                device_id: dev_id,
                                info,
                                config,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::OpenRebootModal => {
                self.reboot_modal_open = true;
                Task::none()
            }
            Message::CancelRebootModal => {
                self.reboot_modal_open = false;
                Task::none()
            }
            Message::ConfirmReboot => {
                self.reboot_modal_open = false;
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                let dev_id = dev.id.clone();
                if dev.is_demo {
                    return self.update(Message::AsyncDone(AsyncOutcome::DeviceRebooted(dev_id)));
                }
                if let Some(rpc) = dev.rpc.clone() {
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let _ = client.reboot().await;
                            client.disconnect();
                            AsyncOutcome::DeviceRebooted(dev_id)
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::PixelStartInput(val) => {
                self.pixel_start_str = val;
                Task::none()
            }
            Message::PixelCountInput(val) => {
                self.pixel_count_str = val;
                Task::none()
            }
            Message::ColorRChanged(r) => {
                self.color_r = r;
                self.color_hex = format!(
                    "{:02X}{:02X}{:02X}",
                    self.color_r, self.color_g, self.color_b
                );
                Task::none()
            }
            Message::ColorGChanged(g) => {
                self.color_g = g;
                self.color_hex = format!(
                    "{:02X}{:02X}{:02X}",
                    self.color_r, self.color_g, self.color_b
                );
                Task::none()
            }
            Message::ColorBChanged(b) => {
                self.color_b = b;
                self.color_hex = format!(
                    "{:02X}{:02X}{:02X}",
                    self.color_r, self.color_g, self.color_b
                );
                Task::none()
            }
            Message::ColorHexInput(hex) => {
                let clean = hex.trim().replace('#', "");
                if clean.len() == 6
                    && let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&clean[0..2], 16),
                        u8::from_str_radix(&clean[2..4], 16),
                        u8::from_str_radix(&clean[4..6], 16),
                    )
                {
                    self.color_r = r;
                    self.color_g = g;
                    self.color_b = b;
                }
                self.color_hex = hex;
                Task::none()
            }
            Message::SelectSwatch(r, g, b) => {
                self.color_r = r;
                self.color_g = g;
                self.color_b = b;
                self.color_hex = format!("{r:02X}{g:02X}{b:02X}");
                Task::none()
            }
            Message::ToggleAutoCommit(checked) => {
                self.auto_commit = checked;
                Task::none()
            }
            Message::ApplyPixels => {
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                let start = self.pixel_start_str.trim().parse::<usize>().unwrap_or(0);
                let count = self.pixel_count_str.trim().parse::<usize>().unwrap_or(1);
                let (r, g, b) = (self.color_r, self.color_g, self.color_b);
                let auto_commit = self.auto_commit;

                if dev.is_demo {
                    self.push_log(
                        format!(
                            "[{}] Set {count} pixel(s) from #{start} to #{r:02X}{g:02X}{b:02X} (auto_commit={auto_commit})",
                            dev.display_name()
                        ),
                        LogLevel::Success,
                    );
                    return Task::none();
                }

                if let (Some(rpc), Some(cfg), Some(sel)) =
                    (dev.rpc.clone(), dev.config.clone(), self.selection.clone())
                {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let mut writes = Vec::new();
                            match sel {
                                TreeSelection::Span {
                                    fixture_idx,
                                    span_idx,
                                    ..
                                } => {
                                    if let Some(span) = cfg
                                        .fixtures
                                        .get(fixture_idx)
                                        .and_then(|f| f.spans.get(span_idx))
                                    {
                                        let ch_off = cfg
                                            .channel_offsets
                                            .get(span.channel)
                                            .copied()
                                            .unwrap_or(0);
                                        let actual_count =
                                            count.min(span.length.saturating_sub(start));
                                        writes.push((
                                            ch_off + (span.start + start) * 3,
                                            actual_count,
                                        ));
                                    }
                                }
                                TreeSelection::Fixture { fixture_idx, .. } => {
                                    if let Some(fixture) = cfg.fixtures.get(fixture_idx) {
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
                                            let ch_off = cfg
                                                .channel_offsets
                                                .get(span.channel)
                                                .copied()
                                                .unwrap_or(0);
                                            writes.push((ch_off + s_start * 3, s_count));
                                        }
                                    }
                                }
                                _ => {}
                            }

                            for (byte_offset, px_count) in writes {
                                let mut buf = vec![0u8; px_count * 3];
                                for i in 0..px_count {
                                    buf[i * 3] = r;
                                    buf[i * 3 + 1] = g;
                                    buf[i * 3 + 2] = b;
                                }
                                if let Err(e) = client
                                    .write_led_buffer_chunked::<fn(u8)>(
                                        byte_offset as u32,
                                        &buf,
                                        None,
                                    )
                                    .await
                                {
                                    return AsyncOutcome::PixelsUpdated {
                                        device_id: dev_id,
                                        result: Err(e),
                                    };
                                }
                            }
                            if auto_commit && let Err(e) = client.commit().await {
                                return AsyncOutcome::PixelsUpdated {
                                    device_id: dev_id,
                                    result: Err(e),
                                };
                            }
                            AsyncOutcome::PixelsUpdated {
                                device_id: dev_id,
                                result: Ok(format!(
                                    "Updated {count} pixel(s) to #{r:02X}{g:02X}{b:02X}"
                                )),
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::RemotePathInput(path) => {
                if let Some(dev) = self.selected_device_mut() {
                    dev.remote_path = path;
                }
                Task::none()
            }
            Message::ToggleDirExpand(dir_path) => {
                let mut should_fetch = false;
                if let Some(dev) = self.selected_device_mut()
                    && !dev.expanded_dirs.remove(&dir_path)
                {
                    dev.expanded_dirs.insert(dir_path.clone());
                    if !dev.is_demo && !dev.tree_data.contains_key(&dir_path) {
                        should_fetch = true;
                    }
                }
                if should_fetch
                    && let Some(dev) = self.selected_device().cloned()
                    && let Some(rpc) = dev.rpc.clone()
                {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let entries = client.list_directory(&dir_path).await;
                            AsyncOutcome::DirectoryListed {
                                device_id: dev_id,
                                path: dir_path,
                                entries,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::SelectFileEntry(path) => {
                if let Some(dev) = self.selected_device_mut() {
                    dev.remote_path = path;
                }
                Task::none()
            }
            Message::ReadRemoteFile(path) => {
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                if dev.is_demo {
                    let bytes = dev.mock_files.get(&path).cloned().unwrap_or_else(|| {
                        format!("// Mock contents for {path}\n{{\n  \"status\": \"ok\"\n}}")
                            .into_bytes()
                    });
                    return self.update(Message::AsyncDone(AsyncOutcome::FileRead {
                        device_id: dev.id,
                        path,
                        bytes: Ok(bytes),
                    }));
                }
                if let Some(rpc) = dev.rpc.clone() {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let bytes = client.read_file_chunked::<fn(u8)>(&path, None).await;
                            AsyncOutcome::FileRead {
                                device_id: dev_id,
                                path,
                                bytes,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::WriteRemoteFile => {
                let editor_text = self.editor_content.text();
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                let path = dev.remote_path.clone();
                let bytes = if dev.staged_is_hex {
                    dev.staged_data.clone()
                } else {
                    editor_text.into_bytes()
                };
                if dev.is_demo {
                    if let Some(d) = self.selected_device_mut() {
                        d.mock_files.insert(path.clone(), bytes.clone());
                        d.staged_data = bytes;
                    }
                    self.push_log(
                        format!("[{}] Wrote {path} to demo storage", dev.display_name()),
                        LogLevel::Success,
                    );
                    return Task::none();
                }
                if let Some(rpc) = dev.rpc.clone() {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let result = client
                                .write_file_chunked::<fn(u8)>(&path, &bytes, None)
                                .await;
                            AsyncOutcome::FileWritten {
                                device_id: dev_id,
                                path,
                                result,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::DeleteRemoteFile => {
                let Some(dev) = self.selected_device().cloned() else {
                    return Task::none();
                };
                let path = dev.remote_path.clone();
                if dev.is_demo {
                    if let Some(d) = self.selected_device_mut() {
                        d.mock_files.remove(&path);
                    }
                    self.push_log(
                        format!("[{}] Deleted {path} from demo storage", dev.display_name()),
                        LogLevel::Success,
                    );
                    return Task::none();
                }
                if let Some(rpc) = dev.rpc.clone() {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let result = client.file_delete(&path).await;
                            AsyncOutcome::FileDeleted {
                                device_id: dev_id,
                                path,
                                result,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::SaveStagedToDisk => {
                let editor_text = self.editor_content.text();
                if let Some(dev) = self.selected_device() {
                    let bytes = if dev.staged_is_hex {
                        dev.staged_data.clone()
                    } else {
                        editor_text.into_bytes()
                    };
                    let fname = if dev.staged_filename.is_empty() {
                        "staged_file.bin"
                    } else {
                        &dev.staged_filename
                    };
                    bridge::save_file_to_disk(&bytes, fname);
                }
                Task::none()
            }
            Message::ClearStagedBuffer => {
                if let Some(dev) = self.selected_device_mut() {
                    dev.staged_data.clear();
                    dev.staged_filename.clear();
                }
                self.editor_content = text_editor::Content::new();
                Task::none()
            }
            Message::ToggleHexView => {
                let editor_text = self.editor_content.text();
                if let Some(dev) = self.selected_device_mut() {
                    if !dev.staged_is_hex {
                        dev.staged_data = editor_text.into_bytes();
                    }
                    dev.staged_is_hex = !dev.staged_is_hex;
                }
                Task::none()
            }
            Message::EditorAction(action) => {
                self.editor_content.perform(action);
                Task::none()
            }
            Message::EffectPathInput(val) => {
                self.effect_path_input = val;
                Task::none()
            }
            Message::EffectDirInput(val) => {
                self.effect_dir_input = val;
                Task::none()
            }
            Message::LoadEffect => {
                let path = self.effect_path_input.trim().to_string();
                if path.is_empty() {
                    self.push_log("Enter a Wasm module path first", LogLevel::Error);
                    return Task::none();
                }
                if let Some(dev) = self.selected_device_mut()
                    && dev.is_demo
                {
                    dev.active_effect_path = path.clone();
                    let name = dev.display_name();
                    self.push_log(
                        format!("[{name}] Loaded effect \"{path}\""),
                        LogLevel::Success,
                    );
                    return Task::none();
                }
                if let Some(dev) = self.selected_device().cloned()
                    && let Some(rpc) = dev.rpc.clone()
                {
                    let dev_id = dev.id.clone();
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let _ = client.set_active_effect(Some(&path)).await;
                            let active = client
                                .get_active_effect()
                                .await
                                .ok()
                                .flatten()
                                .unwrap_or_default();
                            let manifest = client.read_effect_manifest().await.ok();
                            let cmd_len = client.effect_command_stat().await.ok();
                            let param_len = client.effect_parameter_stat().await.ok();
                            let cmd_bytes = client.effect_command_read().await.unwrap_or_default();
                            let param_bytes =
                                client.effect_parameter_read().await.unwrap_or_default();
                            AsyncOutcome::EffectStateLoaded {
                                device_id: dev_id,
                                path: active,
                                manifest,
                                cmd_bytes,
                                param_bytes,
                                cmd_len,
                                param_len,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            Message::UnloadEffect => {
                if let Some(dev) = self.selected_device_mut()
                    && dev.is_demo
                {
                    dev.active_effect_path.clear();
                    let name = dev.display_name();
                    self.push_log(format!("[{name}] Unloaded active effect"), LogLevel::Info);
                    return Task::none();
                }
                Task::none()
            }
            Message::RefreshEffect | Message::BrowseWasmModules => {
                self.push_log("Refreshed Wasm effect module state", LogLevel::Info);
                Task::none()
            }
            Message::CommandHexInput(val) => {
                self.cmd_hex_input = val;
                Task::none()
            }
            Message::ParamHexInput(val) => {
                self.param_hex_input = val;
                Task::none()
            }
            Message::ReadCommandBuf => {
                if let Some(dev) = self.selected_device() {
                    self.cmd_hex_input = bytes_to_hex(&dev.effect_command_bytes);
                    self.push_log("Read command buffer", LogLevel::Success);
                }
                Task::none()
            }
            Message::WriteCommandBuf => {
                match parse_hex(&self.cmd_hex_input) {
                    Ok(bytes) => {
                        let len = bytes.len();
                        if let Some(dev) = self.selected_device_mut() {
                            dev.effect_command_bytes = bytes;
                        }
                        self.push_log(format!("Wrote {len} command byte(s)"), LogLevel::Success);
                    }
                    Err(e) => self.push_log(e, LogLevel::Error),
                }
                Task::none()
            }
            Message::ReadParamBuf => {
                if let Some(dev) = self.selected_device() {
                    self.param_hex_input = bytes_to_hex(&dev.effect_param_bytes);
                    self.push_log("Read parameter buffer", LogLevel::Success);
                }
                Task::none()
            }
            Message::WriteParamBuf => {
                match parse_hex(&self.param_hex_input) {
                    Ok(bytes) => {
                        let len = bytes.len();
                        if let Some(dev) = self.selected_device_mut() {
                            dev.effect_param_bytes = bytes;
                        }
                        self.push_log(format!("Wrote {len} parameter byte(s)"), LogLevel::Success);
                    }
                    Err(e) => self.push_log(e, LogLevel::Error),
                }
                Task::none()
            }
            Message::PlaylistPathInput(val) => {
                self.playlist_path_input = val;
                Task::none()
            }
            Message::PlaylistDirInput(val) => {
                self.playlist_dir_input = val;
                Task::none()
            }
            Message::LoadPlaylist => {
                let path = self.playlist_path_input.trim().to_string();
                if let Some(dev) = self.selected_device_mut()
                    && dev.is_demo
                    && let Ok(ps) = bled_core::PathString::try_from(path.as_str())
                {
                    dev.active_playlist_info = Some(ActivePlaylistInfo::new(ps, Some(0)));
                }
                self.push_log(format!("Loaded playlist \"{path}\""), LogLevel::Success);
                Task::none()
            }
            Message::UnloadPlaylist => {
                if let Some(dev) = self.selected_device_mut()
                    && dev.is_demo
                {
                    dev.active_playlist_info = None;
                }
                self.push_log("Unloaded active playlist", LogLevel::Info);
                Task::none()
            }
            Message::RefreshPlaylist | Message::BrowsePlaylistFiles => {
                self.push_log("Refreshed playlist state", LogLevel::Info);
                Task::none()
            }
            Message::ToggleLogOpen => {
                self.log_open = !self.log_open;
                Task::none()
            }
            Message::ClearLogs => {
                self.logs.clear();
                Task::none()
            }
            Message::AsyncDone(outcome) => self.handle_async_outcome(outcome),
        }
    }

    fn handle_async_outcome(&mut self, outcome: AsyncOutcome) -> Task<Message> {
        match outcome {
            AsyncOutcome::DeviceConnected(wrapped_dev) => {
                self.pending_tasks = self.pending_tasks.saturating_sub(1);
                let dev = wrapped_dev.take();
                let dev_id = dev.id.clone();
                let dev_name = dev.name.clone();
                let rpc_opt = dev.rpc.clone();
                self.devices.push(dev);
                self.selection = Some(TreeSelection::Device {
                    device_id: dev_id.clone(),
                });
                self.push_log(format!("Connected to \"{dev_name}\""), LogLevel::Success);

                if let Some(rpc) = rpc_opt {
                    return Task::perform(
                        SendWrapper::new(async move {
                            let mut client = rpc.lock().await;
                            let info = client.read_device_info().await;
                            let resolved_name = info
                                .as_ref()
                                .map(|i| i.name().to_string())
                                .unwrap_or(dev_name);
                            let config = client
                                .read_fixtures::<fn(u8)>(None)
                                .await
                                .map(|f| normalize_config(&f, &resolved_name));
                            AsyncOutcome::DeviceBootstrap {
                                device_id: dev_id,
                                info,
                                config,
                            }
                        }),
                        Message::AsyncDone,
                    );
                }
                Task::none()
            }
            AsyncOutcome::ConnectFailed(err) => {
                self.pending_tasks = self.pending_tasks.saturating_sub(1);
                self.push_log(format!("Connection failed: {err}"), LogLevel::Error);
                Task::none()
            }
            AsyncOutcome::DeviceBootstrap {
                device_id,
                info,
                config,
            } => {
                if let Some(dev) = self.devices.iter_mut().find(|d| d.id == device_id) {
                    if let Ok(i) = info {
                        dev.device_info = Some(i);
                    }
                    match config {
                        Ok(cfg) => {
                            dev.config = Some(cfg);
                            dev.config_error = None;
                        }
                        Err(e) => {
                            dev.config = None;
                            dev.config_error = Some(e);
                        }
                    }
                }
                Task::none()
            }
            AsyncOutcome::DirectoryListed {
                device_id,
                path,
                entries,
            } => {
                if let Some(dev) = self.devices.iter_mut().find(|d| d.id == device_id) {
                    match entries {
                        Ok(mut list) => {
                            list.sort_by(|a, b| {
                                let a_dir = matches!(a.kind(), DirEntryType::Directory);
                                let b_dir = matches!(b.kind(), DirEntryType::Directory);
                                match (a_dir, b_dir) {
                                    (true, false) => std::cmp::Ordering::Less,
                                    (false, true) => std::cmp::Ordering::Greater,
                                    _ => a.name().cmp(b.name()),
                                }
                            });
                            dev.tree_data.insert(path, list);
                        }
                        Err(e) => {
                            self.push_log(format!("Directory list failed: {e}"), LogLevel::Error);
                        }
                    }
                }
                Task::none()
            }
            AsyncOutcome::FileRead {
                device_id,
                path,
                bytes,
            } => {
                match bytes {
                    Ok(data) => {
                        let fname = path.split('/').next_back().unwrap_or("file").to_string();
                        let text_opt = String::from_utf8(data.clone()).ok();
                        if let Some(dev) = self.devices.iter_mut().find(|d| d.id == device_id) {
                            dev.remote_path = path.clone();
                            dev.staged_filename = fname;
                            dev.staged_data = data;
                            dev.staged_is_hex = text_opt.is_none();
                        }
                        if let Some(text) = text_opt {
                            self.editor_content = text_editor::Content::with_text(&text);
                        }
                        self.push_log(
                            format!("Read {path} into staging buffer"),
                            LogLevel::Success,
                        );
                    }
                    Err(e) => {
                        self.push_log(format!("Failed reading {path}: {e}"), LogLevel::Error);
                    }
                }
                Task::none()
            }
            AsyncOutcome::FileWritten { path, result, .. } => {
                match result {
                    Ok(()) => self.push_log(format!("Wrote {path}"), LogLevel::Success),
                    Err(e) => self.push_log(format!("Failed writing {path}: {e}"), LogLevel::Error),
                }
                Task::none()
            }
            AsyncOutcome::FileDeleted { path, result, .. } => {
                match result {
                    Ok(()) => self.push_log(format!("Deleted {path}"), LogLevel::Success),
                    Err(e) => {
                        self.push_log(format!("Failed deleting {path}: {e}"), LogLevel::Error)
                    }
                }
                Task::none()
            }
            AsyncOutcome::EffectStateLoaded {
                device_id,
                path,
                manifest,
                cmd_bytes,
                param_bytes,
                cmd_len,
                param_len,
            } => {
                if let Some(dev) = self.devices.iter_mut().find(|d| d.id == device_id) {
                    dev.active_effect_path = path.clone();
                    dev.active_effect_manifest = manifest;
                    dev.effect_command_bytes = cmd_bytes.clone();
                    dev.effect_param_bytes = param_bytes.clone();
                    dev.effect_command_len = cmd_len;
                    dev.effect_param_len = param_len;
                }
                self.effect_path_input = path;
                self.cmd_hex_input = bytes_to_hex(&cmd_bytes);
                self.param_hex_input = bytes_to_hex(&param_bytes);
                Task::none()
            }
            AsyncOutcome::PlaylistLoaded {
                device_id,
                info,
                playlist,
            } => {
                if let Some(dev) = self.devices.iter_mut().find(|d| d.id == device_id) {
                    dev.active_playlist_info = info;
                    dev.active_playlist = playlist;
                }
                Task::none()
            }
            AsyncOutcome::PixelsUpdated { result, .. } => {
                match result {
                    Ok(msg) => self.push_log(msg, LogLevel::Success),
                    Err(e) => self.push_log(format!("Pixel update failed: {e}"), LogLevel::Error),
                }
                Task::none()
            }
            AsyncOutcome::DeviceRebooted(dev_id) => {
                self.devices.retain(|d| d.id != dev_id);
                if self
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.device_id() == dev_id)
                {
                    self.selection = self.devices.first().map(|d| TreeSelection::Device {
                        device_id: d.id.clone(),
                    });
                }
                self.push_log(
                    format!("Rebooted and disconnected device {dev_id}"),
                    LogLevel::Info,
                );
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        responsive(move |size| {
            let is_desktop = size.width >= 860.0;
            let is_tablet = (600.0..860.0).contains(&size.width);
            let mode_badge = if is_desktop {
                format!("Desktop ({:.0}×{:.0})", size.width, size.height)
            } else if is_tablet {
                format!("Tablet ({:.0}×{:.0})", size.width, size.height)
            } else {
                format!("Mobile ({:.0}×{:.0})", size.width, size.height)
            };

            let header = self.view_header(is_desktop, mode_badge);

            let body: Element<'_, Message> = if is_desktop {
                row![
                    container(scrollable(self.view_device_tree()))
                        .width(270)
                        .height(Length::Fill)
                        .padding(14)
                        .style(sidebar_style),
                    container(
                        scrollable(
                            container(self.view_main_content(true))
                                .width(Length::Fill)
                                .padding(20)
                        )
                        .height(Length::Fill)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                ]
                .height(Length::Fill)
                .into()
            } else {
                let main_area = container(
                    scrollable(
                        container(self.view_main_content(false))
                            .width(Length::Fill)
                            .padding(12),
                    )
                    .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill);

                if self.mobile_drawer_open {
                    let drawer_width = (size.width * 0.82).min(310.0);
                    let drawer = row![
                        opaque(
                            container(
                                column![
                                    row![
                                        text("Device Tree").size(15).color(Color::WHITE),
                                        space().width(Length::Fill),
                                        button(text("✕ Close").size(12))
                                            .on_press(Message::CloseMobileDrawer)
                                            .padding([6, 10])
                                            .style(button::secondary),
                                    ]
                                    .align_y(Alignment::Center),
                                    rule::horizontal(1),
                                    scrollable(self.view_device_tree()).height(Length::Fill),
                                ]
                                .spacing(10),
                            )
                            .width(drawer_width)
                            .height(Length::Fill)
                            .padding(14)
                            .style(drawer_style),
                        ),
                        button(space().width(Length::Fill).height(Length::Fill))
                            .on_press(Message::CloseMobileDrawer)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .style(backdrop_button_style),
                    ]
                    .width(Length::Fill)
                    .height(Length::Fill);

                    stack![main_area, drawer].into()
                } else {
                    main_area.into()
                }
            };

            let log_panel = self.view_activity_log();

            let base_screen: Element<'_, Message> = container(
                column![header, body, log_panel]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(app_bg_style)
            .into();

            if self.reboot_modal_open {
                let dev_name = self
                    .selected_device()
                    .map(|d| d.display_name())
                    .unwrap_or_else(|| "device".into());
                let modal = opaque(
                    center(
                        container(
                            column![
                                text("Reboot Device").size(18).color(Color::WHITE),
                                text(format!("Are you sure you want to reboot \"{dev_name}\"?"))
                                    .size(14)
                                    .color(Color::from_rgb8(200, 206, 218)),
                                row![
                                    space().width(Length::Fill),
                                    button(text("Cancel").size(13))
                                        .on_press(Message::CancelRebootModal)
                                        .padding([8, 16])
                                        .style(button::secondary),
                                    button(text("Reboot").size(13))
                                        .on_press(Message::ConfirmReboot)
                                        .padding([8, 16])
                                        .style(button::danger),
                                ]
                                .spacing(10),
                            ]
                            .spacing(16)
                            .width(320),
                        )
                        .padding(20)
                        .style(modal_card_style),
                    )
                    .style(backdrop_container_style),
                );
                stack![base_screen, modal].into()
            } else {
                base_screen
            }
        })
        .into()
    }

    fn view_header(&self, is_desktop: bool, mode_badge: String) -> Element<'_, Message> {
        let git_rev = git_version::git_version!(args = ["--always", "--dirty=-modified"]);

        let left_group = if is_desktop {
            row![
                text("BLED").size(20).color(Color::WHITE),
                text(format!("({git_rev})"))
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb8(140, 150, 168)),
                container(
                    text(mode_badge)
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(96, 165, 250))
                )
                .padding([4, 8])
                .style(badge_style),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        } else {
            let active_label = self
                .selection
                .as_ref()
                .map(|s| s.section_label())
                .unwrap_or_else(|| "Select Device".into());

            row![
                button(text(format!("☰ {active_label}")).size(13))
                    .on_press(Message::ToggleMobileDrawer)
                    .padding([8, 12])
                    .style(button::secondary),
                text("BLED").size(17).color(Color::WHITE),
                container(
                    text(mode_badge)
                        .size(10)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(96, 165, 250))
                )
                .padding([3, 6])
                .style(badge_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        let btn_padding = if is_desktop { [7, 12] } else { [8, 10] };
        let ble_label = if is_desktop { "Connect BLE" } else { "BLE" };
        let usb_label = if is_desktop {
            "Connect USB Serial"
        } else {
            "USB"
        };

        let actions = row![
            button(text("+ Demo").size(12))
                .on_press(Message::AddDemoDevice)
                .padding(btn_padding)
                .style(button::secondary),
            button(text(ble_label).size(12))
                .on_press(Message::ConnectBle)
                .padding(btn_padding)
                .style(button::primary),
            button(text(usb_label).size(12))
                .on_press(Message::ConnectSerial)
                .padding(btn_padding)
                .style(button::primary),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        container(
            row![left_group, space().width(Length::Fill), actions]
                .spacing(8)
                .align_y(Alignment::Center)
                .wrap(),
        )
        .width(Length::Fill)
        .padding([10, 14])
        .style(header_style)
        .into()
    }

    fn view_device_tree(&self) -> Element<'_, Message> {
        let mut col = Column::new().spacing(6).width(Length::Fill);

        col = col.push(
            text("DEVICES")
                .size(11)
                .color(Color::from_rgb8(148, 163, 184)),
        );

        if self.devices.is_empty() {
            col = col.push(
                text("No devices connected. Tap '+ Demo', 'BLE', or 'USB'.")
                    .size(12)
                    .color(Color::from_rgb8(110, 120, 138)),
            );
            return col.into();
        }

        for dev in &self.devices {
            let dev_id = dev.id.clone();
            let dev_key = format!("d:{dev_id}");
            let is_collapsed = self.collapsed_tree_nodes.contains(&dev_key);

            let is_dev_sel = matches!(
                &self.selection,
                Some(TreeSelection::Device { device_id }) if device_id == &dev_id
            );
            let is_sd_sel = matches!(
                &self.selection,
                Some(TreeSelection::SdCard { device_id }) if device_id == &dev_id
            );
            let is_fx_sel = matches!(
                &self.selection,
                Some(TreeSelection::Effect { device_id }) if device_id == &dev_id
            );
            let is_pl_sel = matches!(
                &self.selection,
                Some(TreeSelection::Playlist { device_id }) if device_id == &dev_id
            );

            let arrow = if is_collapsed { "▸" } else { "▾" };

            let dev_row = row![
                button(text(arrow).size(12))
                    .on_press(Message::ToggleTreeCollapse(dev_key))
                    .padding([6, 8])
                    .style(button::text),
                button(text(dev.display_name()).size(13).color(if is_dev_sel {
                    Color::WHITE
                } else {
                    Color::from_rgb8(215, 222, 234)
                }))
                .on_press(Message::SelectTree(TreeSelection::Device {
                    device_id: dev_id.clone(),
                }))
                .width(Length::Fill)
                .padding([6, 8])
                .style(if is_dev_sel {
                    button::primary
                } else {
                    button::text
                }),
                button(text("✕").size(11))
                    .on_press(Message::DisconnectDevice(dev_id.clone()))
                    .padding([6, 8])
                    .style(button::danger),
            ]
            .spacing(2)
            .align_y(Alignment::Center);

            col = col.push(dev_row);

            if !is_collapsed {
                let mut sub = Column::new().spacing(4).padding(Padding {
                    top: 2.0,
                    right: 0.0,
                    bottom: 6.0,
                    left: 16.0,
                });

                sub = sub.push(tree_leaf_button(
                    "📁 SD Card",
                    is_sd_sel,
                    Message::SelectTree(TreeSelection::SdCard {
                        device_id: dev_id.clone(),
                    }),
                ));
                sub = sub.push(tree_leaf_button(
                    "✨ Effects",
                    is_fx_sel,
                    Message::SelectTree(TreeSelection::Effect {
                        device_id: dev_id.clone(),
                    }),
                ));
                sub = sub.push(tree_leaf_button(
                    "☰ Playlist",
                    is_pl_sel,
                    Message::SelectTree(TreeSelection::Playlist {
                        device_id: dev_id.clone(),
                    }),
                ));

                if let Some(cfg) = &dev.config {
                    for (f_idx, fixture) in cfg.fixtures.iter().enumerate() {
                        let f_key = format!("d:{dev_id}:f:{f_idx}");
                        let f_collapsed = self.collapsed_tree_nodes.contains(&f_key);
                        let is_f_sel = matches!(
                            &self.selection,
                            Some(TreeSelection::Fixture { device_id, fixture_idx })
                            if device_id == &dev_id && *fixture_idx == f_idx
                        );

                        let f_arrow = if f_collapsed { "▸" } else { "▾" };
                        let f_label = format!("💡 {} ({} px)", fixture.name, fixture.total_pixels);

                        sub = sub.push(
                            row![
                                button(text(f_arrow).size(11))
                                    .on_press(Message::ToggleTreeCollapse(f_key))
                                    .padding([5, 6])
                                    .style(button::text),
                                tree_leaf_button(
                                    f_label,
                                    is_f_sel,
                                    Message::SelectTree(TreeSelection::Fixture {
                                        device_id: dev_id.clone(),
                                        fixture_idx: f_idx,
                                    }),
                                ),
                            ]
                            .spacing(2)
                            .align_y(Alignment::Center),
                        );

                        if !f_collapsed {
                            let mut span_col = Column::new().spacing(3).padding(Padding {
                                top: 0.0,
                                right: 0.0,
                                bottom: 2.0,
                                left: 18.0,
                            });
                            for (s_idx, span) in fixture.spans.iter().enumerate() {
                                let is_s_sel = matches!(
                                    &self.selection,
                                    Some(TreeSelection::Span { device_id, fixture_idx, span_idx })
                                    if device_id == &dev_id && *fixture_idx == f_idx && *span_idx == s_idx
                                );
                                let s_label = format!(
                                    "▪ {} (Ch {}, {}..{})",
                                    span.name,
                                    span.channel,
                                    span.start,
                                    span.start + span.length
                                );
                                span_col = span_col.push(tree_leaf_button(
                                    s_label,
                                    is_s_sel,
                                    Message::SelectTree(TreeSelection::Span {
                                        device_id: dev_id.clone(),
                                        fixture_idx: f_idx,
                                        span_idx: s_idx,
                                    }),
                                ));
                            }
                            sub = sub.push(span_col);
                        }
                    }
                }

                col = col.push(sub);
            }
        }

        col.into()
    }

    fn view_main_content(&self, is_desktop: bool) -> Element<'_, Message> {
        let Some(sel) = &self.selection else {
            return container(
                text("Connect or add a demo device, then select a node in the Device Tree.")
                    .size(14)
                    .color(Color::from_rgb8(148, 163, 184)),
            )
            .padding(32)
            .center_x(Length::Fill)
            .style(card_style)
            .into();
        };

        let Some(dev) = self.selected_device() else {
            return container(text("Device not found.").size(14))
                .padding(24)
                .style(card_style)
                .into();
        };

        match sel {
            TreeSelection::Device { .. } => self.view_device_info(dev),
            TreeSelection::SdCard { .. } => self.view_file_staging(dev, is_desktop),
            TreeSelection::Effect { .. } => self.view_effect_panel(dev, is_desktop),
            TreeSelection::Playlist { .. } => self.view_playlist_panel(dev),
            TreeSelection::Fixture { fixture_idx, .. } => {
                self.view_led_pixels(dev, Some(*fixture_idx), None, is_desktop)
            }
            TreeSelection::Span {
                fixture_idx,
                span_idx,
                ..
            } => self.view_led_pixels(dev, Some(*fixture_idx), Some(*span_idx), is_desktop),
        }
    }

    fn view_device_info<'a>(&'a self, dev: &'a ConnectedDevice) -> Element<'a, Message> {
        let info = dev.device_info.as_ref();
        let rev = info
            .map(|i| i.git_revision().to_string())
            .unwrap_or_else(|| "—".into());
        let boot = info
            .map(|i| format!("{:?}", i.boot_reason()))
            .unwrap_or_else(|| "—".into());
        let uptime = info
            .map(|i| format_uptime(*i.uptime_ms()))
            .unwrap_or_else(|| "—".into());
        let channels = info
            .map(|i| i.channel_count().to_string())
            .unwrap_or_else(|| "—".into());
        let capacity = info
            .map(|i| format_bytes(*i.led_buffer_capacity()))
            .unwrap_or_else(|| "—".into());

        let header = row![
            column![
                text("Device Information").size(17).color(Color::WHITE),
                text(format!(
                    "{} {}",
                    dev.display_name(),
                    if dev.is_demo { "(Demo Mode)" } else { "" }
                ))
                .size(12)
                .font(Font::MONOSPACE)
                .color(Color::from_rgb8(96, 165, 250)),
            ]
            .spacing(2),
            space().width(Length::Fill),
            row![
                button(text("Refresh Info").size(12))
                    .on_press(Message::RefreshDeviceInfo)
                    .padding([8, 12])
                    .style(button::secondary),
                button(text("Reload Fixtures").size(12))
                    .on_press(Message::ReloadFixtures)
                    .padding([8, 12])
                    .style(button::secondary),
                button(text("Reboot Device").size(12))
                    .on_press(Message::OpenRebootModal)
                    .padding([8, 12])
                    .style(button::danger),
            ]
            .spacing(8)
            .wrap(),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .wrap();

        let metrics = row![
            metric_tile("GIT REVISION", rev),
            metric_tile("BOOT REASON", boot),
            metric_tile("UPTIME", uptime),
            metric_tile("OUTPUT CHANNELS", channels),
            metric_tile("BUFFER CAPACITY", capacity),
        ]
        .spacing(10)
        .wrap();

        let mut content = column![header, rule::horizontal(1), metrics].spacing(14);

        if let Some(err) = &dev.config_error {
            content = content.push(
                container(
                    text(format!("Could not read active fixtures: {err}"))
                        .size(12)
                        .color(Color::from_rgb8(252, 165, 165)),
                )
                .padding(12)
                .width(Length::Fill)
                .style(subcard_style),
            );
        }

        container(content)
            .padding(18)
            .width(Length::Fill)
            .style(card_style)
            .into()
    }

    fn view_led_pixels<'a>(
        &'a self,
        dev: &'a ConnectedDevice,
        fixture_idx: Option<usize>,
        span_idx: Option<usize>,
        is_desktop: bool,
    ) -> Element<'a, Message> {
        let mut target_desc = "LED Pixels".to_string();
        let mut limit_desc: Option<String> = None;

        if let Some(cfg) = &dev.config {
            if let (Some(f_idx), Some(s_idx)) = (fixture_idx, span_idx) {
                if let Some(span) = cfg.fixtures.get(f_idx).and_then(|f| f.spans.get(s_idx)) {
                    let f_name = &cfg.fixtures[f_idx].name;
                    target_desc = format!(
                        "{f_name} / {} — Ch {} [{}..{}] ({} px)",
                        span.name,
                        span.channel,
                        span.start,
                        span.start + span.length,
                        span.length
                    );
                    let mode_label = match span.brightness_limit.mode() {
                        BrightnessLimitMode::Clamp => "Clamp",
                        BrightnessLimitMode::Rescale => "Rescale",
                    };
                    let lim = span.brightness_limit.limit();
                    limit_desc = Some(format!(
                        "Limit: {mode_label} (R:{}, G:{}, B:{})",
                        lim.r, lim.g, lim.b
                    ));
                }
            } else if let Some(f_idx) = fixture_idx
                && let Some(fixture) = cfg.fixtures.get(f_idx)
            {
                target_desc = format!(
                    "{}: entire fixture ({} px, {})",
                    fixture.name, fixture.total_pixels, fixture.layout_name
                );
            }
        }

        let swatches: [(&str, u8, u8, u8); 9] = [
            ("Red", 255, 0, 0),
            ("Green", 0, 255, 0),
            ("Blue", 0, 0, 255),
            ("Cyan", 0, 255, 255),
            ("Magenta", 255, 0, 255),
            ("Yellow", 255, 255, 0),
            ("White", 255, 255, 255),
            ("Warm", 255, 167, 87),
            ("Off", 0, 0, 0),
        ];

        let mut swatch_row = Row::new().spacing(8);
        for (name, r, g, b) in swatches {
            let dot = container(space().width(12).height(12)).style(move |_| container::Style {
                background: Some(Color::from_rgb8(r, g, b).into()),
                border: border::rounded(6)
                    .width(1)
                    .color(Color::from_rgb8(90, 100, 120)),
                ..container::Style::default()
            });
            swatch_row = swatch_row.push(
                button(
                    row![dot, text(name).size(12)]
                        .spacing(6)
                        .align_y(Alignment::Center),
                )
                .on_press(Message::SelectSwatch(r, g, b))
                .padding([8, 12])
                .style(button::secondary),
            );
        }

        let left_col = container(
            column![
                text("RANGE & PRESETS")
                    .size(11)
                    .color(Color::from_rgb8(148, 163, 184)),
                row![
                    column![
                        text("Start Pixel").size(12),
                        text_input("0", &self.pixel_start_str)
                            .on_input(Message::PixelStartInput)
                            .padding(8)
                            .size(13),
                    ]
                    .spacing(4)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Count").size(12),
                        text_input("1", &self.pixel_count_str)
                            .on_input(Message::PixelCountInput)
                            .padding(8)
                            .size(13),
                    ]
                    .spacing(4)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(10),
                rule::horizontal(1),
                text("COLOR SWATCHES")
                    .size(11)
                    .color(Color::from_rgb8(148, 163, 184)),
                swatch_row.wrap(),
            ]
            .spacing(12),
        )
        .padding(14)
        .width(Length::Fill)
        .style(subcard_style);

        let (cr, cg, cb) = (self.color_r, self.color_g, self.color_b);
        let preview_box =
            container(space().width(48).height(48)).style(move |_| container::Style {
                background: Some(Color::from_rgb8(cr, cg, cb).into()),
                border: border::rounded(10)
                    .width(1)
                    .color(Color::from_rgb8(100, 116, 139)),
                ..container::Style::default()
            });

        let right_col = container(
            column![
                text("COLOR VALUES & TOUCH SLIDERS")
                    .size(11)
                    .color(Color::from_rgb8(148, 163, 184)),
                row![
                    preview_box,
                    column![
                        text("Hex (RRGGBB)").size(12),
                        text_input("FF6400", &self.color_hex)
                            .on_input(Message::ColorHexInput)
                            .font(Font::MONOSPACE)
                            .padding(8)
                            .size(13),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                column![
                    row![
                        text("Red").size(12).width(48),
                        slider(0..=255, self.color_r, Message::ColorRChanged).height(22),
                        text(format!("{:3}", self.color_r))
                            .size(12)
                            .font(Font::MONOSPACE)
                            .width(34),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    row![
                        text("Green").size(12).width(48),
                        slider(0..=255, self.color_g, Message::ColorGChanged).height(22),
                        text(format!("{:3}", self.color_g))
                            .size(12)
                            .font(Font::MONOSPACE)
                            .width(34),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    row![
                        text("Blue").size(12).width(48),
                        slider(0..=255, self.color_b, Message::ColorBChanged).height(22),
                        text(format!("{:3}", self.color_b))
                            .size(12)
                            .font(Font::MONOSPACE)
                            .width(34),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                ]
                .spacing(10),
            ]
            .spacing(12),
        )
        .padding(14)
        .width(Length::Fill)
        .style(subcard_style);

        let body_grid: Element<'a, Message> = if is_desktop {
            row![left_col, right_col].spacing(14).into()
        } else {
            column![left_col, right_col].spacing(12).into()
        };

        let mut header_row = row![
            column![
                text("LED Pixels").size(17).color(Color::WHITE),
                text(target_desc)
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb8(96, 165, 250)),
            ]
            .spacing(2),
            space().width(Length::Fill),
        ]
        .align_y(Alignment::Center);

        if let Some(lim) = limit_desc {
            header_row = header_row.push(
                container(
                    text(lim)
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(203, 213, 225)),
                )
                .padding([6, 10])
                .style(badge_style),
            );
        }

        let footer = row![
            toggler(self.auto_commit)
                .label("Auto-commit on write")
                .on_toggle(Message::ToggleAutoCommit)
                .text_size(13),
            space().width(Length::Fill),
            button(text("Set Colours").size(13))
                .on_press(Message::ApplyPixels)
                .padding([10, 18])
                .style(button::primary),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .wrap();

        container(
            column![
                header_row,
                rule::horizontal(1),
                body_grid,
                rule::horizontal(1),
                footer
            ]
            .spacing(14),
        )
        .padding(18)
        .width(Length::Fill)
        .style(card_style)
        .into()
    }

    fn view_file_staging<'a>(
        &'a self,
        dev: &'a ConnectedDevice,
        is_desktop: bool,
    ) -> Element<'a, Message> {
        let mut tree_col = Column::new().spacing(4).width(Length::Fill);
        let root_expanded = dev.expanded_dirs.contains("/");
        let root_arrow = if root_expanded { "▾" } else { "▸" };
        tree_col = tree_col.push(
            row![
                button(text(root_arrow).size(12))
                    .on_press(Message::ToggleDirExpand("/".into()))
                    .padding([4, 8])
                    .style(button::text),
                text("💾 / (SD Card Root)").size(13).color(Color::WHITE),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        );

        if root_expanded {
            tree_col = self.push_dir_entries(dev, "/", 1, tree_col);
        }

        let explorer_card = container(
            column![
                row![
                    text("STORAGE TREE")
                        .size(11)
                        .color(Color::from_rgb8(148, 163, 184)),
                    space().width(Length::Fill),
                    text("Tap file to select • 'Read' to load")
                        .size(11)
                        .color(Color::from_rgb8(100, 116, 139)),
                ]
                .align_y(Alignment::Center),
                rule::horizontal(1),
                scrollable(tree_col).height(if is_desktop { 340 } else { 200 }),
            ]
            .spacing(10),
        )
        .padding(14)
        .width(if is_desktop {
            Length::FillPortion(5)
        } else {
            Length::Fill
        })
        .style(subcard_style);

        let viewer_content: Element<'a, Message> = if dev.staged_is_hex {
            let (rows, truncated) = format_hex_rows(&dev.staged_data, 2048);
            let mut hex_col = Column::new().spacing(3).width(Length::Fill);
            hex_col = hex_col.push(
                text("OFFSET   00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F   ASCII")
                    .size(11)
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb8(96, 165, 250)),
            );
            for r in rows {
                hex_col = hex_col.push(
                    text(format!(
                        "{}   {:<23}  {:<23}   {}",
                        r.offset_str, r.hex_left, r.hex_right, r.ascii
                    ))
                    .size(11)
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb8(210, 218, 230)),
                );
            }
            if truncated > 0 {
                hex_col = hex_col.push(
                    text(format!("... ({truncated} more bytes)"))
                        .size(11)
                        .color(Color::from_rgb8(251, 191, 36)),
                );
            }
            container(scrollable(hex_col).height(260))
                .padding(10)
                .width(Length::Fill)
                .style(app_bg_style)
                .into()
        } else {
            text_editor(&self.editor_content)
                .placeholder("Select a file and tap Read, or type directly here...")
                .on_action(Message::EditorAction)
                .font(Font::MONOSPACE)
                .size(12)
                .height(260)
                .into()
        };

        let mode_toggle_label = if dev.staged_is_hex {
            "Switch to Text Editor"
        } else {
            "Switch to Hex Dump"
        };

        let mut editor_col = column![
            row![
                text(if dev.staged_filename.is_empty() {
                    "<Staging Buffer>".to_string()
                } else {
                    format!("📄 {}", dev.staged_filename)
                })
                .size(13)
                .font(Font::MONOSPACE)
                .color(Color::WHITE),
                space().width(Length::Fill),
                button(text(mode_toggle_label).size(12))
                    .on_press(Message::ToggleHexView)
                    .padding([6, 10])
                    .style(button::secondary),
                button(text("Save Disk").size(12))
                    .on_press(Message::SaveStagedToDisk)
                    .padding([6, 10])
                    .style(button::secondary),
                button(text("Clear").size(12))
                    .on_press(Message::ClearStagedBuffer)
                    .padding([6, 10])
                    .style(button::secondary),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .wrap(),
            row![
                text_input("/fixtures.json", &dev.remote_path)
                    .on_input(Message::RemotePathInput)
                    .font(Font::MONOSPACE)
                    .padding(8)
                    .size(13)
                    .width(Length::Fill),
                button(text("Read").size(12))
                    .on_press(Message::ReadRemoteFile(dev.remote_path.clone()))
                    .padding([8, 12])
                    .style(button::primary),
                button(text("Write").size(12))
                    .on_press(Message::WriteRemoteFile)
                    .padding([8, 12])
                    .style(button::primary),
                button(text("Delete").size(12))
                    .on_press(Message::DeleteRemoteFile)
                    .padding([8, 12])
                    .style(button::danger),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        ]
        .spacing(10);

        if let Some(pct) = dev.transfer_progress {
            editor_col = editor_col.push(progress_bar(0.0..=100.0, f32::from(pct)));
        }

        editor_col = editor_col.push(viewer_content);

        let editor_card = container(editor_col)
            .padding(14)
            .width(if is_desktop {
                Length::FillPortion(7)
            } else {
                Length::Fill
            })
            .style(subcard_style);

        let split: Element<'a, Message> = if is_desktop {
            row![explorer_card, editor_card].spacing(14).into()
        } else {
            column![explorer_card, editor_card].spacing(12).into()
        };

        container(
            column![
                text("File Manager • SD Card Storage & Staging")
                    .size(17)
                    .color(Color::WHITE),
                rule::horizontal(1),
                split,
            ]
            .spacing(14),
        )
        .padding(18)
        .width(Length::Fill)
        .style(card_style)
        .into()
    }

    fn push_dir_entries<'a>(
        &'a self,
        dev: &'a ConnectedDevice,
        dir_path: &str,
        depth: usize,
        mut col: Column<'a, Message>,
    ) -> Column<'a, Message> {
        let Some(entries) = dev.tree_data.get(dir_path) else {
            return col;
        };

        let indent = (depth * 16) as f32;

        for entry in entries {
            let name = entry.name().to_string();
            let full_path = if dir_path == "/" {
                format!("/{name}")
            } else {
                format!("{}/{}", dir_path.trim_end_matches('/'), name)
            };
            let is_selected = dev.remote_path == full_path;

            match entry.kind() {
                DirEntryType::Directory => {
                    let is_expanded = dev.expanded_dirs.contains(&full_path);
                    let arrow = if is_expanded { "▾" } else { "▸" };
                    col = col.push(
                        row![
                            space().width(indent),
                            button(text(arrow).size(11))
                                .on_press(Message::ToggleDirExpand(full_path.clone()))
                                .padding([4, 6])
                                .style(button::text),
                            button(text(format!("📁 {name}")).size(12).font(Font::MONOSPACE))
                                .on_press(Message::ToggleDirExpand(full_path.clone()))
                                .padding([4, 8])
                                .style(if is_selected {
                                    button::primary
                                } else {
                                    button::text
                                }),
                        ]
                        .align_y(Alignment::Center),
                    );

                    if is_expanded {
                        col = self.push_dir_entries(dev, &full_path, depth + 1, col);
                    }
                }
                DirEntryType::File { size } => {
                    col = col.push(
                        row![
                            space().width(indent + 18.0),
                            button(
                                text(format!("📄 {name} ({})", format_bytes(*size)))
                                    .size(12)
                                    .font(Font::MONOSPACE)
                            )
                            .on_press(Message::SelectFileEntry(full_path.clone()))
                            .width(Length::Fill)
                            .padding([5, 8])
                            .style(if is_selected {
                                button::primary
                            } else {
                                button::text
                            }),
                            button(text("Read").size(11))
                                .on_press(Message::ReadRemoteFile(full_path))
                                .padding([4, 8])
                                .style(button::secondary),
                        ]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    );
                }
            }
        }

        col
    }

    fn view_effect_panel<'a>(
        &'a self,
        dev: &'a ConnectedDevice,
        is_desktop: bool,
    ) -> Element<'a, Message> {
        let mut module_chips = Row::new().spacing(6);
        for m in &dev.wasm_modules {
            module_chips = module_chips.push(
                button(text(m).size(11).font(Font::MONOSPACE))
                    .on_press(Message::EffectPathInput(m.clone()))
                    .padding([6, 10])
                    .style(button::secondary),
            );
        }

        let cmd_card = container(
            column![
                text(format!(
                    "Command Buffer ({} B)",
                    dev.effect_command_len
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "—".into())
                ))
                .size(12)
                .color(Color::WHITE),
                text_input("01 00", &self.cmd_hex_input)
                    .on_input(Message::CommandHexInput)
                    .font(Font::MONOSPACE)
                    .padding(8)
                    .size(13),
                row![
                    button(text("Read Command").size(12))
                        .on_press(Message::ReadCommandBuf)
                        .padding([7, 12])
                        .style(button::secondary),
                    button(text("Write Command").size(12))
                        .on_press(Message::WriteCommandBuf)
                        .padding([7, 12])
                        .style(button::primary),
                ]
                .spacing(8),
            ]
            .spacing(8),
        )
        .padding(14)
        .width(Length::Fill)
        .style(subcard_style);

        let param_card = container(
            column![
                text(format!(
                    "Parameter Buffer ({} B)",
                    dev.effect_param_len
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "—".into())
                ))
                .size(12)
                .color(Color::WHITE),
                text_input("05 0c ff 64 00", &self.param_hex_input)
                    .on_input(Message::ParamHexInput)
                    .font(Font::MONOSPACE)
                    .padding(8)
                    .size(13),
                row![
                    button(text("Read Parameters").size(12))
                        .on_press(Message::ReadParamBuf)
                        .padding([7, 12])
                        .style(button::secondary),
                    button(text("Write Parameters").size(12))
                        .on_press(Message::WriteParamBuf)
                        .padding([7, 12])
                        .style(button::primary),
                ]
                .spacing(8),
            ]
            .spacing(8),
        )
        .padding(14)
        .width(Length::Fill)
        .style(subcard_style);

        let buffers_layout: Element<'a, Message> = if is_desktop {
            row![cmd_card, param_card].spacing(14).into()
        } else {
            column![cmd_card, param_card].spacing(12).into()
        };

        let mut col = column![
            row![
                column![
                    text("Effects").size(17).color(Color::WHITE),
                    text(if dev.active_effect_path.is_empty() {
                        "No active effect loaded".to_string()
                    } else {
                        format!("Active: {}", dev.active_effect_path)
                    })
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb8(96, 165, 250)),
                ]
                .spacing(2),
                space().width(Length::Fill),
                button(text("Refresh").size(12))
                    .on_press(Message::RefreshEffect)
                    .padding([7, 12])
                    .style(button::secondary),
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1),
            row![
                text_input("/effects/demo.wasm", &self.effect_path_input)
                    .on_input(Message::EffectPathInput)
                    .font(Font::MONOSPACE)
                    .padding(8)
                    .size(13)
                    .width(Length::Fill),
                button(text("Load Effect").size(12))
                    .on_press(Message::LoadEffect)
                    .padding([8, 14])
                    .style(button::primary),
                button(text("Unload").size(12))
                    .on_press(Message::UnloadEffect)
                    .padding([8, 14])
                    .style(button::secondary),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            module_chips.wrap(),
        ]
        .spacing(12);

        if let Some(manifest) = &dev.active_effect_manifest {
            let mut desc_col = Column::new().spacing(4);
            desc_col = desc_col.push(
                text(format!("Manifest: {} — ABI Bitfields", manifest.name()))
                    .size(12)
                    .color(Color::from_rgb8(148, 163, 184)),
            );
            for d in manifest.command_descriptors() {
                desc_col = desc_col.push(
                    text(format!(
                        "  [CMD]   {:<16} bits {}..={}",
                        d.name(),
                        d.start(),
                        d.end()
                    ))
                    .size(11)
                    .font(Font::MONOSPACE),
                );
            }
            for d in manifest.parameter_descriptors() {
                desc_col = desc_col.push(
                    text(format!(
                        "  [PARAM] {:<16} bits {}..={}",
                        d.name(),
                        d.start(),
                        d.end()
                    ))
                    .size(11)
                    .font(Font::MONOSPACE),
                );
            }
            col = col.push(
                container(desc_col)
                    .padding(12)
                    .width(Length::Fill)
                    .style(subcard_style),
            );
        }

        col = col.push(buffers_layout);

        container(col)
            .padding(18)
            .width(Length::Fill)
            .style(card_style)
            .into()
    }

    fn view_playlist_panel<'a>(&'a self, dev: &'a ConnectedDevice) -> Element<'a, Message> {
        let mut entries_col = Column::new().spacing(6);
        let active_pos = dev
            .active_playlist_info
            .as_ref()
            .and_then(|i| *i.position())
            .map(usize::from);

        if let Some(playlist) = &dev.active_playlist {
            for (idx, entry) in playlist.as_slice().iter().enumerate() {
                let is_active = active_pos == Some(idx);
                let marker = if is_active { "▶" } else { " " };
                let dur = match entry.duration() {
                    Some(d) => format!("{} ms", d.as_millis()),
                    None => "Indefinite".into(),
                };
                let after = match entry.after() {
                    NextPlaylistEntryAction::Shift(s) => format!("Shift({s:+})"),
                    NextPlaylistEntryAction::Random => "Random".into(),
                };
                entries_col = entries_col.push(
                    container(
                        row![
                            text(format!("{marker} #{idx}"))
                                .size(12)
                                .font(Font::MONOSPACE)
                                .color(Color::from_rgb8(96, 165, 250))
                                .width(50),
                            text(entry.path().as_str())
                                .size(12)
                                .font(Font::MONOSPACE)
                                .color(Color::WHITE)
                                .width(Length::Fill),
                            text(dur).size(12).font(Font::MONOSPACE).width(95),
                            text(after).size(12).font(Font::MONOSPACE).width(90),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .wrap(),
                    )
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(subcard_style),
                );
            }
        } else {
            entries_col = entries_col.push(
                text("No active playlist loaded.")
                    .size(12)
                    .color(Color::from_rgb8(148, 163, 184)),
            );
        }

        container(
            column![
                row![
                    column![
                        text("Playlist").size(17).color(Color::WHITE),
                        text("Load or inspect an effect playlist JSON on the device")
                            .size(12)
                            .color(Color::from_rgb8(148, 163, 184)),
                    ]
                    .spacing(2),
                    space().width(Length::Fill),
                    button(text("Refresh").size(12))
                        .on_press(Message::RefreshPlaylist)
                        .padding([7, 12])
                        .style(button::secondary),
                ]
                .align_y(Alignment::Center),
                rule::horizontal(1),
                row![
                    text_input("/playlist.json", &self.playlist_path_input)
                        .on_input(Message::PlaylistPathInput)
                        .font(Font::MONOSPACE)
                        .padding(8)
                        .size(13)
                        .width(Length::Fill),
                    button(text("Load Playlist").size(12))
                        .on_press(Message::LoadPlaylist)
                        .padding([8, 14])
                        .style(button::primary),
                    button(text("Unload").size(12))
                        .on_press(Message::UnloadPlaylist)
                        .padding([8, 14])
                        .style(button::secondary),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                entries_col,
            ]
            .spacing(12),
        )
        .padding(18)
        .width(Length::Fill)
        .style(card_style)
        .into()
    }

    fn view_activity_log(&self) -> Element<'_, Message> {
        let toggle_icon = if self.log_open { "▾" } else { "▴" };
        let last_summary = self
            .logs
            .last()
            .map(|l| format!("[{}] {}", l.timestamp, l.message))
            .unwrap_or_else(|| "No log entries.".into());

        let bar = row![
            button(
                row![
                    text(format!("{toggle_icon} Activity Log ({})", self.logs.len())).size(12),
                    text(last_summary)
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(148, 163, 184)),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            )
            .on_press(Message::ToggleLogOpen)
            .width(Length::Fill)
            .padding([6, 10])
            .style(button::text),
            button(text("Clear").size(11))
                .on_press(Message::ClearLogs)
                .padding([4, 10])
                .style(button::secondary),
        ]
        .align_y(Alignment::Center);

        if !self.log_open {
            return container(bar)
                .width(Length::Fill)
                .padding([2, 8])
                .style(header_style)
                .into();
        }

        let mut log_col = Column::new().spacing(4).width(Length::Fill);
        for entry in self.logs.iter().rev() {
            let level_color = match entry.level {
                LogLevel::Info => Color::from_rgb8(96, 165, 250),
                LogLevel::Success => Color::from_rgb8(52, 211, 153),
                LogLevel::Warn => Color::from_rgb8(251, 191, 36),
                LogLevel::Error => Color::from_rgb8(248, 113, 113),
            };
            log_col = log_col.push(
                row![
                    text(&entry.timestamp)
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(100, 116, 139)),
                    text(format!("{:<4}", entry.level.label()))
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(level_color),
                    text(&entry.message)
                        .size(11)
                        .font(Font::MONOSPACE)
                        .color(Color::from_rgb8(226, 232, 240)),
                ]
                .spacing(8),
            );
        }

        container(
            column![
                bar,
                rule::horizontal(1),
                scrollable(container(log_col).padding([6, 10])).height(140),
            ]
            .spacing(4),
        )
        .width(Length::Fill)
        .padding([4, 8])
        .style(header_style)
        .into()
    }
}

fn tree_leaf_button<'a>(
    label: impl Into<String>,
    is_selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    button(text(label.into()).size(12).color(if is_selected {
        Color::WHITE
    } else {
        Color::from_rgb8(180, 190, 206)
    }))
    .on_press(on_press)
    .width(Length::Fill)
    .padding([7, 10])
    .style(if is_selected {
        button::primary
    } else {
        button::text
    })
    .into()
}

fn metric_tile<'a>(label: &'static str, value: String) -> Element<'a, Message> {
    container(
        column![
            text(label).size(10).color(Color::from_rgb8(148, 163, 184)),
            text(value)
                .size(13)
                .font(Font::MONOSPACE)
                .color(Color::WHITE),
        ]
        .spacing(4),
    )
    .padding([10, 14])
    .width(175)
    .style(subcard_style)
    .into()
}

fn app_bg_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(11, 15, 25).into()),
        text_color: Some(Color::from_rgb8(226, 232, 240)),
        ..container::Style::default()
    }
}

fn header_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(17, 24, 39).into()),
        border: Border {
            color: Color::from_rgb8(31, 41, 55),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn sidebar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(15, 23, 42).into()),
        border: Border {
            color: Color::from_rgb8(30, 41, 59),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn drawer_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(15, 23, 42).into()),
        border: Border {
            color: Color::from_rgb8(51, 65, 85),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(17, 24, 39).into()),
        border: border::rounded(12)
            .width(1)
            .color(Color::from_rgb8(31, 41, 55)),
        ..container::Style::default()
    }
}

fn subcard_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(13, 19, 33).into()),
        border: border::rounded(10)
            .width(1)
            .color(Color::from_rgb8(30, 41, 59)),
        ..container::Style::default()
    }
}

fn badge_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(23, 37, 84).into()),
        border: border::rounded(6)
            .width(1)
            .color(Color::from_rgb8(37, 99, 235)),
        ..container::Style::default()
    }
}

fn modal_card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(17, 24, 39).into()),
        border: border::rounded(12)
            .width(1)
            .color(Color::from_rgb8(71, 85, 105)),
        ..container::Style::default()
    }
}

fn backdrop_container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.65).into()),
        ..container::Style::default()
    }
}

fn backdrop_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
        border: Border::default(),
        ..button::Style::default()
    }
}
