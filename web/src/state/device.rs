use super::config::{NormalizedConfig, normalize_config};
use crate::transport::RpcClient;
use bled_core::{
    ChannelNumber, PathString, RGB8,
    api::{ActivePlaylistInfo, BootReason, DeviceInfo, DirEntry, DirEntryType},
    config::{
        BrightnessLimit, BrightnessLimitMode, CircularArray, Fixture, Fixtures, LinearArray,
        Matrix2d, PixelLayout, PixelSpan,
    },
    effect::{EffectManifest, EffectPlaylist},
};
use futures::lock::Mutex;
use send_wrapper::SendWrapper;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "OK",
            Self::Warn => "WARN",
            Self::Error => "ERR",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub level: LogLevel,
}

pub fn now_timestamp() -> String {
    use chrono::Timelike;
    let millis = js_sys::Date::now() as i64;
    if let Some(dt) = chrono::DateTime::from_timestamp_millis(millis) {
        format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second())
    } else {
        "00:00:00".to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeSelection {
    Device {
        device_id: String,
    },
    SdCard {
        device_id: String,
    },
    Effect {
        device_id: String,
    },
    Playlist {
        device_id: String,
    },
    Fixture {
        device_id: String,
        fixture_idx: usize,
    },
    Span {
        device_id: String,
        fixture_idx: usize,
        span_idx: usize,
    },
}

impl TreeSelection {
    pub fn device_id(&self) -> &str {
        match self {
            Self::Device { device_id }
            | Self::SdCard { device_id }
            | Self::Effect { device_id }
            | Self::Playlist { device_id }
            | Self::Fixture { device_id, .. }
            | Self::Span { device_id, .. } => device_id,
        }
    }

    pub fn section_label(&self) -> String {
        match self {
            Self::Device { .. } => "Device Info".into(),
            Self::SdCard { .. } => "SD Card".into(),
            Self::Effect { .. } => "Effects".into(),
            Self::Playlist { .. } => "Playlist".into(),
            Self::Fixture { fixture_idx, .. } => format!("Fixture #{fixture_idx}"),
            Self::Span {
                fixture_idx,
                span_idx,
                ..
            } => format!("Fixture #{fixture_idx} / Span #{span_idx}"),
        }
    }
}

#[derive(Clone)]
pub struct ConnectedDevice {
    pub id: String,
    pub name: String,
    pub is_ble: bool,
    pub is_demo: bool,
    pub rpc: Option<SendWrapper<Rc<Mutex<RpcClient>>>>,
    pub config: Option<NormalizedConfig>,
    pub config_error: Option<String>,
    pub device_info: Option<DeviceInfo>,

    // SD Card / File Staging state
    pub tree_data: HashMap<String, Vec<DirEntry>>,
    pub mock_files: HashMap<String, Vec<u8>>,
    pub expanded_dirs: HashSet<String>,
    pub remote_path: String,
    pub staged_filename: String,
    pub staged_data: Vec<u8>,
    pub staged_is_hex: bool,
    pub transfer_progress: Option<u8>,

    // Effect state
    pub active_effect_path: String,
    pub active_effect_manifest: Option<EffectManifest>,
    pub effect_command_bytes: Vec<u8>,
    pub effect_param_bytes: Vec<u8>,
    pub effect_command_len: Option<u64>,
    pub effect_param_len: Option<u64>,
    pub wasm_modules: Vec<String>,

    // Playlist state
    pub active_playlist_info: Option<ActivePlaylistInfo>,
    pub active_playlist: Option<EffectPlaylist>,
    pub json_files: Vec<String>,
}

impl std::fmt::Debug for ConnectedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectedDevice")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("is_ble", &self.is_ble)
            .field("is_demo", &self.is_demo)
            .finish()
    }
}

impl ConnectedDevice {
    pub fn new(rpc: RpcClient) -> Self {
        let id = rpc.id();
        let name = rpc.name();
        let is_ble = rpc.is_ble();
        let mut expanded_dirs = HashSet::new();
        expanded_dirs.insert("/".to_string());

        Self {
            id,
            name,
            is_ble,
            is_demo: false,
            rpc: Some(SendWrapper::new(Rc::new(Mutex::new(rpc)))),
            config: None,
            config_error: None,
            device_info: None,
            tree_data: HashMap::new(),
            mock_files: HashMap::new(),
            expanded_dirs,
            remote_path: "/fixtures.json".to_string(),
            staged_filename: String::new(),
            staged_data: Vec::new(),
            staged_is_hex: false,
            transfer_progress: None,
            active_effect_path: String::new(),
            active_effect_manifest: None,
            effect_command_bytes: Vec::new(),
            effect_param_bytes: Vec::new(),
            effect_command_len: None,
            effect_param_len: None,
            wasm_modules: Vec::new(),
            active_playlist_info: None,
            active_playlist: None,
            json_files: Vec::new(),
        }
    }

    pub fn new_demo(index: usize) -> Self {
        let id = format!("demo-{index}");
        let name = if index == 1 {
            "bled-desk-matrix".to_string()
        } else {
            format!("bled-demo-{index}")
        };

        let dev_name = core::str::FromStr::from_str(&name).unwrap_or_default();
        let rev = heapless::String::try_from("86723d1").unwrap_or_default();
        let info = DeviceInfo::new(dev_name, rev, BootReason::Normal, 3_742_500, 16 * 1024, 4);

        let limit_rescale =
            BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(180, 180, 180));
        let limit_clamp =
            BrightnessLimit::new(BrightnessLimitMode::Clamp, RGB8::new(200, 200, 200));

        let fixture1 = Fixture::new(
            "desk-strip".try_into().unwrap(),
            PixelLayout::LinearArray(LinearArray::default()),
            [
                PixelSpan::new(
                    "left-edge".try_into().unwrap(),
                    ChannelNumber::new(0),
                    0,
                    30,
                    limit_rescale,
                ),
                PixelSpan::new(
                    "right-edge".try_into().unwrap(),
                    ChannelNumber::new(0),
                    30,
                    30,
                    limit_rescale,
                ),
            ]
            .into(),
        );

        let fixture2 = Fixture::new(
            "ring-lamp".try_into().unwrap(),
            PixelLayout::CircularArray(CircularArray::default()),
            [PixelSpan::new(
                "ring-24".try_into().unwrap(),
                ChannelNumber::new(1),
                0,
                24,
                limit_clamp,
            )]
            .into(),
        );

        let fixture3 = Fixture::new(
            "wall-matrix".try_into().unwrap(),
            PixelLayout::Matrix2d(Matrix2d::default()),
            [PixelSpan::new(
                "panel-16x16".try_into().unwrap(),
                ChannelNumber::new(2),
                0,
                256,
                BrightnessLimit::default(),
            )]
            .into(),
        );

        let fixtures = Fixtures::new([fixture1, fixture2, fixture3].into());
        let config = normalize_config(&fixtures, &name);

        let mut tree_data = HashMap::new();
        tree_data.insert(
            "/".to_string(),
            vec![
                DirEntry::new("effects".try_into().unwrap(), DirEntryType::Directory),
                DirEntry::new(
                    "fixtures.json".try_into().unwrap(),
                    DirEntryType::File { size: 612 },
                ),
                DirEntry::new(
                    "playlist.json".try_into().unwrap(),
                    DirEntryType::File { size: 284 },
                ),
                DirEntry::new(
                    "ble_bonds.bin".try_into().unwrap(),
                    DirEntryType::File { size: 64 },
                ),
            ],
        );
        tree_data.insert(
            "/effects".to_string(),
            vec![
                DirEntry::new(
                    "matrix-game-of-life.wasm".try_into().unwrap(),
                    DirEntryType::File { size: 5120 },
                ),
                DirEntry::new(
                    "span-colour-chase.wasm".try_into().unwrap(),
                    DirEntryType::File { size: 4823 },
                ),
                DirEntry::new(
                    "span-painter.wasm".try_into().unwrap(),
                    DirEntryType::File { size: 4819 },
                ),
            ],
        );

        let sample_fixtures_json = r#"[
  {
    "name": "desk-strip",
    "layout": { "linear_array": { "length": 60 } },
    "spans": [
      { "name": "left-edge", "channel": 0, "start": 0, "length": 30 },
      { "name": "right-edge", "channel": 0, "start": 30, "length": 30 }
    ]
  },
  {
    "name": "ring-lamp",
    "layout": { "circular_array": { "radius": 12 } },
    "spans": [
      { "name": "ring-24", "channel": 1, "start": 0, "length": 24 }
    ]
  }
]"#;

        let sample_playlist_json = r#"[
  {
    "path": "/effects/span-colour-chase.wasm",
    "duration": 15000,
    "after": { "shift": 1 }
  },
  {
    "path": "/effects/matrix-game-of-life.wasm",
    "duration": 30000,
    "after": "random"
  },
  {
    "path": "/effects/span-painter.wasm",
    "duration": null,
    "after": { "shift": -1 }
  }
]"#;

        let mut mock_files = HashMap::new();
        mock_files.insert(
            "/fixtures.json".to_string(),
            sample_fixtures_json.as_bytes().to_vec(),
        );
        mock_files.insert(
            "/playlist.json".to_string(),
            sample_playlist_json.as_bytes().to_vec(),
        );
        mock_files.insert(
            "/ble_bonds.bin".to_string(),
            (0..64u8).map(|i| i.wrapping_mul(37) ^ 0xA5).collect(),
        );
        mock_files.insert(
            "/effects/span-colour-chase.wasm".to_string(),
            b"\x00asm\x01\x00\x00\x00\x01\x0c\x02\x60\x02\x7f\x7f\x00\x60\x00\x00\x03\x03\x02\x00\x01span-colour-chase".to_vec(),
        );

        let manifest: Option<EffectManifest> = bled_core::io::decode_json(
            br#"{
                "name": "span-colour-chase",
                "command_descriptors": [
                    { "name": "reset_phase", "start": 0, "end": 0 },
                    { "name": "reverse_dir", "start": 1, "end": 1 }
                ],
                "parameter_descriptors": [
                    { "name": "speed_hz", "start": 0, "end": 7 },
                    { "name": "tail_length", "start": 8, "end": 15 },
                    { "name": "primary_rgb", "start": 16, "end": 39 }
                ]
            }"#,
        )
        .ok();

        let playlist: Option<EffectPlaylist> =
            bled_core::io::decode_json(sample_playlist_json.as_bytes()).ok();

        let mut expanded_dirs = HashSet::new();
        expanded_dirs.insert("/".to_string());
        expanded_dirs.insert("/effects".to_string());

        Self {
            id,
            name,
            is_ble: true,
            is_demo: true,
            rpc: None,
            config: Some(config),
            config_error: None,
            device_info: Some(info),
            tree_data,
            mock_files,
            expanded_dirs,
            remote_path: "/fixtures.json".to_string(),
            staged_filename: "fixtures.json".to_string(),
            staged_data: sample_fixtures_json.as_bytes().to_vec(),
            staged_is_hex: false,
            transfer_progress: None,
            active_effect_path: "/effects/span-colour-chase.wasm".to_string(),
            active_effect_manifest: manifest,
            effect_command_bytes: vec![0x01, 0x00],
            effect_param_bytes: vec![0x05, 0x0c, 0xff, 0x64, 0x00],
            effect_command_len: Some(2),
            effect_param_len: Some(5),
            wasm_modules: vec![
                "/effects/matrix-game-of-life.wasm".to_string(),
                "/effects/span-colour-chase.wasm".to_string(),
                "/effects/span-painter.wasm".to_string(),
            ],
            active_playlist_info: Some(ActivePlaylistInfo::new(
                PathString::try_from("/playlist.json").unwrap(),
                Some(0),
            )),
            active_playlist: playlist,
            json_files: vec!["/fixtures.json".to_string(), "/playlist.json".to_string()],
        }
    }

    pub fn display_name(&self) -> String {
        if let Some(info) = &self.device_info {
            let n = info.name().as_str();
            if !n.is_empty() {
                return n.to_string();
            }
        }
        if let Some(cfg) = &self.config
            && !cfg.name.is_empty()
        {
            return cfg.name.clone();
        }
        self.name.clone()
    }
}
