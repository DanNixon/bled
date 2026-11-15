//! Fixture configuration decoder.

use wasm_bindgen::prelude::*;

/// Pure Rust decoding of `Fixtures` from Postcard or JSON.
pub fn internal_decode_fixtures(data: &[u8]) -> Result<bled_core::config::Fixtures, String> {
    match bled_core::io::decode_postcard(data) {
        Ok(c) => Ok(c),
        Err(_) => {
            bled_core::io::decode_json(data).map_err(|e| format!("failed to decode Fixtures: {e}"))
        }
    }
}

/// Decodes the Postcard payload returned by the fixtures RPC into a JS object.
#[wasm_bindgen]
pub fn decode_fixtures(data: &[u8]) -> Result<JsValue, JsError> {
    let fixtures = internal_decode_fixtures(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&fixtures)
        .map_err(|e| JsError::new(&format!("failed to serialize Fixtures: {e}")))
}

/// Pure Rust decoding of `EffectPlaylist` from Postcard or JSON.
pub fn internal_decode_playlist(data: &[u8]) -> Result<bled_core::effect::EffectPlaylist, String> {
    match bled_core::io::decode_postcard(data) {
        Ok(p) => Ok(p),
        Err(_) => bled_core::io::decode_json(data)
            .map_err(|e| format!("failed to decode EffectPlaylist: {e}")),
    }
}

/// Decodes the Postcard payload returned by the playlist RPC into a JS object.
#[wasm_bindgen]
pub fn decode_playlist(data: &[u8]) -> Result<JsValue, JsError> {
    let playlist = internal_decode_playlist(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&playlist)
        .map_err(|e| JsError::new(&format!("failed to serialize EffectPlaylist: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_core::{
        ChannelNumber, RGB8,
        config::{
            BrightnessLimit, BrightnessLimitMode, Fixture, Fixtures, LinearArray, PixelLayout,
            PixelSpan,
        },
    };

    #[test]
    fn test_fixtures_json_decoding() {
        let json = r#"[
            {
                "name": "onboard",
                "layout": { "linear_array": {} },
                "spans": [
                    { "name": "a", "channel": 0, "start": 0, "length": 1 },
                    {
                        "name": "b",
                        "channel": 1,
                        "start": 0,
                        "length": 1,
                        "limit": {
                            "mode": "rescale",
                            "limit": { "r": 63, "g": 127, "b": 255 }
                        }
                    }
                ]
            }
        ]"#;

        let decoded = internal_decode_fixtures(json.as_bytes()).unwrap();
        assert_eq!(decoded.as_slice().len(), 1);
        assert_eq!(decoded.as_slice()[0].name().as_str(), "onboard");
        assert_eq!(decoded.as_slice()[0].spans().len(), 2);
        assert_eq!(
            *decoded.as_slice()[0].spans()[0].brightness_limit(),
            BrightnessLimit::default()
        );
        assert_eq!(
            *decoded.as_slice()[0].spans()[1].brightness_limit(),
            BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(63, 127, 255))
        );
    }

    #[test]
    fn test_fixtures_postcard_decoding() {
        let fixture = Fixture::new(
            "test-fixture".try_into().unwrap(),
            PixelLayout::LinearArray(LinearArray::default()),
            [PixelSpan::new(
                "a".try_into().unwrap(),
                ChannelNumber::new(0),
                0,
                4,
                BrightnessLimit::default(),
            )]
            .try_into()
            .unwrap(),
        );
        let fixtures = Fixtures::new([fixture].into());

        let mut buffer = [0u8; 512];
        let len = bled_core::io::encode_postcard(&fixtures, &mut buffer).unwrap();

        let decoded = internal_decode_fixtures(&buffer[..len]).unwrap();
        assert_eq!(decoded, fixtures);
    }

    #[test]
    fn test_playlist_decoding() {
        use bled_core::effect::{EffectPlaylist, EffectPlaylistEntry, NextPlaylistEntryAction};
        use core::time::Duration;

        let json = r#"[
            { "path": "/effects/a.wasm", "duration": 15000 },
            { "path": "/effects/b.json", "after": "random" }
        ]"#;

        let decoded_json = internal_decode_playlist(json.as_bytes()).unwrap();
        assert_eq!(decoded_json.len(), 2);
        assert_eq!(decoded_json[0].path().as_str(), "/effects/a.wasm");
        assert_eq!(
            *decoded_json[0].duration(),
            Some(Duration::from_millis(15000))
        );
        assert_eq!(*decoded_json[0].after(), NextPlaylistEntryAction::Shift(1));
        assert_eq!(decoded_json[1].path().as_str(), "/effects/b.json");
        assert_eq!(*decoded_json[1].duration(), None);
        assert_eq!(*decoded_json[1].after(), NextPlaylistEntryAction::Random);

        let playlist = EffectPlaylist::new(
            [EffectPlaylistEntry::new(
                "/effects/demo.wasm".try_into().unwrap(),
                Some(Duration::from_millis(5000)),
                NextPlaylistEntryAction::Shift(1),
            )]
            .into(),
        );
        let mut buffer = [0u8; 512];
        let len = bled_core::io::encode_postcard(&playlist, &mut buffer).unwrap();

        let decoded_postcard = internal_decode_playlist(&buffer[..len]).unwrap();
        assert_eq!(decoded_postcard, playlist);
    }
}
