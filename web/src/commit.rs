//! Channel mask commit command encoding and decoding.

use wasm_bindgen::prelude::*;

/// Pure Rust encoding of channel indices (0..7) into a Commit command CBOR payload.
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

/// Pure Rust decoding of a Commit command CBOR payload into an array of selected channel indices.
pub fn internal_decode_commit(data: &[u8]) -> Result<Vec<u8>, String> {
    let mask: bled_api::ChannelMask = bled_api::io::decode_cbor(data)
        .map_err(|e| format!("failed to decode Commit mask: {e}"))?;
    Ok(mask.iter_indices().map(|i| i as u8).collect())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
