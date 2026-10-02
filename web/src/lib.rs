//! WebAssembly bindings for `bled-api`.
//!
//! Provides protocol encoding, decoding, and client framing for Web Bluetooth
//! and other JavaScript/TypeScript environments.

use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PixelRangeItem {
    pub start: u16,
    pub count: u16,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedRangesResult {
    pub channel: u8,
    pub ranges: Vec<PixelRangeItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BufferWriteChunk {
    pub offset: u32,
    #[serde(with = "serde_bytes")]
    pub control: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileWriteChunk {
    pub offset: u32,
    #[serde(with = "serde_bytes")]
    pub control: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// GATT UUID Constants
// ---------------------------------------------------------------------------

/// LED control GATT service UUID.
#[wasm_bindgen]
pub fn service_uuid() -> String {
    bled_api::ble::SERVICE_UUID.to_string()
}

/// Device Info GATT characteristic UUID.
#[wasm_bindgen]
pub fn device_info_uuid() -> String {
    bled_api::ble::DEVICE_INFO_UUID.to_string()
}

/// Reset device GATT characteristic UUID.
#[wasm_bindgen]
pub fn reset_uuid() -> String {
    bled_api::ble::RESET_UUID.to_string()
}

/// Config control GATT characteristic UUID.
#[wasm_bindgen]
pub fn config_control_uuid() -> String {
    bled_api::ble::CONFIG_CONTROL_UUID.to_string()
}

/// Config data GATT characteristic UUID.
#[wasm_bindgen]
pub fn config_data_uuid() -> String {
    bled_api::ble::CONFIG_DATA_UUID.to_string()
}

/// Render staged channel GATT characteristic UUID.
#[wasm_bindgen]
pub fn commit_uuid() -> String {
    bled_api::ble::COMMIT_UUID.to_string()
}

/// Packed pixel range GATT characteristic UUID.
#[wasm_bindgen]
pub fn led_range_uuid() -> String {
    bled_api::ble::LED_RANGE_UUID.to_string()
}

/// LED buffer control GATT characteristic UUID.
#[wasm_bindgen]
pub fn led_buffer_control_uuid() -> String {
    bled_api::ble::LED_BUFFER_CONTROL_UUID.to_string()
}

/// LED buffer data GATT characteristic UUID.
#[wasm_bindgen]
pub fn led_buffer_data_uuid() -> String {
    bled_api::ble::LED_BUFFER_DATA_UUID.to_string()
}

/// File IO control GATT characteristic UUID.
#[wasm_bindgen]
pub fn file_io_control_uuid() -> String {
    bled_api::ble::FILE_IO_CONTROL_UUID.to_string()
}

/// File IO data GATT characteristic UUID.
#[wasm_bindgen]
pub fn file_io_data_uuid() -> String {
    bled_api::ble::FILE_IO_DATA_UUID.to_string()
}

/// LED buffer data GATT characteristic UUID (backward compatibility alias).
#[wasm_bindgen]
pub fn led_data_uuid() -> String {
    bled_api::ble::LED_BUFFER_DATA_UUID.to_string()
}

/// Maximum number of LED channels addressable on one device.
#[wasm_bindgen]
pub fn max_channel_count() -> usize {
    bled_api::MAX_CHANNEL_COUNT
}

/// Maximum length of an attribute value in bytes.
#[wasm_bindgen]
pub fn max_attribute_value_len() -> usize {
    bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN
}

/// Maximum path length in bytes for File IO operations.
#[wasm_bindgen]
pub fn max_path_len() -> usize {
    bled_api::MAX_PATH_LEN
}

/// Git revision of the build.
#[wasm_bindgen]
pub fn git_revision() -> String {
    git_version::git_version!().to_string()
}

// ---------------------------------------------------------------------------
// Pure Rust Core Logic
// ---------------------------------------------------------------------------

pub fn internal_encode_commit(channels: &[u8]) -> Result<Vec<u8>, String> {
    if channels.is_empty() {
        return Err("at least one channel index is required".into());
    }
    let mask = bled_api::ChannelMask::from_indices(channels.iter().copied().map(usize::from))
        .ok_or_else(|| "channel index out of range (must be 0..7)".to_string())?;
    let mut buffer = [0u8; 16];
    let written = bled_api::io::encode_cbor(&mask, &mut buffer)
        .map_err(|e| format!("failed to encode Commit command: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_decode_commit(data: &[u8]) -> Result<Vec<u8>, String> {
    let mask: bled_api::ChannelMask = bled_api::io::decode_cbor(data)
        .map_err(|e| format!("failed to decode Commit mask: {e}"))?;
    Ok(mask.iter_indices().map(|i| i as u8).collect())
}

pub fn internal_encode_range(
    channel: u8,
    start: u16,
    count: u16,
    r: u8,
    g: u8,
    b: u8,
) -> Result<Vec<u8>, String> {
    let end = start
        .checked_add(count)
        .ok_or_else(|| "pixel range extends beyond u16 address space".to_string())?;
    let mut ranges = bled_api::Rgb8Ranges::<24>::default();
    bled_api::PixelRanges::insert(&mut ranges, start..end, bled_api::RGB8::new(r, g, b))
        .map_err(|e| format!("failed to insert range: {e}"))?;
    let channel_ranges =
        bled_api::ChannelRanges::<bled_api::RGB8, bled_api::Rgb8Ranges<24>>::new(channel, ranges)
            .map_err(|e| format!("failed to create channel ranges: {e}"))?;
    let mut buffer = [0u8; 128];
    let written = bled_api::io::encode_cbor(&channel_ranges, &mut buffer)
        .map_err(|e| format!("failed to encode range: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_encode_ranges(channel: u8, items: &[PixelRangeItem]) -> Result<Vec<u8>, String> {
    if items.len() > 24 {
        return Err("cannot encode more than 24 ranges in a single packet".to_string());
    }
    let mut ranges = bled_api::Rgb8Ranges::<24>::default();
    for item in items {
        let end = item
            .start
            .checked_add(item.count)
            .ok_or_else(|| "pixel range extends beyond u16 address space".to_string())?;
        bled_api::PixelRanges::insert(
            &mut ranges,
            item.start..end,
            bled_api::RGB8::new(item.r, item.g, item.b),
        )
        .map_err(|e| format!("failed to insert range: {e}"))?;
    }
    let channel_ranges =
        bled_api::ChannelRanges::<bled_api::RGB8, bled_api::Rgb8Ranges<24>>::new(channel, ranges)
            .map_err(|e| format!("failed to create channel ranges: {e}"))?;
    let mut buffer = [0u8; 512];
    let written = bled_api::io::encode_cbor(&channel_ranges, &mut buffer)
        .map_err(|e| format!("failed to encode ranges: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_decode_ranges(data: &[u8]) -> Result<(u8, Vec<PixelRangeItem>), String> {
    type DecodedRange = bled_api::ChannelRanges<bled_api::RGB8, bled_api::Rgb8Ranges<24>>;
    let decoded: DecodedRange =
        bled_api::io::decode_cbor(data).map_err(|e| format!("failed to decode ranges: {e}"))?;
    let channel = *decoded.channel();
    let mut items = Vec::new();
    for (range, colour) in bled_api::PixelRanges::iter(decoded.ranges()) {
        let start = u16::try_from(range.start).map_err(|_| "start exceeds u16".to_string())?;
        let count = u16::try_from(range.len()).map_err(|_| "count exceeds u16".to_string())?;
        items.push(PixelRangeItem {
            start,
            count,
            r: colour.r,
            g: colour.g,
            b: colour.b,
        });
    }
    Ok((channel, items))
}

pub fn internal_encode_led_buffer_control(
    cmd: &bled_api::LedBufferControl,
) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_api::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode LED buffer control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_pixels_per_write(payload_budget: usize) -> Result<usize, String> {
    let budget = payload_budget.min(bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN);
    let count = budget / 3;
    if count == 0 {
        return Err("ATT payload budget is too small for an RGB pixel".into());
    }
    Ok(count)
}

pub fn internal_slice_led_buffer_write(
    start_offset: u32,
    data: &[u8],
    payload_budget: usize,
) -> Result<Vec<BufferWriteChunk>, String> {
    let chunk_size = payload_budget.min(bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN);
    if chunk_size == 0 {
        return Err("payload budget must be greater than 0".into());
    }
    let mut chunks = Vec::new();
    let mut offset = start_offset;
    for chunk in data.chunks(chunk_size) {
        let control =
            internal_encode_led_buffer_control(&bled_api::LedBufferControl::Write { offset })?;
        chunks.push(BufferWriteChunk {
            offset,
            control,
            data: chunk.to_vec(),
        });
        offset = offset
            .checked_add(chunk.len() as u32)
            .ok_or_else(|| "offset overflow".to_string())?;
    }
    Ok(chunks)
}

fn to_heapless_path(path: &str) -> Result<heapless::String<{ bled_api::MAX_PATH_LEN }>, String> {
    if path.is_empty() {
        return Err("remote path cannot be empty".to_string());
    }
    path.try_into().map_err(|_| {
        format!(
            "remote path exceeds maximum length of {} bytes",
            bled_api::MAX_PATH_LEN
        )
    })
}

pub fn internal_encode_file_io_control(cmd: &bled_api::FileIoControl) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_api::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode File IO control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_encode_file_io_stat(path: &str) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_api::FileIoControl::Stat {
        path: heapless_path,
    })
}

pub fn internal_encode_file_io_read(path: &str, offset: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_api::FileIoControl::Read {
        path: heapless_path,
        offset,
    })
}

pub fn internal_encode_file_io_create(path: &str, size: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_api::FileIoControl::Create {
        path: heapless_path,
        size,
    })
}

pub fn internal_encode_file_io_write(path: &str, offset: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_api::FileIoControl::Write {
        path: heapless_path,
        offset,
    })
}

pub fn internal_encode_file_io_delete(path: &str) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_api::FileIoControl::Delete {
        path: heapless_path,
    })
}

pub fn internal_encode_file_io_reset() -> Result<Vec<u8>, String> {
    internal_encode_file_io_control(&bled_api::FileIoControl::Reset)
}

pub fn internal_decode_file_stat(data: &[u8]) -> Result<bled_api::FileStatResponse, String> {
    bled_api::io::decode_cbor(data).map_err(|e| format!("failed to decode FileStatResponse: {e}"))
}

pub fn internal_encode_config_control(cmd: &bled_api::ConfigControl) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_api::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode Config control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

pub fn internal_encode_config_stat() -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Stat)
}

pub fn internal_encode_config_read(offset: u32) -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Read { offset })
}

pub fn internal_encode_config_reset() -> Result<Vec<u8>, String> {
    internal_encode_config_control(&bled_api::ConfigControl::Reset)
}

pub fn internal_decode_config_stat(data: &[u8]) -> Result<bled_api::ConfigStatResponse, String> {
    bled_api::io::decode_cbor(data).map_err(|e| format!("failed to decode ConfigStatResponse: {e}"))
}

pub fn internal_slice_file_write(
    path: &str,
    data: &[u8],
    payload_budget: usize,
) -> Result<Vec<FileWriteChunk>, String> {
    let chunk_size = payload_budget.min(bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN);
    if chunk_size == 0 {
        return Err("payload budget must be greater than 0".into());
    }
    let heapless_path = to_heapless_path(path)?;
    let mut chunks = Vec::new();
    let mut offset: u32 = 0;
    for chunk in data.chunks(chunk_size) {
        let control = internal_encode_file_io_control(&bled_api::FileIoControl::Write {
            path: heapless_path.clone(),
            offset,
        })?;
        chunks.push(FileWriteChunk {
            offset,
            control,
            data: chunk.to_vec(),
        });
        offset = offset
            .checked_add(chunk.len() as u32)
            .ok_or_else(|| "offset overflow".to_string())?;
    }
    Ok(chunks)
}

// ---------------------------------------------------------------------------
// wasm-bindgen Exports
// ---------------------------------------------------------------------------

/// Decodes the CBOR payload read from the Device Info characteristic into a JS object.
#[wasm_bindgen]
pub fn decode_device_info(data: &[u8]) -> Result<JsValue, JsError> {
    let info: bled_api::DeviceInfo = bled_api::io::decode_cbor(data)
        .map_err(|e| JsError::new(&format!("failed to decode DeviceInfo: {e}")))?;
    serde_wasm_bindgen::to_value(&info)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceInfo: {e}")))
}

/// Decodes the payload for DeviceConfig (CBOR or JSON) into a JS object.
#[wasm_bindgen]
pub fn decode_device_config(data: &[u8]) -> Result<JsValue, JsError> {
    let config: bled_api::DeviceConfig = match bled_api::io::decode_cbor(data) {
        Ok(c) => c,
        Err(_) => bled_api::io::decode_json(data)
            .map_err(|e| JsError::new(&format!("failed to decode DeviceConfig: {e}")))?,
    };
    serde_wasm_bindgen::to_value(&config)
        .map_err(|e| JsError::new(&format!("failed to serialize DeviceConfig: {e}")))
}

/// Encodes an array of channel indices (0..7) into a Commit command CBOR payload.
#[wasm_bindgen]
pub fn encode_commit(channels: &[u8]) -> Result<Vec<u8>, JsError> {
    internal_encode_commit(channels).map_err(|e| JsError::new(&e))
}

/// Decodes a Commit command CBOR payload into an array of selected channel indices.
#[wasm_bindgen]
pub fn decode_commit(data: &[u8]) -> Result<Vec<u8>, JsError> {
    internal_decode_commit(data).map_err(|e| JsError::new(&e))
}

/// Encodes a single solid-colour pixel range update into CBOR for LED_RANGE_UUID.
#[wasm_bindgen]
pub fn encode_range(
    channel: u8,
    start: u16,
    count: u16,
    r: u8,
    g: u8,
    b: u8,
) -> Result<Vec<u8>, JsError> {
    internal_encode_range(channel, start, count, r, g, b).map_err(|e| JsError::new(&e))
}

/// Encodes multiple solid-colour pixel ranges into CBOR for LED_RANGE_UUID.
/// Expects an array of `{ start: number, count: number, r: number, g: number, b: number }`.
#[wasm_bindgen]
pub fn encode_ranges(channel: u8, ranges_val: JsValue) -> Result<Vec<u8>, JsError> {
    let items: Vec<PixelRangeItem> = serde_wasm_bindgen::from_value(ranges_val)
        .map_err(|e| JsError::new(&format!("invalid ranges: {e}")))?;
    internal_encode_ranges(channel, &items).map_err(|e| JsError::new(&e))
}

/// Decodes a CBOR payload from LED_RANGE_UUID into `{ channel: number, ranges: [...] }`.
#[wasm_bindgen]
pub fn decode_ranges(data: &[u8]) -> Result<JsValue, JsError> {
    let (channel, items) = internal_decode_ranges(data).map_err(|e| JsError::new(&e))?;
    let result = DecodedRangesResult {
        channel,
        ranges: items,
    };
    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsError::new(&format!("failed to serialize decoded ranges: {e}")))
}

/// Encodes a `LedBufferControl::Info` request into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_info() -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_api::LedBufferControl::Info)
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Read { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_read(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_api::LedBufferControl::Read { offset })
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Write { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_write(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_api::LedBufferControl::Write { offset })
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Reset` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_api::LedBufferControl::Reset)
        .map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload for `LedBufferInformation` into `{ capacity: number, size: number }`.
#[wasm_bindgen]
pub fn decode_led_buffer_info(data: &[u8]) -> Result<JsValue, JsError> {
    let info: bled_api::LedBufferInformation = bled_api::io::decode_cbor(data)
        .map_err(|e| JsError::new(&format!("failed to decode LedBufferInformation: {e}")))?;
    serde_wasm_bindgen::to_value(&info)
        .map_err(|e| JsError::new(&format!("failed to serialize LedBufferInformation: {e}")))
}

/// Slices LED buffer write data into chunks fitting `payload_budget`, returning an array of
/// `{ offset: number, control: Uint8Array, data: Uint8Array }`.
#[wasm_bindgen]
pub fn slice_led_buffer_write(
    start_offset: u32,
    data: &[u8],
    payload_budget: usize,
) -> Result<JsValue, JsError> {
    let chunks = internal_slice_led_buffer_write(start_offset, data, payload_budget)
        .map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&chunks)
        .map_err(|e| JsError::new(&format!("failed to serialize buffer write chunks: {e}")))
}

/// Maximum number of RGB pixels that fit into a single write given `payload_budget` (ATT MTU - 3).
#[wasm_bindgen]
pub fn pixels_per_write(payload_budget: usize) -> Result<usize, JsError> {
    internal_pixels_per_write(payload_budget).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Stat` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_stat(path: &str) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_stat(path).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Read { path, offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_read(path: &str, offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_read(path, offset).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Create { path, size }` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_create(path: &str, size: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_create(path, size).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Write { path, offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_write(path: &str, offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_write(path, offset).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Delete { path }` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_delete(path: &str) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_delete(path).map_err(|e| JsError::new(&e))
}

/// Encodes a `FileIoControl::Reset` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_reset().map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload from `FILE_IO_DATA_UUID` for a stat request into `{ size: number }`.
#[wasm_bindgen]
pub fn decode_file_stat(data: &[u8]) -> Result<JsValue, JsError> {
    let stat = internal_decode_file_stat(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&stat)
        .map_err(|e| JsError::new(&format!("failed to serialize FileStatResponse: {e}")))
}

/// Slices file write data into chunks fitting `payload_budget`, returning an array of
/// `{ offset: number, control: Uint8Array, data: Uint8Array }`.
#[wasm_bindgen]
pub fn slice_file_write(
    path: &str,
    data: &[u8],
    payload_budget: usize,
) -> Result<JsValue, JsError> {
    let chunks =
        internal_slice_file_write(path, data, payload_budget).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&chunks)
        .map_err(|e| JsError::new(&format!("failed to serialize file write chunks: {e}")))
}

/// Encodes a `ConfigControl::Stat` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_stat() -> Result<Vec<u8>, JsError> {
    internal_encode_config_stat().map_err(|e| JsError::new(&e))
}

/// Encodes a `ConfigControl::Read { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_read(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_config_read(offset).map_err(|e| JsError::new(&e))
}

/// Encodes a `ConfigControl::Reset` command into CBOR.
#[wasm_bindgen]
pub fn encode_config_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_config_reset().map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload from `CONFIG_DATA_UUID` for a stat request into `{ size: number }`.
#[wasm_bindgen]
pub fn decode_config_stat(data: &[u8]) -> Result<JsValue, JsError> {
    let stat = internal_decode_config_stat(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&stat)
        .map_err(|e| JsError::new(&format!("failed to serialize ConfigStatResponse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_api::PixelRanges;

    #[test]
    fn uuid_strings_match_api() {
        assert_eq!(service_uuid(), bled_api::ble::SERVICE_UUID.to_string());
        assert_eq!(
            device_info_uuid(),
            bled_api::ble::DEVICE_INFO_UUID.to_string()
        );
        assert_eq!(reset_uuid(), bled_api::ble::RESET_UUID.to_string());
        assert_eq!(
            config_control_uuid(),
            bled_api::ble::CONFIG_CONTROL_UUID.to_string()
        );
        assert_eq!(
            config_data_uuid(),
            bled_api::ble::CONFIG_DATA_UUID.to_string()
        );
        assert_eq!(commit_uuid(), bled_api::ble::COMMIT_UUID.to_string());
        assert_eq!(led_range_uuid(), bled_api::ble::LED_RANGE_UUID.to_string());
        assert_eq!(
            led_buffer_control_uuid(),
            bled_api::ble::LED_BUFFER_CONTROL_UUID.to_string()
        );
        assert_eq!(
            led_buffer_data_uuid(),
            bled_api::ble::LED_BUFFER_DATA_UUID.to_string()
        );
        assert_eq!(
            file_io_control_uuid(),
            bled_api::ble::FILE_IO_CONTROL_UUID.to_string()
        );
        assert_eq!(
            file_io_data_uuid(),
            bled_api::ble::FILE_IO_DATA_UUID.to_string()
        );
        assert_eq!(
            led_data_uuid(),
            bled_api::ble::LED_BUFFER_DATA_UUID.to_string()
        );
        assert_eq!(max_channel_count(), bled_api::MAX_CHANNEL_COUNT);
        assert_eq!(
            max_attribute_value_len(),
            bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN
        );
        assert_eq!(max_path_len(), bled_api::MAX_PATH_LEN);
        assert!(!git_revision().is_empty());
    }

    #[test]
    fn test_encode_commit_round_trip() {
        let channels = [0u8, 2, 5];
        let bytes = internal_encode_commit(&channels).unwrap();

        let decoded = internal_decode_commit(&bytes).unwrap();
        assert_eq!(decoded, vec![0, 2, 5]);

        let mask: bled_api::ChannelMask = bled_api::io::decode_cbor(&bytes).unwrap();
        assert!(mask.contains_index(0));
        assert!(mask.contains_index(2));
        assert!(mask.contains_index(5));
        assert!(!mask.contains_index(1));
    }

    #[test]
    fn test_encode_commit_errors() {
        assert!(internal_encode_commit(&[]).is_err());
        assert!(internal_encode_commit(&[8]).is_err());
    }

    #[test]
    fn test_encode_range_round_trip() {
        let bytes = internal_encode_range(3, 10, 5, 255, 128, 0).unwrap();

        type DecodedRange = bled_api::ChannelRanges<bled_api::RGB8, bled_api::Rgb8Ranges<24>>;
        let decoded: DecodedRange = bled_api::io::decode_cbor(&bytes).unwrap();
        assert_eq!(*decoded.channel(), 3);
        assert_eq!(decoded.ranges().len(), 1);
        let items: Vec<_> = bled_api::PixelRanges::iter(decoded.ranges()).collect();
        assert_eq!(items, vec![(10..15, bled_api::RGB8::new(255, 128, 0))]);

        let (ch, decoded_items) = internal_decode_ranges(&bytes).unwrap();
        assert_eq!(ch, 3);
        assert_eq!(
            decoded_items,
            vec![PixelRangeItem {
                start: 10,
                count: 5,
                r: 255,
                g: 128,
                b: 0,
            }]
        );
    }

    #[test]
    fn test_encode_ranges() {
        let items = [
            PixelRangeItem {
                start: 0,
                count: 10,
                r: 255,
                g: 0,
                b: 0,
            },
            PixelRangeItem {
                start: 10,
                count: 5,
                r: 0,
                g: 255,
                b: 0,
            },
        ];
        let bytes = internal_encode_ranges(0, &items).unwrap();

        type DecodedRange = bled_api::ChannelRanges<bled_api::RGB8, bled_api::Rgb8Ranges<24>>;
        let decoded: DecodedRange = bled_api::io::decode_cbor(&bytes).unwrap();
        assert_eq!(*decoded.channel(), 0);
        assert_eq!(decoded.ranges().len(), 2);

        let (channel, decoded_items) = internal_decode_ranges(&bytes).unwrap();
        assert_eq!(channel, 0);
        assert_eq!(decoded_items, items.to_vec());
    }

    #[test]
    fn test_pixels_per_write() {
        assert!(internal_pixels_per_write(2).is_err());
        assert_eq!(internal_pixels_per_write(3).unwrap(), 1);
        assert_eq!(internal_pixels_per_write(102).unwrap(), 34);
        assert_eq!(
            internal_pixels_per_write(1000).unwrap(),
            bled_api::ble::MAX_ATTRIBUTE_VALUE_LEN / 3
        );
    }

    #[test]
    fn test_led_buffer_control() {
        let info_bytes =
            internal_encode_led_buffer_control(&bled_api::LedBufferControl::Info).unwrap();
        let decoded_info: bled_api::LedBufferControl =
            bled_api::io::decode_cbor(&info_bytes).unwrap();
        assert_eq!(decoded_info, bled_api::LedBufferControl::Info);

        let write_bytes =
            internal_encode_led_buffer_control(&bled_api::LedBufferControl::Write { offset: 42 })
                .unwrap();
        let decoded_write: bled_api::LedBufferControl =
            bled_api::io::decode_cbor(&write_bytes).unwrap();
        assert_eq!(
            decoded_write,
            bled_api::LedBufferControl::Write { offset: 42 }
        );

        let reset_bytes =
            internal_encode_led_buffer_control(&bled_api::LedBufferControl::Reset).unwrap();
        let decoded_reset: bled_api::LedBufferControl =
            bled_api::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded_reset, bled_api::LedBufferControl::Reset);
    }

    #[test]
    fn test_slice_led_buffer_write() {
        let raw_data = vec![123u8; 300];
        let chunks = internal_slice_led_buffer_write(100, &raw_data, 100).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].offset, 100);
        assert_eq!(chunks[0].data.len(), 100);
        assert_eq!(chunks[1].offset, 200);
        assert_eq!(chunks[1].data.len(), 100);
        assert_eq!(chunks[2].offset, 300);
        assert_eq!(chunks[2].data.len(), 100);

        let decoded: bled_api::LedBufferControl =
            bled_api::io::decode_cbor(&chunks[0].control).unwrap();
        assert_eq!(decoded, bled_api::LedBufferControl::Write { offset: 100 });
    }

    #[test]
    fn test_file_io_control_round_trip() {
        // Stat
        let stat_bytes = internal_encode_file_io_stat("/config.json").unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&stat_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Stat {
                path: "/config.json".try_into().unwrap()
            }
        );

        // Read
        let read_bytes = internal_encode_file_io_read("/test.txt", 128).unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&read_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Read {
                path: "/test.txt".try_into().unwrap(),
                offset: 128,
            }
        );

        // Create
        let create_bytes = internal_encode_file_io_create("/new.bin", 1024).unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&create_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Create {
                path: "/new.bin".try_into().unwrap(),
                size: 1024,
            }
        );

        // Write
        let write_bytes = internal_encode_file_io_write("/write.bin", 64).unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&write_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Write {
                path: "/write.bin".try_into().unwrap(),
                offset: 64,
            }
        );

        // Delete
        let delete_bytes = internal_encode_file_io_delete("/del.bin").unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&delete_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Delete {
                path: "/del.bin".try_into().unwrap()
            }
        );

        // Reset
        let reset_bytes = internal_encode_file_io_reset().unwrap();
        let decoded: bled_api::FileIoControl = bled_api::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded, bled_api::FileIoControl::Reset);
    }

    #[test]
    fn test_file_io_path_validation() {
        assert!(internal_encode_file_io_stat("").is_err());
        let long_path = "a".repeat(bled_api::MAX_PATH_LEN + 1);
        assert!(internal_encode_file_io_stat(&long_path).is_err());
        let exact_path = "a".repeat(bled_api::MAX_PATH_LEN);
        assert!(internal_encode_file_io_stat(&exact_path).is_ok());
    }

    #[test]
    fn test_decode_file_stat() {
        let stat = bled_api::FileStatResponse::new(4096);
        let mut buffer = [0u8; 64];
        let written = bled_api::io::encode_cbor(&stat, &mut buffer).unwrap();
        let decoded = internal_decode_file_stat(&buffer[..written]).unwrap();
        assert_eq!(*decoded.size(), 4096);
    }

    #[test]
    fn test_slice_file_write() {
        let raw_data = vec![42u8; 250];
        let chunks = internal_slice_file_write("/out.bin", &raw_data, 100).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].offset, 0);
        assert_eq!(chunks[0].data.len(), 100);
        assert_eq!(chunks[1].offset, 100);
        assert_eq!(chunks[1].data.len(), 100);
        assert_eq!(chunks[2].offset, 200);
        assert_eq!(chunks[2].data.len(), 50);

        let decoded: bled_api::FileIoControl =
            bled_api::io::decode_cbor(&chunks[1].control).unwrap();
        assert_eq!(
            decoded,
            bled_api::FileIoControl::Write {
                path: "/out.bin".try_into().unwrap(),
                offset: 100,
            }
        );
    }

    #[test]
    fn test_config_control() {
        // Stat
        let stat_bytes = internal_encode_config_stat().unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&stat_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Stat);

        // Read
        let read_bytes = internal_encode_config_read(128).unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&read_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Read { offset: 128 });

        // Reset
        let reset_bytes = internal_encode_config_reset().unwrap();
        let decoded: bled_api::ConfigControl = bled_api::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded, bled_api::ConfigControl::Reset);

        // Decode Stat
        let stat_response = bled_api::ConfigStatResponse::new(512);
        let mut buffer = [0u8; 64];
        let written = bled_api::io::encode_cbor(&stat_response, &mut buffer).unwrap();
        let decoded_stat = internal_decode_config_stat(&buffer[..written]).unwrap();
        assert_eq!(*decoded_stat.size(), 512);
    }
}
