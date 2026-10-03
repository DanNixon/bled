//! Talking to a bled device over BLE (GATT).

use anyhow::{Context, Result, anyhow, bail};
use bled_api::{
    ChannelMask, ChannelRanges, ConfigControl, ConfigStatResponse, DeviceInfo, FileIoControl,
    FileStatResponse, LedBufferControl, LedBufferInformation, MAX_PATH_LEN, RGB8, Rgb8Ranges,
    ble::{
        COMMIT_UUID, CONFIG_CONTROL_UUID, CONFIG_DATA_UUID, DEVICE_INFO_UUID, FILE_IO_CONTROL_UUID,
        FILE_IO_DATA_UUID, LED_BUFFER_CONTROL_UUID, LED_BUFFER_DATA_UUID, LED_RANGE_UUID,
        MAX_ATTRIBUTE_VALUE_LEN, RESET_UUID, SERVICE_UUID,
    },
};
use btleplug::{
    api::{
        BDAddr, Central, CentralEvent, CentralState, CharPropFlags, Characteristic, Manager as _,
        Peripheral as _, ScanFilter, WriteType,
    },
    platform::{Adapter, Peripheral},
};
use futures::StreamExt as _;
use std::{
    collections::{BTreeSet, HashSet},
    time::{Duration, Instant},
};
use uuid::Uuid;

/// How long to wait for a connection attempt.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Picks the first powered-on Bluetooth adapter.
pub async fn pick_adapter(manager: &btleplug::platform::Manager) -> Result<Adapter> {
    let adapters = manager.adapters().await.context("listing adapters")?;
    for adapter in &adapters {
        if adapter.adapter_state().await? == CentralState::PoweredOn {
            log::info!("using adapter {}", adapter.adapter_info().await?);
            return Ok(adapter.clone());
        }
    }
    bail!("no enabled Bluetooth adapter found")
}

/// Scans for `scan_time`, collecting peripherals advertising the bled service.
pub async fn scan(adapter: &Adapter, scan_time: Duration) -> Result<Vec<Peripheral>> {
    let filter = ScanFilter {
        services: vec![SERVICE_UUID],
    };
    let mut events = adapter.events().await?;
    adapter.start_scan(filter).await.context("starting scan")?;
    let mut seen = HashSet::new();
    let deadline = Instant::now() + scan_time;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, events.next()).await {
            Ok(Some(CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id))) => {
                seen.insert(id);
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }
    adapter.stop_scan().await.context("stopping scan")?;

    let mut peripherals = Vec::new();
    for id in seen {
        match adapter.peripheral(&id).await {
            Ok(peripheral) => peripherals.push(peripheral),
            Err(error) => log::debug!("dropping peripheral {id:?}: {error}"),
        }
    }
    Ok(peripherals)
}

/// A discovered device together with the properties read from its advertising
/// report.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub peripheral: Peripheral,
    pub name: Option<String>,
    pub address: Option<BDAddr>,
}

impl Candidate {
    /// A human-readable label for logging.
    pub fn label(&self) -> String {
        format_label(self.name.as_deref(), self.address)
    }

    /// The identifying fields of this device, without the peripheral handle.
    pub fn identity(&self) -> DeviceIdentity<'_> {
        DeviceIdentity {
            name: self.name.as_deref(),
            address: self.address,
        }
    }
}

/// The advertised identity of a device, borrowed from a [`Candidate`].
///
/// Lets device selection and completion be written against plain data rather
/// than a live [`Peripheral`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceIdentity<'a> {
    pub name: Option<&'a str>,
    pub address: Option<BDAddr>,
}

impl DeviceIdentity<'_> {
    /// A human-readable label for logging.
    pub fn label(&self) -> String {
        format_label(self.name, self.address)
    }
}

/// Formats a device label from whichever of its name and address are known.
pub fn format_label(name: Option<&str>, address: Option<BDAddr>) -> String {
    match (name, address) {
        (Some(name), Some(address)) => format!("{name} ({address})"),
        (Some(name), None) => name.to_owned(),
        (None, Some(address)) => address.to_string(),
        (None, None) => "<unknown>".into(),
    }
}

/// Reads advertising properties for each device and applies the optional name
/// and address filters. Results are sorted by address and de-duplicated.
pub async fn filter_candidates(
    devices: impl IntoIterator<Item = Peripheral>,
    name_filter: Option<&str>,
    address_filter: Option<BDAddr>,
) -> Result<Vec<Candidate>> {
    let name_filter = name_filter.map(str::to_lowercase);
    let mut candidates = Vec::new();
    for peripheral in devices {
        let properties = match peripheral.properties().await {
            Ok(Some(properties)) => properties,
            Ok(None) => {
                if name_filter.is_some() || address_filter.is_some() {
                    continue;
                }
                candidates.push(Candidate {
                    peripheral,
                    name: None,
                    address: None,
                });
                continue;
            }
            Err(error) => {
                log::debug!("could not read properties, dropping device: {error}");
                continue;
            }
        };
        let name = properties
            .local_name
            .clone()
            .or(properties.advertisement_name.clone());
        if let Some(wanted) = &name_filter {
            match &name {
                Some(have) if have.to_lowercase().contains(wanted) => {}
                _ => continue,
            }
        }
        if let Some(wanted) = address_filter
            && properties.address != wanted
        {
            continue;
        }
        candidates.push(Candidate {
            peripheral,
            name,
            address: Some(properties.address),
        });
    }
    candidates.sort_by_key(|candidate| candidate.address.map(BDAddr::into_inner));
    candidates.dedup_by(|a, b| match (a.address, b.address) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    });
    Ok(candidates)
}

/// Connects to a device and discovers its GATT services.
pub async fn connect(peripheral: &Peripheral) -> Result<()> {
    if !peripheral.is_connected().await? {
        peripheral
            .connect_with_timeout(CONNECT_TIMEOUT)
            .await
            .context("connecting")?;
    }
    peripheral
        .discover_services_with_timeout(CONNECT_TIMEOUT)
        .await
        .context("discovering services")?;
    Ok(())
}

/// The characteristics that make up a device's bled LED service.
#[derive(Debug, Clone)]
pub struct BledService {
    /// Device Info (`READ`): firmware and runtime information.
    pub device_info: Characteristic,
    /// Reset (`WRITE`): reboots the device.
    pub reset: Characteristic,

    /// Config Control (`WRITE`): active config commands.
    pub config_control: Characteristic,
    /// Config Data (`READ`): active config data transfers.
    pub config_data: Characteristic,

    /// File IO Control (`WRITE`): filesystem commands.
    pub file_io_control: Characteristic,
    /// File IO Data (`READ`, `WRITE`): file data transfers.
    pub file_io_data: Characteristic,

    /// Commit (`WRITE`): renders the staged frame.
    pub commit: Characteristic,
    /// LED Buffer Control (`WRITE`): raw LED buffer commands.
    pub led_buffer_control: Characteristic,
    /// LED Buffer Data (`READ`, `WRITE`): raw LED buffer data transfers.
    pub led_buffer_data: Characteristic,
    /// Range Data (`WRITE`): staged solid-colour ranges.
    pub range_data: Characteristic,
}

/// Locates the bled characteristics by UUID within a discovered set.
pub fn find_service(characteristics: &BTreeSet<Characteristic>) -> Result<BledService> {
    let find = |uuid: Uuid, required: CharPropFlags| -> Result<Characteristic> {
        let characteristic = characteristics
            .iter()
            .find(|characteristic| {
                characteristic.service_uuid == SERVICE_UUID && characteristic.uuid == uuid
            })
            .cloned()
            .with_context(|| format!("device does not expose v2 characteristic {uuid}"))?;
        if !characteristic.properties.contains(required) {
            bail!(
                "characteristic {uuid} has properties {:?}, expected {:?}",
                characteristic.properties,
                required
            );
        }
        Ok(characteristic)
    };

    let device_info = find(DEVICE_INFO_UUID, CharPropFlags::READ)?;
    let reset = find(RESET_UUID, CharPropFlags::WRITE)?;

    let config_control = find(CONFIG_CONTROL_UUID, CharPropFlags::WRITE)?;
    let config_data = find(CONFIG_DATA_UUID, CharPropFlags::READ)?;

    let file_io_control = find(FILE_IO_CONTROL_UUID, CharPropFlags::WRITE)?;
    let file_io_data = find(
        FILE_IO_DATA_UUID,
        CharPropFlags::READ | CharPropFlags::WRITE,
    )?;

    let commit = find(COMMIT_UUID, CharPropFlags::WRITE)?;
    let led_buffer_control = find(LED_BUFFER_CONTROL_UUID, CharPropFlags::WRITE)?;
    let led_buffer_data = find(
        LED_BUFFER_DATA_UUID,
        CharPropFlags::READ | CharPropFlags::WRITE,
    )?;
    let range_data = find(LED_RANGE_UUID, CharPropFlags::WRITE)?;

    Ok(BledService {
        device_info,
        reset,
        config_control,
        config_data,
        file_io_control,
        file_io_data,
        commit,
        led_buffer_control,
        led_buffer_data,
        range_data,
    })
}

/// Reads and decodes the Device Info characteristic.
pub async fn read_device_info(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
) -> Result<DeviceInfo> {
    let data = peripheral
        .read(characteristic)
        .await
        .context("reading Device Info")?;
    bled_api::io::decode_cbor(&data)
        .map_err(|error| anyhow!("device returned invalid Device Info: {error}"))
}

/// Requests a device reset.
pub async fn reset(peripheral: &Peripheral, characteristic: &Characteristic) -> Result<()> {
    peripheral
        .write(characteristic, &[], WriteType::WithResponse)
        .await
        .context("writing reset request")
}

/// Sends a generic control command CBOR-encoded to the given characteristic.
async fn send_control<C: serde::Serialize>(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    name: &str,
    command: &C,
) -> Result<()> {
    let mut buffer = [0; MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_api::io::encode_cbor(command, &mut buffer)
        .map_err(|error| anyhow!("encoding {name} control command: {error}"))?;
    peripheral
        .write(characteristic, &buffer[..written], WriteType::WithResponse)
        .await
        .with_context(|| format!("writing {name} control command"))
}

/// Reads a stream of data chunks from a peripheral characteristic until EOF or expected size.
async fn read_chunked<C, F>(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
    name: &str,
    expected_size: Option<usize>,
    reset_command: C,
    mut make_read_command: F,
) -> Result<Vec<u8>>
where
    C: serde::Serialize,
    F: FnMut(u32) -> Result<C>,
{
    let mut contents = match expected_size {
        Some(size) => Vec::with_capacity(size),
        None => Vec::new(),
    };

    while expected_size.is_none_or(|size| contents.len() < size) {
        let offset = u32::try_from(contents.len())?;
        let cmd = make_read_command(offset)?;

        if let Err(error) = send_control(peripheral, control, name, &cmd).await {
            let _ = send_control(peripheral, control, name, &reset_command).await;
            return Err(error).with_context(|| {
                format!("preparing to read chunk from {name} at offset {offset}")
            });
        }

        let chunk = match peripheral.read(data).await {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = send_control(peripheral, control, name, &reset_command).await;
                return Err(error).with_context(|| format!("reading chunk from {name}"));
            }
        };

        if chunk.is_empty() {
            break;
        }
        contents.extend_from_slice(&chunk);
    }

    if let Err(error) = send_control(peripheral, control, name, &reset_command).await {
        log::debug!("could not reset {name} state machine after reading: {error}");
    }

    Ok(contents)
}

/// Writes data to a peripheral characteristic in chunks bounded by MTU.
async fn write_chunked<C, F>(
    peripheral: &Peripheral,
    (control, data_char): (&Characteristic, &Characteristic),
    name: &str,
    data: &[u8],
    start_offset: usize,
    reset_command: C,
    mut make_write_command: F,
) -> Result<()>
where
    C: serde::Serialize,
    F: FnMut(u32) -> Result<C>,
{
    let mtu = peripheral.mtu();
    let chunk_size = (usize::from(mtu) - 3).min(MAX_ATTRIBUTE_VALUE_LEN);

    let mut offset = start_offset;
    for chunk in data.chunks(chunk_size) {
        let current_offset = u32::try_from(offset)?;
        let cmd = make_write_command(current_offset)?;

        if let Err(error) = send_control(peripheral, control, name, &cmd).await {
            let _ = send_control(peripheral, control, name, &reset_command).await;
            return Err(error).with_context(|| {
                format!("preparing to write chunk to {name} at offset {current_offset}")
            });
        }

        if let Err(error) = peripheral
            .write(data_char, chunk, WriteType::WithResponse)
            .await
        {
            let _ = send_control(peripheral, control, name, &reset_command).await;
            return Err(error).with_context(|| format!("writing chunk to {name}"));
        }

        offset += chunk.len();
    }

    send_control(peripheral, control, name, &reset_command)
        .await
        .with_context(|| format!("completing write for {name}"))?;

    Ok(())
}

/// Sends a control command to the Config Control characteristic.
pub async fn send_config_control(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    command: &ConfigControl,
) -> Result<()> {
    send_control(peripheral, characteristic, "Config", command).await
}

/// Stats active device configuration.
pub async fn stat_config(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
) -> Result<ConfigStatResponse> {
    send_config_control(peripheral, control, &ConfigControl::Stat)
        .await
        .context("preparing to stat active config")?;

    let bytes = match peripheral.read(data).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = send_config_control(peripheral, control, &ConfigControl::Reset).await;
            return Err(error).context("reading config stat data");
        }
    };

    let _ = send_config_control(peripheral, control, &ConfigControl::Reset).await;

    bled_api::io::decode_cbor(&bytes)
        .map_err(|error| anyhow!("decoding config stat response: {error}"))
}

/// Reads the active device configuration from the connected device.
pub async fn read_config(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
) -> Result<Vec<u8>> {
    let stat = stat_config(peripheral, control, data).await?;
    let total_size = usize::try_from(*stat.size())?;
    read_chunked(
        peripheral,
        control,
        data,
        "config",
        Some(total_size),
        ConfigControl::Reset,
        |offset| Ok(ConfigControl::Read { offset }),
    )
    .await
}

/// Stages one solid-colour pixel range without rendering it.
pub async fn send_range(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    channel: u8,
    start: u16,
    count: u16,
    colour: RGB8,
) -> Result<()> {
    let end = start
        .checked_add(count)
        .ok_or_else(|| anyhow!("pixel range extends beyond u16 address space"))?;
    let mut ranges = Rgb8Ranges::<24>::default();
    bled_api::PixelRanges::insert(&mut ranges, start..end, colour)
        .map_err(|error| anyhow!("creating pixel range update: {error}"))?;
    let ranges = ChannelRanges::<RGB8, Rgb8Ranges<24>>::new(channel, ranges)
        .map_err(|error| anyhow!("creating channel range update: {error}"))?;
    let mut buffer = [0; 128];
    let written = bled_api::io::encode_cbor(&ranges, &mut buffer)
        .map_err(|error| anyhow!("encoding pixel range: {error}"))?;
    peripheral
        .write(characteristic, &buffer[..written], WriteType::WithResponse)
        .await
        .context("writing pixel range")
}

/// Renders the staged data for one or more channels in a single write.
pub async fn commit_channels(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    channels: &[u8],
) -> Result<()> {
    if channels.is_empty() {
        return Err(anyhow!("at least one channel index is required"));
    }
    let mask = ChannelMask::from_indices(channels.iter().copied().map(usize::from))
        .ok_or_else(|| anyhow!("channel index out of range"))?;
    let mut buffer = [0; 16];
    let written = bled_api::io::encode_cbor(&mask, &mut buffer)
        .map_err(|error| anyhow!("encoding Commit command: {error}"))?;
    peripheral
        .write(characteristic, &buffer[..written], WriteType::WithResponse)
        .await
        .with_context(|| format!("committing channels {channels:?}"))
}

/// Sends a control command to the File IO Control characteristic.
pub async fn send_file_io_control(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    command: &FileIoControl,
) -> Result<()> {
    send_control(peripheral, characteristic, "File IO", command).await
}

/// Stats a file on the connected device.
pub async fn stat_file(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
    path: &str,
) -> Result<FileStatResponse> {
    let heapless_path = path
        .try_into()
        .map_err(|_| anyhow!("path exceeds maximum length of {MAX_PATH_LEN} bytes"))?;

    send_file_io_control(
        peripheral,
        control,
        &FileIoControl::Stat {
            path: heapless_path,
        },
    )
    .await
    .with_context(|| format!("preparing stat for {path}"))?;

    let bytes = match peripheral.read(data).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = send_file_io_control(peripheral, control, &FileIoControl::Reset).await;
            return Err(error).with_context(|| format!("reading stat data for {path}"));
        }
    };

    let _ = send_file_io_control(peripheral, control, &FileIoControl::Reset).await;

    bled_api::io::decode_cbor(&bytes)
        .map_err(|error| anyhow!("decoding file stat response for {path}: {error}"))
}

/// Reads the entire contents of a file from the connected device.
pub async fn read_file(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
    path: &str,
) -> Result<Vec<u8>> {
    let heapless_path: heapless::String<MAX_PATH_LEN> = path
        .try_into()
        .map_err(|_| anyhow!("path exceeds maximum length of {MAX_PATH_LEN} bytes"))?;

    read_chunked(
        peripheral,
        control,
        data,
        path,
        None,
        FileIoControl::Reset,
        |offset| {
            Ok(FileIoControl::Read {
                path: heapless_path.clone(),
                offset,
            })
        },
    )
    .await
}

/// Writes data to a file on the connected device.
pub async fn write_file(
    peripheral: &Peripheral,
    control: &Characteristic,
    data_char: &Characteristic,
    path: &str,
    data: &[u8],
) -> Result<()> {
    let heapless_path: heapless::String<MAX_PATH_LEN> = path
        .try_into()
        .map_err(|_| anyhow!("path exceeds maximum length of {MAX_PATH_LEN} bytes"))?;

    send_file_io_control(
        peripheral,
        control,
        &FileIoControl::Create {
            path: heapless_path.clone(),
            size: u32::try_from(data.len())?,
        },
    )
    .await
    .with_context(|| format!("creating file {path} with size {}", data.len()))?;

    write_chunked(
        peripheral,
        (control, data_char),
        path,
        data,
        0,
        FileIoControl::Reset,
        |offset| {
            Ok(FileIoControl::Write {
                path: heapless_path.clone(),
                offset,
            })
        },
    )
    .await
}

/// Deletes a file on the connected device.
pub async fn delete_file(
    peripheral: &Peripheral,
    control: &Characteristic,
    path: &str,
) -> Result<()> {
    let heapless_path = path
        .try_into()
        .map_err(|_| anyhow!("path exceeds maximum length of {MAX_PATH_LEN} bytes"))?;
    send_file_io_control(
        peripheral,
        control,
        &FileIoControl::Delete {
            path: heapless_path,
        },
    )
    .await
    .with_context(|| format!("deleting {path}"))
}

/// Sends a control command to the LED Buffer Control characteristic.
pub async fn send_led_buffer_control(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    command: &LedBufferControl,
) -> Result<()> {
    send_control(peripheral, characteristic, "LED buffer", command).await
}

/// Reads the LED buffer information from the connected device.
pub async fn read_led_buffer_info(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
) -> Result<LedBufferInformation> {
    send_led_buffer_control(peripheral, control, &LedBufferControl::Info)
        .await
        .context("preparing to read LED buffer info")?;

    let bytes = match peripheral.read(data).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = send_led_buffer_control(peripheral, control, &LedBufferControl::Reset).await;
            return Err(error).context("reading LED buffer info data");
        }
    };

    let _ = send_led_buffer_control(peripheral, control, &LedBufferControl::Reset).await;

    bled_api::io::decode_cbor(&bytes)
        .map_err(|error| anyhow!("decoding LED buffer information: {error}"))
}

/// Reads the entire contents of the LED buffer from the connected device.
pub async fn read_led_buffer(
    peripheral: &Peripheral,
    control: &Characteristic,
    data: &Characteristic,
) -> Result<Vec<u8>> {
    let info = read_led_buffer_info(peripheral, control, data).await?;
    let total_size = usize::try_from(*info.size())?;
    read_chunked(
        peripheral,
        control,
        data,
        "LED buffer",
        Some(total_size),
        LedBufferControl::Reset,
        |offset| Ok(LedBufferControl::Read { offset }),
    )
    .await
}

/// Writes data to the LED buffer on the connected device.
pub async fn write_led_buffer(
    peripheral: &Peripheral,
    control: &Characteristic,
    data_char: &Characteristic,
    data: &[u8],
    offset: usize,
) -> Result<()> {
    write_chunked(
        peripheral,
        (control, data_char),
        "LED buffer",
        data,
        offset,
        LedBufferControl::Reset,
        |offset| Ok(LedBufferControl::Write { offset }),
    )
    .await
}
