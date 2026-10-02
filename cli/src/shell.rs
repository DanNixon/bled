use crate::{
    cli::{
        BufferAction, ConfigAction, ConnectTarget, DeviceAction, FileAction, ShellCommand,
        parse_command,
    },
    device::{self, BledService, Candidate, DeviceIdentity},
    prompt::Prompt,
};
use anyhow::{Context, Result, anyhow, bail};
use bled_api::{DeviceConfig, DeviceInfo, RGB8};
use btleplug::{api::Peripheral as _, platform::Adapter};
use std::{path::Path, time::Duration};

/// A connected device and the characteristics discovered on it.
///
/// The service is resolved once at connect time rather than on every command.
#[derive(Clone)]
struct Connection {
    candidate: Candidate,
    service: BledService,
}

pub struct Shell {
    adapter: Adapter,
    candidates: Vec<Candidate>,
    connected: Option<Connection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellFlow {
    Continue,
    Quit,
}

impl Shell {
    pub fn new(adapter: Adapter) -> Self {
        Self {
            adapter,
            candidates: Vec::new(),
            connected: None,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut prompt = Prompt::new()?;
        let result = self.run_loop(&mut prompt).await;

        prompt.save_history();
        self.disconnect_on_shutdown().await;
        result
    }

    async fn run_loop(&mut self, prompt: &mut Prompt) -> Result<()> {
        loop {
            let identities: Vec<_> = self.candidates.iter().map(Candidate::identity).collect();
            prompt.set_devices(completion_tokens(&identities));
            let Some(line) = prompt.read_line().await? else {
                println!();
                return Ok(());
            };
            if line.trim().is_empty() {
                continue;
            }
            let command = match parse_command(&line) {
                Ok(command) => command,
                Err(error) => {
                    // clap writes help and errors itself; a failure to print is
                    // not worth aborting the shell over.
                    let _ = error.print();
                    continue;
                }
            };
            match self.execute(command).await {
                Ok(ShellFlow::Continue) => {}
                Ok(ShellFlow::Quit) => return Ok(()),
                Err(error) => eprintln!("{error:#}"),
            }
        }
    }

    async fn execute(&mut self, command: ShellCommand) -> Result<ShellFlow> {
        match command {
            ShellCommand::Scan { scan_time } => self.scan(scan_time).await?,
            ShellCommand::Connect { target } => self.connect(&target).await?,
            ShellCommand::Disconnect => self.disconnect().await?,
            ShellCommand::Device { action } => match action {
                DeviceAction::Info => self.device_info().await?,
                DeviceAction::Reset => self.reset().await?,
                DeviceAction::Config { action } => match action {
                    ConfigAction::Stat => self.device_config_stat().await?,
                    ConfigAction::Read { local_path } => {
                        self.device_config_read(local_path.as_deref()).await?;
                    }
                },
            },
            ShellCommand::File { action } => match action {
                FileAction::Read {
                    remote_path,
                    local_path,
                } => {
                    self.file_read(&remote_path, local_path.as_deref()).await?;
                }
                FileAction::Write {
                    local_path,
                    remote_path,
                } => {
                    self.file_write(&local_path, &remote_path).await?;
                }
                FileAction::Delete { remote_path } => {
                    self.file_delete(&remote_path).await?;
                }
                FileAction::Stat { remote_path } => {
                    self.file_stat(&remote_path).await?;
                }
            },
            ShellCommand::Commit { channels } => self.commit(&channels).await?,
            ShellCommand::Buffer { action } => match action {
                BufferAction::Info => {
                    self.buffer_info().await?;
                }
                BufferAction::Read { local_path } => {
                    self.buffer_read(local_path.as_deref()).await?;
                }
                BufferAction::Write { local_path, offset } => {
                    self.buffer_write(&local_path, offset).await?;
                }
            },
            ShellCommand::Range {
                channel,
                start,
                count,
                red,
                green,
                blue,
            } => {
                self.range(channel, start, count, RGB8::new(red, green, blue))
                    .await?;
            }
            ShellCommand::Quit => return Ok(ShellFlow::Quit),
        }
        Ok(ShellFlow::Continue)
    }

    async fn scan(&mut self, scan_time: f32) -> Result<()> {
        println!("scanning for {scan_time} seconds...");
        let devices = device::scan(&self.adapter, Duration::from_secs_f32(scan_time))
            .await
            .context("scan failed")?;
        self.candidates = device::filter_candidates(devices, None, None)
            .await
            .context("could not inspect scan results")?;
        if self.candidates.is_empty() {
            println!("no bled devices found");
        } else {
            for (index, candidate) in self.candidates.iter().enumerate() {
                println!("{index}: {}", candidate.label());
            }
        }
        Ok(())
    }

    async fn connect(&mut self, target: &ConnectTarget) -> Result<()> {
        let identities: Vec<_> = self.candidates.iter().map(Candidate::identity).collect();
        let index = resolve_target(&identities, target).map_err(|message| anyhow!(message))?;
        let candidate = self.candidates[index].clone();
        if let Some(current) = self.connected.as_ref() {
            match current.candidate.peripheral.is_connected().await {
                Ok(true) => {
                    bail!(
                        "already connected to {}; disconnect first",
                        current.candidate.label()
                    );
                }
                Ok(false) => self.connected = None,
                Err(error) => return Err(error).context("could not check the current connection"),
            }
        }
        let label = candidate.label();
        println!("connecting to {label}...");
        if let Err(error) = device::connect(&candidate.peripheral).await {
            self.clean_up_failed_connection(&candidate, &label).await;
            return Err(error).with_context(|| format!("could not connect to {label}"));
        }
        // Resolve the characteristics once, here, so a device that does not
        // speak the bled protocol is rejected at connect time rather than at
        // the first command that needs it.
        let service = match device::find_service(&candidate.peripheral.characteristics()) {
            Ok(service) => service,
            Err(error) => {
                self.clean_up_failed_connection(&candidate, &label).await;
                return Err(error).with_context(|| format!("could not use {label}"));
            }
        };
        println!("connected to {label}");
        self.connected = Some(Connection { candidate, service });
        Ok(())
    }

    async fn clean_up_failed_connection(&self, candidate: &Candidate, label: &str) {
        if candidate.peripheral.is_connected().await.unwrap_or(false)
            && let Err(error) = candidate.peripheral.disconnect().await
        {
            log::warn!("{label}: cleanup after connection failure failed: {error}");
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        let Some(connection) = self.connected.take() else {
            bail!("no device is connected");
        };
        let label = connection.candidate.label();
        match connection.candidate.peripheral.disconnect().await {
            Ok(()) => println!("disconnected from {label}"),
            Err(error) => {
                self.connected = Some(connection);
                return Err(error).with_context(|| format!("could not disconnect from {label}"));
            }
        }
        Ok(())
    }

    async fn device_info(&mut self) -> Result<()> {
        let connection = self.active_connection().await?;
        let info = device::read_device_info(
            &connection.candidate.peripheral,
            &connection.service.device_info,
        )
        .await
        .with_context(|| connection.candidate.label())?;
        println!("{}", format_device_info(&info));
        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        let connection = self.active_connection().await?;
        let label = connection.candidate.label();
        device::reset(&connection.candidate.peripheral, &connection.service.reset)
            .await
            .with_context(|| label.clone())?;
        self.connected = None;
        println!("reset requested for {label}");
        Ok(())
    }

    async fn device_config_stat(&mut self) -> Result<()> {
        let connection = self.active_connection().await?;
        let stat = device::stat_config(
            &connection.candidate.peripheral,
            &connection.service.config_control,
            &connection.service.config_data,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!("active config: {} bytes", stat.size());
        Ok(())
    }

    async fn device_config_read(&mut self, local_path: Option<&Path>) -> Result<()> {
        let connection = self.active_connection().await?;
        let data = device::read_config(
            &connection.candidate.peripheral,
            &connection.service.config_control,
            &connection.service.config_data,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        match bled_api::io::decode_cbor::<DeviceConfig>(&data) {
            Ok(config) => {
                let mut json_buf = [0u8; 4096];
                let len = bled_api::io::encode_json(&config, &mut json_buf)
                    .map_err(|error| anyhow!("encoding device config as JSON: {error}"))?;
                let json_bytes = &json_buf[..len];
                match local_path {
                    Some(path) => {
                        let bytes_to_write =
                            if path.extension().and_then(|s| s.to_str()) == Some("cbor") {
                                &data[..]
                            } else {
                                json_bytes
                            };
                        std::fs::write(path, bytes_to_write)
                            .with_context(|| format!("writing to local file {}", path.display()))?;
                        println!("read {} bytes of config to {}", data.len(), path.display());
                    }
                    None => match std::str::from_utf8(json_bytes) {
                        Ok(text) => println!("{text}"),
                        Err(_) => {
                            println!(
                                "<{} bytes of config data; specify a local path to save>",
                                data.len()
                            );
                        }
                    },
                }
            }
            Err(error) => {
                if let Some(path) = local_path {
                    std::fs::write(path, &data)
                        .with_context(|| format!("writing to local file {}", path.display()))?;
                    println!(
                        "read {} bytes of raw config to {}",
                        data.len(),
                        path.display()
                    );
                } else {
                    bail!("decoding device config: {error}");
                }
            }
        }
        Ok(())
    }

    async fn range(&mut self, channel: u8, start: u16, count: u16, colour: RGB8) -> Result<()> {
        let connection = self.active_connection().await?;
        device::send_range(
            &connection.candidate.peripheral,
            &connection.service.range_data,
            channel,
            start,
            count,
            colour,
        )
        .await
        .with_context(|| connection.candidate.label())?;
        println!("staged {count} pixels from pixel {start} on channel {channel}");
        Ok(())
    }

    async fn commit(&mut self, channels: &[u8]) -> Result<()> {
        let connection = self.active_connection().await?;
        device::commit_channels(
            &connection.candidate.peripheral,
            &connection.service.commit,
            channels,
        )
        .await
        .with_context(|| connection.candidate.label())?;
        println!(
            "rendered channels {}",
            channels
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(())
    }

    async fn file_read(&mut self, remote_path: &str, local_path: Option<&Path>) -> Result<()> {
        let connection = self.active_connection().await?;
        let data = device::read_file(
            &connection.candidate.peripheral,
            &connection.service.file_io_control,
            &connection.service.file_io_data,
            remote_path,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        match local_path {
            Some(path) => {
                std::fs::write(path, &data)
                    .with_context(|| format!("writing to local file {}", path.display()))?;
                println!(
                    "read {} bytes from {remote_path} to {}",
                    data.len(),
                    path.display()
                );
            }
            None => match std::str::from_utf8(&data) {
                Ok(text) => println!("{text}"),
                Err(_) => {
                    println!(
                        "<{} bytes of binary data; specify a local path to save>",
                        data.len()
                    );
                }
            },
        }
        Ok(())
    }

    async fn file_write(&mut self, local_path: &Path, remote_path: &str) -> Result<()> {
        let connection = self.active_connection().await?;
        let data = std::fs::read(local_path)
            .with_context(|| format!("reading local file {}", local_path.display()))?;
        let byte_count = data.len();
        device::write_file(
            &connection.candidate.peripheral,
            &connection.service.file_io_control,
            &connection.service.file_io_data,
            remote_path,
            &data,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!(
            "wrote {byte_count} bytes from {} to {remote_path}",
            local_path.display()
        );
        Ok(())
    }

    async fn file_delete(&mut self, remote_path: &str) -> Result<()> {
        let connection = self.active_connection().await?;
        device::delete_file(
            &connection.candidate.peripheral,
            &connection.service.file_io_control,
            remote_path,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!("deleted {remote_path}");
        Ok(())
    }

    async fn file_stat(&mut self, remote_path: &str) -> Result<()> {
        let connection = self.active_connection().await?;
        let stat = device::stat_file(
            &connection.candidate.peripheral,
            &connection.service.file_io_control,
            &connection.service.file_io_data,
            remote_path,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!("{remote_path}: {} bytes", stat.size());
        Ok(())
    }

    async fn buffer_info(&mut self) -> Result<()> {
        let connection = self.active_connection().await?;
        let info = device::read_led_buffer_info(
            &connection.candidate.peripheral,
            &connection.service.led_buffer_control,
            &connection.service.led_buffer_data,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!(
            "capacity: {} bytes, size: {} bytes",
            info.capacity(),
            info.size()
        );
        Ok(())
    }

    async fn buffer_read(&mut self, local_path: Option<&Path>) -> Result<()> {
        let connection = self.active_connection().await?;
        let data = device::read_led_buffer(
            &connection.candidate.peripheral,
            &connection.service.led_buffer_control,
            &connection.service.led_buffer_data,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        match local_path {
            Some(path) => {
                std::fs::write(path, &data)
                    .with_context(|| format!("writing to local file {}", path.display()))?;
                println!(
                    "read {} bytes from LED buffer to {}",
                    data.len(),
                    path.display()
                );
            }
            None => {
                println!(
                    "<{} bytes of LED buffer data; specify a local path to save>",
                    data.len()
                );
            }
        }
        Ok(())
    }

    async fn buffer_write(&mut self, local_path: &Path, offset: usize) -> Result<()> {
        let connection = self.active_connection().await?;
        let data = std::fs::read(local_path)
            .with_context(|| format!("reading local file {}", local_path.display()))?;
        let byte_count = data.len();
        device::write_led_buffer(
            &connection.candidate.peripheral,
            &connection.service.led_buffer_control,
            &connection.service.led_buffer_data,
            &data,
            offset,
        )
        .await
        .with_context(|| connection.candidate.label())?;

        println!(
            "wrote {byte_count} bytes from {} to LED buffer at offset {offset}",
            local_path.display()
        );
        Ok(())
    }

    /// Returns the current connection, or reports why there is not one.
    async fn active_connection(&mut self) -> Result<Connection> {
        let Some(connection) = self.connected.as_ref().cloned() else {
            bail!("no device is connected; run scan and connect first");
        };
        match connection.candidate.peripheral.is_connected().await {
            Ok(true) => Ok(connection),
            Ok(false) => {
                self.connected = None;
                bail!("{} is no longer connected", connection.candidate.label());
            }
            Err(error) => Err(error).context("could not check the connection"),
        }
    }

    async fn disconnect_on_shutdown(&mut self) {
        if let Some(connection) = self.connected.take() {
            let label = connection.candidate.label();
            if let Err(error) = connection.candidate.peripheral.disconnect().await {
                log::warn!("{label}: disconnect during shutdown failed: {error}");
            }
        }
    }
}

/// Finds the index of the device a connect target refers to, or explains why
/// it cannot be resolved.
fn resolve_target(devices: &[DeviceIdentity<'_>], target: &ConnectTarget) -> Result<usize, String> {
    if devices.is_empty() {
        return Err("no devices known; run scan first".to_owned());
    }
    match target {
        ConnectTarget::Index(index) => devices
            .get(*index)
            .map(|_| *index)
            .ok_or_else(|| format!("no device {index}; run scan to list available devices")),
        ConnectTarget::Address(address) => devices
            .iter()
            .position(|device| device.address == Some(*address))
            .ok_or_else(|| format!("no device with address {address} in the last scan")),
        ConnectTarget::Name(needle) => {
            let matches: Vec<usize> = devices
                .iter()
                .enumerate()
                .filter(|(_, device)| {
                    device
                        .name
                        .is_some_and(|name| name.to_lowercase().contains(needle))
                })
                .map(|(index, _)| index)
                .collect();
            match matches.as_slice() {
                [] => Err(format!("no device name contains \"{needle}\"")),
                [index] => Ok(*index),
                ambiguous => {
                    let mut message = format!("{} devices match \"{needle}\":", ambiguous.len());
                    for index in ambiguous {
                        message.push_str(&format!("\n  {index}: {}", devices[*index].label()));
                    }
                    message.push_str("\nconnect by index or address instead");
                    Err(message)
                }
            }
        }
    }
}

/// Tokens offered when completing a `connect` argument.
///
/// Names containing whitespace are omitted because commands are split on
/// whitespace, so such a name cannot be typed as a single argument anyway.
fn completion_tokens(devices: &[DeviceIdentity<'_>]) -> Vec<String> {
    let mut tokens = Vec::with_capacity(devices.len() * 2);
    for device in devices {
        if let Some(name) = device.name
            && !name.is_empty()
            && !name.contains(char::is_whitespace)
        {
            tokens.push(name.to_owned());
        }
        if let Some(address) = device.address {
            tokens.push(address.to_string());
        }
    }
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

fn format_device_info(info: &DeviceInfo) -> String {
    format!(
        "git revision: {}\nboot reason: {:?}\nuptime: {} ms",
        info.git_revision(),
        info.boot_reason(),
        info.uptime_ms()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_api::BootReason;
    use btleplug::api::BDAddr;
    use std::str::FromStr as _;

    #[test]
    fn device_info_reports_all_fields() {
        let info = DeviceInfo::new(
            "deadbeef".try_into().unwrap(),
            BootReason::WatchdogTimeout,
            123_456,
        );

        assert_eq!(
            format_device_info(&info),
            "git revision: deadbeef\nboot reason: WatchdogTimeout\nuptime: 123456 ms"
        );
    }

    fn address(last: u8) -> BDAddr {
        BDAddr::from_str(&format!("aa:bb:cc:dd:ee:{last:02x}")).unwrap()
    }

    fn devices() -> Vec<DeviceIdentity<'static>> {
        vec![
            DeviceIdentity {
                name: Some("bled-kitchen"),
                address: Some(address(1)),
            },
            DeviceIdentity {
                name: Some("bled-hall"),
                address: Some(address(2)),
            },
            DeviceIdentity {
                name: None,
                address: Some(address(3)),
            },
        ]
    }

    #[test]
    fn no_devices_asks_for_a_scan() {
        for target in [
            ConnectTarget::Index(0),
            ConnectTarget::Address(address(1)),
            ConnectTarget::Name("bled".to_owned()),
        ] {
            let error = resolve_target(&[], &target).unwrap_err();
            assert!(error.contains("run scan first"), "{error}");
        }
    }

    #[test]
    fn index_selects_by_position() {
        assert_eq!(resolve_target(&devices(), &ConnectTarget::Index(1)), Ok(1));
        let error = resolve_target(&devices(), &ConnectTarget::Index(9)).unwrap_err();
        assert!(error.contains("no device 9"), "{error}");
    }

    #[test]
    fn address_selects_by_exact_match() {
        assert_eq!(
            resolve_target(&devices(), &ConnectTarget::Address(address(3))),
            Ok(2)
        );
        let error = resolve_target(&devices(), &ConnectTarget::Address(address(9))).unwrap_err();
        assert!(error.contains("no device with address"), "{error}");
    }

    #[test]
    fn name_selects_by_unique_substring() {
        assert_eq!(
            resolve_target(&devices(), &ConnectTarget::Name("hall".to_owned())),
            Ok(1)
        );
        // Matching is case-insensitive; ConnectTarget lowercases on parse.
        assert_eq!(
            resolve_target(&devices(), &ConnectTarget::Name("kitchen".to_owned())),
            Ok(0)
        );
    }

    #[test]
    fn ambiguous_name_lists_the_matches() {
        let error =
            resolve_target(&devices(), &ConnectTarget::Name("bled".to_owned())).unwrap_err();
        assert!(error.starts_with("2 devices match \"bled\":"), "{error}");
        assert!(error.contains("0: bled-kitchen"), "{error}");
        assert!(error.contains("1: bled-hall"), "{error}");
    }

    #[test]
    fn unmatched_name_is_reported() {
        let error =
            resolve_target(&devices(), &ConnectTarget::Name("garage".to_owned())).unwrap_err();
        assert!(error.contains("no device name contains"), "{error}");
    }

    #[test]
    fn completion_offers_names_and_addresses() {
        let tokens = completion_tokens(&devices());
        assert!(tokens.contains(&"bled-kitchen".to_owned()));
        assert!(tokens.contains(&"bled-hall".to_owned()));
        assert!(tokens.contains(&address(3).to_string()));
        // The unnamed device contributes only its address.
        assert_eq!(tokens.len(), 5);
    }

    #[test]
    fn completion_skips_names_that_cannot_be_typed() {
        let devices = [
            DeviceIdentity {
                name: Some("living room"),
                address: Some(address(1)),
            },
            DeviceIdentity {
                name: Some(""),
                address: None,
            },
        ];
        let tokens = completion_tokens(&devices);
        assert_eq!(tokens, [address(1).to_string()]);
    }
}
