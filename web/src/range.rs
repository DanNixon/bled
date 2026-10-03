//! Packed pixel range encoding and decoding.

use wasm_bindgen::prelude::*;

/// Maximum number of ranges that fit into a single packet.
pub const MAX_RANGES_PER_PACKET: usize = 24;

/// Type alias for channel pixel ranges with 24 RGB8 ranges.
pub type ChannelRgb8Ranges =
    bled_api::ChannelRanges<bled_api::RGB8, bled_api::Rgb8Ranges<MAX_RANGES_PER_PACKET>>;

/// A single contiguous range of pixels assigned a solid RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PixelRangeItem {
    pub start: u16,
    pub count: u16,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Decoded result containing the target channel index and decoded pixel ranges.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedRangesResult {
    pub channel: u8,
    pub ranges: Vec<PixelRangeItem>,
}

/// Pure Rust encoding of a single solid-colour pixel range into CBOR.
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
    let mut ranges = bled_api::Rgb8Ranges::<MAX_RANGES_PER_PACKET>::default();
    bled_api::PixelRanges::insert(&mut ranges, start..end, bled_api::RGB8::new(r, g, b))
        .map_err(|e| format!("failed to insert range: {e}"))?;
    let channel_ranges = ChannelRgb8Ranges::new(channel, ranges)
        .map_err(|e| format!("failed to create channel ranges: {e}"))?;
    let mut buffer = [0u8; 128];
    let written = bled_api::io::encode_cbor(&channel_ranges, &mut buffer)
        .map_err(|e| format!("failed to encode range: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Pure Rust encoding of multiple solid-colour pixel ranges into CBOR.
pub fn internal_encode_ranges(channel: u8, items: &[PixelRangeItem]) -> Result<Vec<u8>, String> {
    if items.len() > MAX_RANGES_PER_PACKET {
        return Err(format!(
            "cannot encode more than {MAX_RANGES_PER_PACKET} ranges in a single packet"
        ));
    }
    let mut ranges = bled_api::Rgb8Ranges::<MAX_RANGES_PER_PACKET>::default();
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
    let channel_ranges = ChannelRgb8Ranges::new(channel, ranges)
        .map_err(|e| format!("failed to create channel ranges: {e}"))?;
    let mut buffer = [0u8; 512];
    let written = bled_api::io::encode_cbor(&channel_ranges, &mut buffer)
        .map_err(|e| format!("failed to encode ranges: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Pure Rust decoding of a CBOR payload into channel and pixel range items.
pub fn internal_decode_ranges(data: &[u8]) -> Result<(u8, Vec<PixelRangeItem>), String> {
    let decoded: ChannelRgb8Ranges =
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

#[cfg(test)]
mod tests {
    use super::*;
    use bled_api::PixelRanges;

    #[test]
    fn test_encode_range_round_trip() {
        let bytes = internal_encode_range(3, 10, 5, 255, 128, 0).unwrap();

        let decoded: ChannelRgb8Ranges = bled_api::io::decode_cbor(&bytes).unwrap();
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

        let decoded: ChannelRgb8Ranges = bled_api::io::decode_cbor(&bytes).unwrap();
        assert_eq!(*decoded.channel(), 0);
        assert_eq!(decoded.ranges().len(), 2);

        let (channel, decoded_items) = internal_decode_ranges(&bytes).unwrap();
        assert_eq!(channel, 0);
        assert_eq!(decoded_items, items.to_vec());
    }
}
