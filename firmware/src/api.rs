//! Device API RPC server.
//!
//! Serves the RPC API described by [`bled_core::api::Request`] and
//! [`bled_core::api::Response`] over any `teeny-rpc` transport, such as the
//! emulated serial over BLE stream or USB CDC ACM transport.

use crate::{
    effect, leds,
    sdcard::{FsError, OpenOptions, SdCardStorage},
};
use bled_core::{
    PathString,
    api::{
        ApiError, Chunk, DirEntry, MAX_CHUNK_SIZE, RPC_ACK_TIMEOUT, RPC_CHANNEL_ID,
        RPC_FRAME_BUF_LEN, Request, Response,
    },
    config::MAX_CONFIG_CBOR_SIZE,
};
use core::time::Duration;
use defmt::{debug, warn};
use teeny_rpc::{Error as RpcError, Server, transport::Transport};

/// Maximum time to wait for the next RPC request before checking for errors.
const REQUEST_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs the RPC API server until the transport fails (e.g. the client
/// disconnects).
pub(crate) async fn serve<T: Transport>(transport: T, sd: SdCardStorage) {
    let mut server = Server::<T, Request, Response, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN>::new(
        transport,
        RPC_ACK_TIMEOUT,
    );

    loop {
        match server.wait_for_request(REQUEST_IDLE_TIMEOUT).await {
            Ok(request) => {
                debug!("RPC request: {}", request);
                let response = handle_request(&sd, request).await;
                debug!("RPC response: {}", response);

                match server.send_response(response).await {
                    Ok(()) => {}
                    Err(RpcError::TransportError) => break,
                    Err(error) => warn!("Failed to send RPC response: {}", error),
                }
            }
            Err(RpcError::TransportError) => break,
            Err(RpcError::Timeout) => {}
            Err(error) => warn!("RPC error: {}", error),
        }
    }
}

async fn handle_request(sd: &SdCardStorage, request: Request) -> Response {
    match request {
        Request::GetDeviceInfo => Response::DeviceInfo(crate::device_info()),
        Request::Reboot => {
            crate::status::reset();
            Response::Ok
        }
        Request::ConfigStat => match config_bytes() {
            Ok((_, config_length)) => Response::Size(config_length as u64),
            Err(error) => Response::Error(error),
        },
        Request::ConfigRead { offset, length } => match config_bytes() {
            Ok((bytes, config_length)) => {
                match read_window(Some(&bytes[..config_length]), offset, length) {
                    Ok(data) => Response::Data(data),
                    Err(error) => Response::Error(error),
                }
            }
            Err(error) => Response::Error(error),
        },
        Request::Commit => {
            leds::draw().await;
            Response::Ok
        }
        Request::FileStat { path } => match sd.stat_file(path.as_str()).await {
            Ok(size) => Response::Size(size),
            Err(error) => Response::Error(map_fs_error(error)),
        },
        Request::FileRead {
            path,
            offset,
            length,
        } => match file_read(sd, &path, offset, length).await {
            Ok(data) => Response::Data(data),
            Err(error) => Response::Error(error),
        },
        Request::FileWrite { path, offset, data } => {
            match file_write(sd, &path, offset, &data).await {
                Ok(()) => Response::Ok,
                Err(error) => Response::Error(error),
            }
        }
        Request::FileCreate { path, size } => {
            match sd.create_file(path.as_str(), size as u64).await {
                Ok(()) => Response::Ok,
                Err(error) => Response::Error(map_fs_error(error)),
            }
        }
        Request::FileDelete { path } => match sd.delete_file(path.as_str()).await {
            Ok(()) => Response::Ok,
            Err(error) => Response::Error(map_fs_error(error)),
        },
        Request::ReadDirEntry { path, index } => match read_dir_entry(sd, &path, index).await {
            Ok(entry) => Response::DirEntry(entry),
            Err(error) => Response::Error(error),
        },
        Request::LedBufferInfo => Response::LedBufferInfo(leds::data().await.info()),
        Request::LedBufferRead { offset, length } => match led_buffer_read(offset, length).await {
            Ok(data) => Response::Data(data),
            Err(error) => Response::Error(error),
        },
        Request::LedBufferWrite { offset, data } => match led_buffer_write(offset, &data).await {
            Ok(()) => Response::Ok,
            Err(error) => Response::Error(error),
        },
        Request::GetActiveEffect => Response::ActiveEffect(effect::active_effect_path().await),
        Request::SetActiveEffect { path } => match effect::activate_effect(sd, path).await {
            Ok(()) => Response::Ok,
            Err(()) => Response::Error(ApiError::EffectFailure),
        },
        Request::EffectManifestStat => {
            match effect::with_active_effect_manifest(|manifest| {
                manifest
                    .map(|(_, bytes)| bytes.len() as u64)
                    .ok_or(ApiError::NoActiveEffect)
            })
            .await
            {
                Ok(size) => Response::Size(size),
                Err(error) => Response::Error(error),
            }
        }
        Request::EffectManifestRead { offset, length } => {
            match effect::with_active_effect_manifest(|manifest| {
                read_window(manifest.map(|(_, bytes)| bytes), offset, length)
            })
            .await
            {
                Ok(data) => Response::Data(data),
                Err(error) => Response::Error(error),
            }
        }
        Request::EffectCommandStat => match effect::effect_command_size().await {
            Ok(size) => Response::Size(size as u64),
            Err(()) => Response::Error(effect_error().await),
        },
        Request::EffectCommandRead => {
            let mut buffer = [0u8; MAX_CHUNK_SIZE];
            match effect::read_effect_command(&mut buffer).await {
                Ok(length) => match Chunk::try_from(&buffer[..length]) {
                    Ok(data) => Response::Data(data),
                    Err(_) => Response::Error(ApiError::Internal),
                },
                Err(()) => Response::Error(effect_error().await),
            }
        }
        Request::EffectCommandWrite { data } => match effect::write_effect_command(&data).await {
            Ok(()) => Response::Ok,
            Err(()) => Response::Error(effect_error().await),
        },
        Request::EffectParameterStat => match effect::effect_parameter_size().await {
            Ok(size) => Response::Size(size as u64),
            Err(()) => Response::Error(effect_error().await),
        },
        Request::EffectParameterRead => {
            let mut buffer = [0u8; MAX_CHUNK_SIZE];
            match effect::read_effect_parameters(&mut buffer).await {
                Ok(length) => match Chunk::try_from(&buffer[..length]) {
                    Ok(data) => Response::Data(data),
                    Err(_) => Response::Error(ApiError::Internal),
                },
                Err(()) => Response::Error(effect_error().await),
            }
        }
        Request::EffectParameterWrite { data } => {
            match effect::write_effect_parameters(&data).await {
                Ok(()) => Response::Ok,
                Err(()) => Response::Error(effect_error().await),
            }
        }
    }
}

/// Encodes the active device configuration as CBOR.
fn config_bytes() -> Result<([u8; MAX_CONFIG_CBOR_SIZE], usize), ApiError> {
    let mut buffer = [0u8; MAX_CONFIG_CBOR_SIZE];
    let length = bled_core::io::encode_cbor(crate::config::get(), &mut buffer)
        .map_err(|_| ApiError::Internal)?;
    Ok((buffer, length))
}

/// Reads a window of up to `MAX_CHUNK_SIZE` bytes from optional source bytes.
fn read_window(bytes: Option<&[u8]>, offset: u32, length: u32) -> Result<Chunk, ApiError> {
    let length = length as usize;
    if length == 0 || length > MAX_CHUNK_SIZE {
        return Err(ApiError::InvalidArgument);
    }

    let bytes = bytes.ok_or(ApiError::NoActiveEffect)?;

    let start = offset as usize;
    if start > bytes.len() {
        return Err(ApiError::OutOfBounds);
    }

    let end = start.saturating_add(length).min(bytes.len());
    let mut chunk = Chunk::new();
    chunk
        .extend_from_slice(&bytes[start..end])
        .map_err(|_| ApiError::Internal)?;
    Ok(chunk)
}

async fn file_read(
    sd: &SdCardStorage,
    path: &PathString,
    offset: u32,
    length: u32,
) -> Result<Chunk, ApiError> {
    let length = length as usize;
    if length == 0 || length > MAX_CHUNK_SIZE {
        return Err(ApiError::InvalidArgument);
    }

    let mut file = sd
        .open_file(path.as_str(), OpenOptions::new().read(true))
        .await
        .map_err(map_fs_error)?;

    let result = async {
        sd.seek_file(&mut file, offset as u64)
            .await
            .map_err(map_fs_error)?;

        let mut chunk = Chunk::new();
        chunk.resize(length, 0).map_err(|_| ApiError::Internal)?;
        let bytes_read = sd
            .read_open_file(&mut file, &mut chunk)
            .await
            .map_err(map_fs_error)?;
        chunk.truncate(bytes_read);
        Ok(chunk)
    }
    .await;

    let _ = sd.close_file(file).await;
    result
}

async fn file_write(
    sd: &SdCardStorage,
    path: &PathString,
    offset: u32,
    data: &Chunk,
) -> Result<(), ApiError> {
    if data.is_empty() {
        return Ok(());
    }

    let mut file = sd
        .open_file(path.as_str(), OpenOptions::new().read(true).write(true))
        .await
        .map_err(map_fs_error)?;

    let result = async {
        sd.seek_file(&mut file, offset as u64)
            .await
            .map_err(map_fs_error)?;
        sd.write_open_file(&mut file, data)
            .await
            .map_err(map_fs_error)
    }
    .await;

    let _ = sd.close_file(file).await;
    result
}

async fn read_dir_entry(
    sd: &SdCardStorage,
    path: &PathString,
    index: u32,
) -> Result<Option<DirEntry>, ApiError> {
    let mut dir = sd.open_dir(path.as_str()).await.map_err(map_fs_error)?;

    let mut current_index = 0u32;
    loop {
        match sd.next_dir_entry(&mut dir).await.map_err(map_fs_error)? {
            Some(entry) if current_index == index => return Ok(Some(entry)),
            Some(_) => current_index += 1,
            None => return Ok(None),
        }
    }
}

async fn led_buffer_read(offset: u32, length: u32) -> Result<Chunk, ApiError> {
    let length = length as usize;
    if length == 0 || length > MAX_CHUNK_SIZE {
        return Err(ApiError::InvalidArgument);
    }

    let data = leds::data().await;
    let size = data.len();
    let start = (offset as usize).min(size);
    let end = start.saturating_add(length).min(size);

    let mut chunk = Chunk::new();
    chunk
        .extend_from_slice(&data.inner()[start..end])
        .map_err(|_| ApiError::Internal)?;
    Ok(chunk)
}

async fn led_buffer_write(offset: u32, data: &Chunk) -> Result<(), ApiError> {
    if data.is_empty() {
        return Ok(());
    }

    let mut buffer = leds::data().await;
    let start = offset as usize;
    let Some(end) = start.checked_add(data.len()) else {
        return Err(ApiError::OutOfBounds);
    };
    if end > buffer.len() {
        return Err(ApiError::OutOfBounds);
    }

    buffer.inner_mut()[start..end].copy_from_slice(data);
    Ok(())
}

/// Distinguishes a missing active effect from a failing one.
async fn effect_error() -> ApiError {
    if effect::active_effect_path().await.is_none() {
        ApiError::NoActiveEffect
    } else {
        ApiError::EffectFailure
    }
}

fn map_fs_error(error: FsError) -> ApiError {
    use exfat_slim::blocking::error::ExFatError;

    match error {
        ExFatError::FileNotFound | ExFatError::DirectoryNotFound => ApiError::NotFound,
        ExFatError::InvalidFileName { .. } => ApiError::InvalidArgument,
        _ => ApiError::Storage,
    }
}
