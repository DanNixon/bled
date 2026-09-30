use bled_api::{MAX_CHANNEL_COUNT, MAX_PATH_LEN};
use btleplug::api::BDAddr;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, str::FromStr};

#[derive(Parser, Debug)]
#[command(
    name = "bled-cli",
    version,
    about = "Interact with bled BLE LED controllers"
)]
pub struct Cli;

#[derive(Parser, Debug)]
#[command(name = "bled", disable_version_flag = true)]
struct ShellInput {
    #[command(subcommand)]
    command: ShellCommand,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum ShellCommand {
    /// Scan for bled devices and replace the current device list.
    Scan {
        /// Seconds to scan for devices.
        #[arg(long, default_value_t = 2.0, value_parser = parse_positive_f32)]
        scan_time: f32,
    },

    /// Connect to a device from the latest scan.
    Connect {
        /// Device index printed by scan, a MAC address, or part of a device name.
        #[arg(value_parser = ConnectTarget::from_str)]
        target: ConnectTarget,
    },

    /// Disconnect from the current device.
    Disconnect,

    /// Perform actions realted to the connected device.
    Device {
        #[command(subcommand)]
        action: DeviceAction,
    },

    /// Perform file operations on the connected device.
    File {
        #[command(subcommand)]
        action: FileAction,
    },

    /// Render the staged data for one or more channels.
    Commit {
        /// Zero-based channel indices.
        #[arg(required = true, num_args = 1.., value_parser = parse_channel_index)]
        channels: Vec<u8>,
    },

    /// Perform LED buffer operations on the connected device.
    Buffer {
        #[command(subcommand)]
        action: BufferAction,
    },

    /// Stage one solid-colour pixel range.
    Range {
        /// Zero-based channel index.
        #[arg(value_parser = parse_channel_index)]
        channel: u8,
        /// Zero-based index of the first pixel.
        start: u16,
        /// Number of pixels to set.
        #[arg(value_parser = parse_positive_u16)]
        count: u16,
        /// Red component.
        red: u8,
        /// Green component.
        green: u8,
        /// Blue component.
        blue: u8,
    },

    /// Disconnect and leave the shell.
    #[command(alias = "exit")]
    Quit,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum DeviceAction {
    /// Report firmware and runtime information from the connected device.
    Info,

    /// Reset the connected device.
    Reset,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum FileAction {
    /// Read a file from the connected device.
    Read {
        /// Path of the file on the device.
        #[arg(value_parser = parse_remote_path)]
        remote_path: String,
        /// Local file path to write to (prints to terminal if omitted).
        local_path: Option<PathBuf>,
    },

    /// Write a local file to the connected device.
    Write {
        /// Path of the local file to read.
        local_path: PathBuf,
        /// Path of the file on the device.
        #[arg(value_parser = parse_remote_path)]
        remote_path: String,
    },

    /// Delete a file on the connected device.
    Delete {
        /// Path of the file on the device to delete.
        #[arg(value_parser = parse_remote_path)]
        remote_path: String,
    },

    /// Stat a file on the connected device.
    Stat {
        /// Path of the file on the device.
        #[arg(value_parser = parse_remote_path)]
        remote_path: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum BufferAction {
    /// Read the current buffer information.
    Info,

    /// Read the raw LED buffer from the connected device.
    Read {
        /// Local file path to write to (prints to terminal as hex if omitted).
        local_path: Option<PathBuf>,
    },

    /// Write raw data to the LED buffer on the connected device.
    Write {
        /// Path of the local file to read.
        local_path: PathBuf,

        /// Zero-based byte offset in the buffer to write to.
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
}

/// How the user identified the device they want to connect to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectTarget {
    /// The index printed alongside the device by the scan command.
    Index(usize),
    /// An exact Bluetooth device address.
    Address(BDAddr),
    /// A case-insensitive substring of the device name.
    Name(String),
}

impl FromStr for ConnectTarget {
    type Err = String;

    /// Parses a connect target, preferring the least ambiguous interpretation.
    ///
    /// A bare number is always a scan index, so an address made only of decimal
    /// digits must be written with `:` separators to be recognised as one.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err("expected a scan index, MAC address, or device name".to_owned());
        }
        if let Ok(index) = value.parse::<usize>() {
            return Ok(Self::Index(index));
        }
        if let Ok(address) = BDAddr::from_str(value) {
            return Ok(Self::Address(address));
        }
        Ok(Self::Name(value.to_lowercase()))
    }
}

pub fn parse_command(line: &str) -> Result<ShellCommand, clap::Error> {
    ShellInput::try_parse_from(std::iter::once("bled").chain(line.split_whitespace()))
        .map(|input| input.command)
}

/// Names of every shell command, for tab completion.
pub const COMMAND_NAMES: &[&str] = &[
    "scan",
    "connect",
    "disconnect",
    "device-info",
    "reset",
    "range",
    "commit",
    "buffer",
    "file",
    "quit",
    "exit",
    "help",
];

/// Subcommands for the `buffer` command, for tab completion.
pub const BUFFER_SUBCOMMAND_NAMES: &[&str] = &["info", "read", "write"];

/// Subcommands for the `file` command, for tab completion.
pub const FILE_SUBCOMMAND_NAMES: &[&str] = &["read", "write", "delete", "stat"];

fn parse_remote_path(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err("remote path cannot be empty".to_owned());
    }
    if value.len() > MAX_PATH_LEN {
        return Err(format!(
            "remote path cannot exceed {MAX_PATH_LEN} bytes (got {})",
            value.len()
        ));
    }
    Ok(value.to_owned())
}

fn parse_positive_f32(value: &str) -> Result<f32, String> {
    let value: f32 = value
        .parse()
        .map_err(|_| "expected a positive number".to_owned())?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err("expected a positive number".to_owned())
    }
}

fn parse_positive_u16(value: &str) -> Result<u16, String> {
    let value: u16 = value
        .parse()
        .map_err(|_| "expected a positive integer no greater than 65535".to_owned())?;
    if value > 0 {
        Ok(value)
    } else {
        Err("expected a positive integer".to_owned())
    }
}

fn parse_channel_index(value: &str) -> Result<u8, String> {
    let index: u8 = value.parse().map_err(|_| {
        format!(
            "expected a channel index from 0 to {}",
            MAX_CHANNEL_COUNT - 1
        )
    })?;
    if usize::from(index) < MAX_CHANNEL_COUNT {
        Ok(index)
    } else {
        Err(format!(
            "expected a channel index from 0 to {}",
            MAX_CHANNEL_COUNT - 1
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_commands_reject_invalid_options() {
        for command in [
            "scan --scan-time 0",
            "scan --scan-time inf",
            "scan --name bled",
            "scan --addr aa:bb:cc:dd:ee:ff",
            "swipe --fps 0",
            "range 8 0 1 0 0 0",
            "range 0 0 0 0 0 0",
            "range 0 65536 1 0 0 0",
            "range 0 0 65536 0 0 0",
            "range 0 0 1 256 0 0",
            "commit",
            "commit 8",
            "commit 1 8 3",
            "buffer",
            "buffer unknown",
            "buffer write",
            "file",
            "file unknown",
            "file read",
            "file write local.json",
            "file delete",
            "file stat",
            "file read /this/path/is/way/too/long/and/exceeds/sixty/four/bytes/limit/set/by/protocol/config.json",
        ] {
            assert!(parse_command(command).is_err(), "{command}");
        }
        assert_eq!(parse_channel_index("7"), Ok(7));
    }

    #[test]
    fn connect_targets_prefer_index_then_address_then_name() {
        let address = BDAddr::from_str("aa:bb:cc:dd:ee:ff").unwrap();

        // A bare number is always a scan index.
        assert_eq!(ConnectTarget::from_str("0"), Ok(ConnectTarget::Index(0)));
        assert_eq!(ConnectTarget::from_str("12"), Ok(ConnectTarget::Index(12)));

        // Addresses are recognised with and without separators.
        assert_eq!(
            ConnectTarget::from_str("aa:bb:cc:dd:ee:ff"),
            Ok(ConnectTarget::Address(address))
        );
        assert_eq!(
            ConnectTarget::from_str("AA:BB:CC:DD:EE:FF"),
            Ok(ConnectTarget::Address(address))
        );
        assert_eq!(
            ConnectTarget::from_str("aabbccddeeff"),
            Ok(ConnectTarget::Address(address))
        );

        // Anything else is a name substring, lowercased for comparison.
        assert_eq!(
            ConnectTarget::from_str("Bled"),
            Ok(ConnectTarget::Name("bled".to_owned()))
        );
        assert_eq!(
            ConnectTarget::from_str("kitchen-1"),
            Ok(ConnectTarget::Name("kitchen-1".to_owned()))
        );

        // A name that happens to look like a truncated address is still a name.
        assert_eq!(
            ConnectTarget::from_str("aa:bb"),
            Ok(ConnectTarget::Name("aa:bb".to_owned()))
        );

        assert!(ConnectTarget::from_str("").is_err());
    }

    #[test]
    fn connect_accepts_every_target_form() {
        assert_eq!(
            parse_command("connect bedroom").unwrap(),
            ShellCommand::Connect {
                target: ConnectTarget::Name("bedroom".to_owned())
            }
        );
        assert_eq!(
            parse_command("connect aa:bb:cc:dd:ee:ff").unwrap(),
            ShellCommand::Connect {
                target: ConnectTarget::Address(BDAddr::from_str("aa:bb:cc:dd:ee:ff").unwrap())
            }
        );
    }

    #[test]
    fn command_names_cover_every_shell_command() {
        // Every completion candidate must actually parse as a command, so the
        // list cannot drift out of sync with the parser.
        for name in COMMAND_NAMES {
            let error = parse_command(name).err().map(|error| error.kind());
            assert!(
                !matches!(
                    error,
                    Some(
                        clap::error::ErrorKind::InvalidSubcommand
                            | clap::error::ErrorKind::UnknownArgument
                    )
                ),
                "{name} is not a valid command"
            );
        }
    }

    #[test]
    fn buffer_commands_parse() {
        assert_eq!(
            parse_command("buffer info").unwrap(),
            ShellCommand::Buffer {
                action: BufferAction::Info
            }
        );
        assert_eq!(
            parse_command("buffer read").unwrap(),
            ShellCommand::Buffer {
                action: BufferAction::Read { local_path: None }
            }
        );
        assert_eq!(
            parse_command("buffer read out.bin").unwrap(),
            ShellCommand::Buffer {
                action: BufferAction::Read {
                    local_path: Some(PathBuf::from("out.bin"))
                }
            }
        );
        assert_eq!(
            parse_command("buffer write in.bin").unwrap(),
            ShellCommand::Buffer {
                action: BufferAction::Write {
                    local_path: PathBuf::from("in.bin"),
                    offset: 0,
                }
            }
        );
        assert_eq!(
            parse_command("buffer write in.bin --offset 42").unwrap(),
            ShellCommand::Buffer {
                action: BufferAction::Write {
                    local_path: PathBuf::from("in.bin"),
                    offset: 42,
                }
            }
        );
    }
}
