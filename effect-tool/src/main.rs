use anyhow::{Context, Result, anyhow, bail};
use bled_core::{
    PathString,
    api::{
        Chunk, MAX_CHUNK_SIZE, RPC_ACK_TIMEOUT, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN,
        RPC_LOAD_TIMEOUT, RPC_RESPONSE_TIMEOUT, Request, Response,
    },
    config::{CONFIG_PATH, DeviceConfig, MAX_CONFIG_JSON_SIZE},
    effect::{EffectPreset, MAX_EFFECT_PRESET_JSON_SIZE},
    usb::{USB_PID, USB_VID},
};
use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use teeny_rpc::{Client, transport::SerialTransport};
use tokio::time::sleep;
use tokio_serial::{SerialPortInfo, SerialPortType};

const BAUD_RATE: u32 = 115_200;
const REBOOT_WAIT: Duration = Duration::from_millis(750);
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const RECONNECT_POLL_INTERVAL: Duration = Duration::from_millis(250);

const REMOTE_WASM_PATH: &str = "/test.wasm";
const REMOTE_PRESET_PATH: &str = "/test.json";

type RpcClient = Client<SerialTransport, Request, Response, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN>;

#[derive(Parser, Debug)]
#[command(about = "Rapidly test an effect or preset on a physical BLED controller")]
struct Args {
    /// Serial port path (e.g. /dev/ttyACM0) or USB serial number of the device
    #[arg(short, long)]
    device: Option<String>,

    /// Path to the device configuration JSON file
    config: PathBuf,

    /// Path to the WebAssembly effect module (.wasm) or preset (.json) file
    effect: PathBuf,
}

struct StagedPayload {
    wasm_bytes: Vec<u8>,
    preset_bytes: Option<Vec<u8>>,
    active_path: PathString,
}

#[derive(Debug, Clone)]
struct TargetDevice {
    port_name: String,
    serial_number: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config_bytes = load_and_validate_config(&args.config)?;
    let staged = load_and_prepare_effect(&args.effect)?;

    let target = resolve_device(args.device.as_deref())?;
    println!(
        "Connecting to {} (serial: {})...",
        target.port_name,
        target.serial_number.as_deref().unwrap_or("unknown")
    );

    let mut client = connect(&target.port_name).await?;

    let config_path: PathString = CONFIG_PATH
        .try_into()
        .map_err(|_| anyhow!("invalid config path"))?;
    println!(
        "Uploading configuration to {CONFIG_PATH} ({} bytes)...",
        config_bytes.len()
    );
    upload_file(&mut client, &config_path, &config_bytes).await?;

    println!("Rebooting device...");
    call_ok(&mut client, Request::Reboot, RPC_RESPONSE_TIMEOUT).await?;
    drop(client);

    sleep(REBOOT_WAIT).await;

    println!("Waiting for device to reconnect...");
    let mut client = reconnect(&target, args.device.as_deref()).await?;

    let wasm_path: PathString = REMOTE_WASM_PATH
        .try_into()
        .map_err(|_| anyhow!("invalid remote WASM path"))?;
    println!(
        "Uploading WASM module to {REMOTE_WASM_PATH} ({} bytes)...",
        staged.wasm_bytes.len()
    );
    upload_file(&mut client, &wasm_path, &staged.wasm_bytes).await?;

    if let Some(preset_bytes) = &staged.preset_bytes {
        let preset_path: PathString = REMOTE_PRESET_PATH
            .try_into()
            .map_err(|_| anyhow!("invalid remote preset path"))?;
        println!(
            "Uploading preset to {REMOTE_PRESET_PATH} ({} bytes)...",
            preset_bytes.len()
        );
        upload_file(&mut client, &preset_path, preset_bytes).await?;
    }

    println!("Activating {}...", staged.active_path);
    call_ok(
        &mut client,
        Request::SetActiveEffect {
            path: Some(staged.active_path.clone()),
        },
        RPC_LOAD_TIMEOUT,
    )
    .await?;

    println!("Done.");
    Ok(())
}

fn load_and_validate_config(path: &Path) -> Result<Vec<u8>> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read config file {}", path.display()))?;
    if bytes.len() > MAX_CONFIG_JSON_SIZE {
        bail!(
            "config file {} exceeds maximum size ({} > {} bytes)",
            path.display(),
            bytes.len(),
            MAX_CONFIG_JSON_SIZE
        );
    }

    let _config: DeviceConfig = bled_core::io::decode_json(&bytes)
        .map_err(|e| anyhow!("invalid device config JSON in {}: {e}", path.display()))?;

    Ok(bytes)
}

fn load_and_prepare_effect(path: &Path) -> Result<StagedPayload> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read effect/preset file {}", path.display()))?;

    let is_wasm = bytes.starts_with(b"\0asm")
        || path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"));

    if is_wasm {
        let active_path = REMOTE_WASM_PATH
            .try_into()
            .map_err(|_| anyhow!("invalid remote WASM path"))?;
        return Ok(StagedPayload {
            wasm_bytes: bytes,
            preset_bytes: None,
            active_path,
        });
    }

    let mut preset: EffectPreset = bled_core::io::decode_json(&bytes)
        .map_err(|e| anyhow!("failed to parse preset JSON in {}: {e}", path.display()))?;

    let module_rel = preset.module().as_str().trim_start_matches('/');
    let preset_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let wasm_local_path = preset_dir.join(module_rel);
    let wasm_bytes = fs::read(&wasm_local_path).with_context(|| {
        format!(
            "failed to read WASM module {} referenced by preset {}",
            wasm_local_path.display(),
            path.display()
        )
    })?;

    let remote_wasm_path: PathString = REMOTE_WASM_PATH
        .try_into()
        .map_err(|_| anyhow!("invalid remote WASM path"))?;
    preset.set_module(remote_wasm_path);

    let mut preset_buf = vec![0u8; MAX_EFFECT_PRESET_JSON_SIZE];
    let preset_len = bled_core::io::encode_json(&preset, &mut preset_buf)
        .map_err(|e| anyhow!("failed to serialize updated preset JSON: {e}"))?;
    preset_buf.truncate(preset_len);

    let active_path = REMOTE_PRESET_PATH
        .try_into()
        .map_err(|_| anyhow!("invalid remote preset path"))?;

    Ok(StagedPayload {
        wasm_bytes,
        preset_bytes: Some(preset_buf),
        active_path,
    })
}

fn resolve_device(selector: Option<&str>) -> Result<TargetDevice> {
    let ports = tokio_serial::available_ports().context("failed to enumerate serial ports")?;

    if let Some(selector) = selector {
        for port in &ports {
            let usb_serial = usb_serial_number(port);
            if port.port_name == selector || usb_serial.as_deref() == Some(selector) {
                return Ok(TargetDevice {
                    port_name: port.port_name.clone(),
                    serial_number: usb_serial,
                });
            }
        }

        if Path::new(selector).exists() {
            return Ok(TargetDevice {
                port_name: selector.to_owned(),
                serial_number: None,
            });
        }

        bail!("no serial device matching '{selector}' was found");
    }

    let matching: Vec<&SerialPortInfo> = ports.iter().filter(|p| is_bled_usb(p)).collect();
    match matching.as_slice() {
        [] => bail!(
            "no BLED controller found (USB VID:PID {:04x}:{:04x})",
            USB_VID,
            USB_PID
        ),
        [port] => Ok(TargetDevice {
            port_name: port.port_name.clone(),
            serial_number: usb_serial_number(port),
        }),
        many => {
            let list = many
                .iter()
                .map(|p| {
                    format!(
                        "{} (serial: {})",
                        p.port_name,
                        usb_serial_number(p).as_deref().unwrap_or("unknown")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            bail!("multiple BLED controllers found ({list}); specify one with --device")
        }
    }
}

fn is_bled_usb(port: &SerialPortInfo) -> bool {
    matches!(
        &port.port_type,
        SerialPortType::UsbPort(usb) if usb.vid == USB_VID && usb.pid == USB_PID
    )
}

fn usb_serial_number(port: &SerialPortInfo) -> Option<String> {
    match &port.port_type {
        SerialPortType::UsbPort(usb) => usb.serial_number.clone(),
        _ => None,
    }
}

async fn connect(port_name: &str) -> Result<RpcClient> {
    // Assert DTR before opening the transport so the firmware's USB CDC task wakes up.
    if let Ok(mut port) = tokio_serial::new(port_name, BAUD_RATE)
        .dtr_on_open(true)
        .open()
    {
        let _ = port.write_data_terminal_ready(true);
    }

    let transport = SerialTransport::new(port_name, BAUD_RATE)
        .map_err(|e| anyhow!("failed to open serial transport on {port_name}: {e}"))?;
    let mut client = RpcClient::new(transport, RPC_ACK_TIMEOUT);

    // Verify connection by fetching device info.
    match call(&mut client, Request::GetDeviceInfo, RPC_RESPONSE_TIMEOUT).await? {
        Response::DeviceInfo(_) => Ok(client),
        other => bail!("unexpected response to GetDeviceInfo: {other:?}"),
    }
}

async fn reconnect(initial: &TargetDevice, selector: Option<&str>) -> Result<RpcClient> {
    let deadline = Instant::now() + RECONNECT_TIMEOUT;

    loop {
        let candidate_port = if let Some(serial) = initial.serial_number.as_deref() {
            resolve_device(Some(serial)).ok().map(|d| d.port_name)
        } else {
            resolve_device(selector)
                .ok()
                .map(|d| d.port_name)
                .or_else(|| {
                    Path::new(&initial.port_name)
                        .exists()
                        .then(|| initial.port_name.clone())
                })
        };

        if let Some(port_name) = candidate_port
            && let Ok(client) = connect(&port_name).await
        {
            println!("Reconnected to {port_name}.");
            return Ok(client);
        }

        if Instant::now() >= deadline {
            bail!("timed out waiting for device to reconnect after reboot");
        }

        sleep(RECONNECT_POLL_INTERVAL).await;
    }
}

async fn upload_file(client: &mut RpcClient, path: &PathString, data: &[u8]) -> Result<()> {
    let size = u32::try_from(data.len()).context("file is too large for device API")?;

    call_ok(
        client,
        Request::FileCreate {
            path: path.clone(),
            size,
        },
        RPC_RESPONSE_TIMEOUT,
    )
    .await
    .with_context(|| format!("failed to create {path} on device"))?;

    for (index, slice) in data.chunks(MAX_CHUNK_SIZE).enumerate() {
        let offset = u32::try_from(index * MAX_CHUNK_SIZE).unwrap();
        let chunk = Chunk::try_from(slice).map_err(|_| anyhow!("chunk exceeds maximum size"))?;
        call_ok(
            client,
            Request::FileWrite {
                path: path.clone(),
                offset,
                data: chunk,
            },
            RPC_RESPONSE_TIMEOUT,
        )
        .await
        .with_context(|| format!("failed to write chunk at offset {offset} to {path}"))?;
    }

    Ok(())
}

async fn call(client: &mut RpcClient, request: Request, timeout: Duration) -> Result<Response> {
    let response = client
        .call(request, timeout)
        .await
        .map_err(|e| anyhow!("RPC error: {e}"))?;
    match response {
        Response::Error(err) => bail!("device API error: {err:?}"),
        other => Ok(other),
    }
}

async fn call_ok(client: &mut RpcClient, request: Request, timeout: Duration) -> Result<()> {
    match call(client, request, timeout).await? {
        Response::Ok => Ok(()),
        other => bail!("unexpected RPC response: {other:?}"),
    }
}
