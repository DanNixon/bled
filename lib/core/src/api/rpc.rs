//! RPC API
//!
//! The device API is a set of request/response remote procedure calls carried
//! over a bidirectional byte stream (such as a UART, USB CDC ACM device, or
//! emulated serial over BLE) using the [`teeny-rpc`] protocol.
//!
//! Responses that transfer more than [`MAX_CHUNK_SIZE`] bytes of data are
//! chunked by the client using the `offset`/`length` fields of the relevant
//! requests.
//!
//! [`teeny-rpc`]: https://docs.rs/teeny-rpc

use super::{DeviceInfo, DirEntry, LedBufferInformation};
use crate::PathString;
use core::time::Duration;
use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Maximum size of a data chunk transferred in a single RPC.
pub const MAX_CHUNK_SIZE: usize = 512;

/// Channel ID used for API RPCs.
pub const RPC_CHANNEL_ID: u8 = 1;

/// Maximum encoded RPC frame size, including header, payload, CRC, COBS
/// overhead, and the trailing delimiter.
pub const RPC_FRAME_BUF_LEN: usize = 640;

/// Timeout for RPC request and response acknowledgements.
pub const RPC_ACK_TIMEOUT: Duration = Duration::from_millis(1000);

/// Default timeout for RPC responses.
pub const RPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for RPC responses to requests that load effects.
pub const RPC_LOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// A chunk of data transferred within a single RPC.
pub type Chunk = Vec<u8, MAX_CHUNK_SIZE>;

/// RPC request.
#[allow(clippy::large_enum_variant)] // Inline fixed-capacity chunks keep the no-alloc API simple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Request {
    /// Fetch device information.
    GetDeviceInfo,

    /// Reboot the device.
    Reboot,

    /// Stat the active device configuration (CBOR encoded).
    ConfigStat,

    /// Read a chunk of the active device configuration (CBOR encoded).
    ConfigRead { offset: u32, length: u32 },

    /// Render the staged contents of the LED buffer.
    Commit,

    /// Stat a file.
    FileStat { path: PathString },

    /// Read a chunk of a file.
    FileRead {
        path: PathString,
        offset: u32,
        length: u32,
    },

    /// Write a chunk to a file.
    FileWrite {
        path: PathString,
        offset: u32,
        data: Chunk,
    },

    /// Create a file and preallocate it to the given size.
    FileCreate { path: PathString, size: u32 },

    /// Delete a file.
    FileDelete { path: PathString },

    /// Read a single directory entry at the given index.
    ReadDirEntry { path: PathString, index: u32 },

    /// Fetch LED buffer information.
    LedBufferInfo,

    /// Read a chunk of the LED buffer.
    LedBufferRead { offset: u32, length: u32 },

    /// Write a chunk to the LED buffer.
    LedBufferWrite { offset: u32, data: Chunk },

    /// Fetch the path of the active effect.
    GetActiveEffect,

    /// Activate an effect, or unload the active effect if no path is given.
    SetActiveEffect { path: Option<PathString> },

    /// Stat the manifest of the active effect (JSON encoded).
    EffectManifestStat,

    /// Read a chunk of the manifest of the active effect (JSON encoded).
    EffectManifestRead { offset: u32, length: u32 },

    /// Stat the command buffer of the active effect.
    EffectCommandStat,

    /// Read the command buffer of the active effect.
    EffectCommandRead,

    /// Write the command buffer of the active effect.
    EffectCommandWrite { data: Chunk },

    /// Stat the parameter buffer of the active effect.
    EffectParameterStat,

    /// Read the parameter buffer of the active effect.
    EffectParameterRead,

    /// Write the parameter buffer of the active effect.
    EffectParameterWrite { data: Chunk },
}

/// RPC response.
#[allow(clippy::large_enum_variant)] // Inline fixed-capacity chunks keep the no-alloc API simple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Response {
    DeviceInfo(DeviceInfo),
    Ok,
    Size(u64),
    Data(Chunk),
    DirEntry(Option<DirEntry>),
    LedBufferInfo(LedBufferInformation),
    ActiveEffect(Option<PathString>),
    Error(ApiError),
}

/// API error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ApiError {
    /// The requested path does not exist.
    NotFound,

    /// A request argument is invalid.
    InvalidArgument,

    /// An offset or length is out of bounds.
    OutOfBounds,

    /// No effect is active.
    NoActiveEffect,

    /// The active effect failed to handle a request.
    EffectFailure,

    /// A storage error occurred.
    Storage,

    /// An internal device error occurred.
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::BootReason;
    use heapless::String;

    fn round_trip_request(request: &Request) {
        let mut buffer = [0u8; 2048];
        let serialized = postcard::to_slice(request, &mut buffer).unwrap();
        let decoded: Request = postcard::from_bytes(serialized).unwrap();
        assert_eq!(&decoded, request);
    }

    fn round_trip_response(response: &Response) {
        let mut buffer = [0u8; 2048];
        let serialized = postcard::to_slice(response, &mut buffer).unwrap();
        let decoded: Response = postcard::from_bytes(serialized).unwrap();
        assert_eq!(&decoded, response);
    }

    fn chunk(data: &[u8]) -> Chunk {
        data.try_into().unwrap()
    }

    #[test]
    fn simple_requests_round_trip() {
        for request in [
            Request::GetDeviceInfo,
            Request::Reboot,
            Request::ConfigStat,
            Request::Commit,
            Request::LedBufferInfo,
            Request::GetActiveEffect,
            Request::EffectManifestStat,
            Request::EffectCommandStat,
            Request::EffectCommandRead,
            Request::EffectParameterStat,
            Request::EffectParameterRead,
        ] {
            round_trip_request(&request);
        }
    }

    #[test]
    fn parameterised_requests_round_trip() {
        round_trip_request(&Request::ConfigRead {
            offset: 512,
            length: 256,
        });
        round_trip_request(&Request::FileStat {
            path: "/effect.wasm".try_into().unwrap(),
        });
        round_trip_request(&Request::FileRead {
            path: "/effect.wasm".try_into().unwrap(),
            offset: 1024,
            length: 512,
        });
        round_trip_request(&Request::FileWrite {
            path: "/effect.wasm".try_into().unwrap(),
            offset: 2048,
            data: chunk(&[42; MAX_CHUNK_SIZE]),
        });
        round_trip_request(&Request::FileCreate {
            path: "/new.bin".try_into().unwrap(),
            size: 4096,
        });
        round_trip_request(&Request::FileDelete {
            path: "/old.bin".try_into().unwrap(),
        });
        round_trip_request(&Request::ReadDirEntry {
            path: "/".try_into().unwrap(),
            index: 7,
        });
        round_trip_request(&Request::LedBufferRead {
            offset: 128,
            length: 512,
        });
        round_trip_request(&Request::LedBufferWrite {
            offset: 0,
            data: chunk(&[1; 3]),
        });
        round_trip_request(&Request::SetActiveEffect {
            path: Some("/effects/demo.json".try_into().unwrap()),
        });
        round_trip_request(&Request::SetActiveEffect { path: None });
        round_trip_request(&Request::EffectManifestRead {
            offset: 16,
            length: 512,
        });
        round_trip_request(&Request::EffectCommandWrite {
            data: chunk(&[7; 16]),
        });
        round_trip_request(&Request::EffectParameterWrite {
            data: chunk(&[9; 16]),
        });
    }

    #[test]
    fn responses_round_trip() {
        round_trip_response(&Response::DeviceInfo(DeviceInfo::new(
            String::try_from("a1b2c3d4").unwrap(),
            BootReason::WatchdogTimeout,
            42_000,
            16 * 1024,
            4,
        )));
        round_trip_response(&Response::Ok);
        round_trip_response(&Response::Size(65536));
        round_trip_response(&Response::Data(chunk(&[3; MAX_CHUNK_SIZE])));
        round_trip_response(&Response::DirEntry(Some(DirEntry::new(
            "test.txt".try_into().unwrap(),
            crate::api::DirEntryType::File { size: 512 },
        ))));
        round_trip_response(&Response::DirEntry(None));
        round_trip_response(&Response::LedBufferInfo(LedBufferInformation::new(
            16384, 48,
        )));
        round_trip_response(&Response::ActiveEffect(Some(
            "/effects/demo.json".try_into().unwrap(),
        )));
        round_trip_response(&Response::ActiveEffect(None));
        for error in [
            ApiError::NotFound,
            ApiError::InvalidArgument,
            ApiError::OutOfBounds,
            ApiError::NoActiveEffect,
            ApiError::EffectFailure,
            ApiError::Storage,
            ApiError::Internal,
        ] {
            round_trip_response(&Response::Error(error));
        }
    }

    #[test]
    fn largest_request_fits_in_frame_buffer() {
        // Worst case request: a file write carrying a maximum size chunk.
        let request = Request::FileWrite {
            path: "x"
                .repeat(crate::api::MAX_PATH_LEN)
                .as_str()
                .try_into()
                .unwrap(),
            offset: u32::MAX,
            data: chunk(&[42; MAX_CHUNK_SIZE]),
        };

        let mut buffer = [0u8; 2048];
        let serialized = postcard::to_slice(&request, &mut buffer).unwrap();

        // teeny-rpc frames consist of a 6 byte header, the postcard encoded
        // payload, a 2 byte CRC, COBS overhead (up to one byte per 254 bytes of
        // data), and the trailing zero delimiter.
        let raw_frame_len = 6 + serialized.len() + 2;
        let encoded_frame_len = raw_frame_len + raw_frame_len.div_ceil(254) + 1;
        assert!(
            encoded_frame_len <= RPC_FRAME_BUF_LEN,
            "encoded frame length {encoded_frame_len} exceeds frame buffer of {RPC_FRAME_BUF_LEN}"
        );
    }
}
