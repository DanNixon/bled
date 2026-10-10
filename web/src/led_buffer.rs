//! LED buffer streaming protocol, chunk slicing, and control encoding.

use wasm_bindgen::prelude::*;

/// A sliced chunk of LED buffer write data and its associated control packet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BufferWriteChunk {
    pub offset: u32,
    #[serde(with = "serde_bytes")]
    pub control: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Pure Rust encoding of a `LedBufferControl` command into CBOR.
pub fn internal_encode_led_buffer_control(
    cmd: &bled_core::api::LedBufferControl,
) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_core::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode LED buffer control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Calculates the maximum number of RGB pixels that fit into a single write given `payload_budget`.
pub fn internal_pixels_per_write(payload_budget: usize) -> Result<usize, String> {
    let budget = payload_budget.min(bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN);
    let count = budget / 3;
    if count == 0 {
        return Err("ATT payload budget is too small for an RGB pixel".into());
    }
    Ok(count)
}

/// Slices raw buffer data into chunks fitting `payload_budget` with corresponding Write control packets.
pub fn internal_slice_led_buffer_write(
    start_offset: u32,
    data: &[u8],
    payload_budget: usize,
) -> Result<Vec<BufferWriteChunk>, String> {
    let chunk_size = payload_budget.min(bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN);
    if chunk_size == 0 {
        return Err("payload budget must be greater than 0".into());
    }
    let mut chunks = Vec::new();
    let mut offset = start_offset;
    for chunk in data.chunks(chunk_size) {
        let control = internal_encode_led_buffer_control(
            &bled_core::api::LedBufferControl::Write { offset },
        )?;
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

/// Encodes a `LedBufferControl::Info` request into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_info() -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Info)
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Read { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_read(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Read { offset })
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Write { offset }` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_write(offset: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Write { offset })
        .map_err(|e| JsError::new(&e))
}

/// Encodes a `LedBufferControl::Reset` command into CBOR.
#[wasm_bindgen]
pub fn encode_led_buffer_reset() -> Result<Vec<u8>, JsError> {
    internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Reset)
        .map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload for `LedBufferInformation` into `{ capacity: number, size: number }`.
#[wasm_bindgen]
pub fn decode_led_buffer_info(data: &[u8]) -> Result<JsValue, JsError> {
    let info: bled_core::api::LedBufferInformation = bled_core::io::decode_cbor(data)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixels_per_write() {
        assert!(internal_pixels_per_write(2).is_err());
        assert_eq!(internal_pixels_per_write(3).unwrap(), 1);
        assert_eq!(internal_pixels_per_write(102).unwrap(), 34);
        assert_eq!(
            internal_pixels_per_write(1000).unwrap(),
            bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN / 3
        );
    }

    #[test]
    fn test_led_buffer_control() {
        let info_bytes =
            internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Info).unwrap();
        let decoded_info: bled_core::api::LedBufferControl =
            bled_core::io::decode_cbor(&info_bytes).unwrap();
        assert_eq!(decoded_info, bled_core::api::LedBufferControl::Info);

        let write_bytes = internal_encode_led_buffer_control(
            &bled_core::api::LedBufferControl::Write { offset: 42 },
        )
        .unwrap();
        let decoded_write: bled_core::api::LedBufferControl =
            bled_core::io::decode_cbor(&write_bytes).unwrap();
        assert_eq!(
            decoded_write,
            bled_core::api::LedBufferControl::Write { offset: 42 }
        );

        let reset_bytes =
            internal_encode_led_buffer_control(&bled_core::api::LedBufferControl::Reset).unwrap();
        let decoded_reset: bled_core::api::LedBufferControl =
            bled_core::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded_reset, bled_core::api::LedBufferControl::Reset);
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

        let decoded: bled_core::api::LedBufferControl =
            bled_core::io::decode_cbor(&chunks[0].control).unwrap();
        assert_eq!(
            decoded,
            bled_core::api::LedBufferControl::Write { offset: 100 }
        );
    }
}
