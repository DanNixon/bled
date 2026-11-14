use crate::{NameString, PathString, ble::BlePasskey, config::Fixtures};
use getset::Getters;
use serde::{Deserialize, Serialize};

/// Maximum allowable size of the config file in bytes when encoded as JSON.
pub const MAX_CONFIG_JSON_SIZE: usize = 1024 * 10;

/// Maximum allowable size of the config file in bytes when encoded as Postcard.
pub const MAX_CONFIG_POSTCARD_SIZE: usize = 1024 * 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: NameString,

    /// 6-digit passkey required for Bluetooth Low Energy pairing.
    pub ble_passkey: BlePasskey,

    /// Light fixtures controlled by this device.
    pub fixtures: Fixtures,

    /// Filename of the WASM binary or JSON preset for the effect to load on boot.
    pub default_effect: Option<PathString>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChannelNumber, RGB8,
        config::{
            BrightnessLimit, BrightnessLimitMode, Fixture, MAX_FIXTURE_COUNT,
            MAX_SPAN_PER_FIXTURE_COUNT, Matrix2d, MatrixLayout, MatrixOrigin, MatrixWiringOrder,
            PixelLayout, PixelSpan, Size,
        },
    };
    use heapless::Vec;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<DeviceConfig>(), 3064);
    }

    fn biggest_config() -> DeviceConfig {
        let name: NameString = "0123456789abcdef".try_into().unwrap();
        let default_effect: PathString =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .try_into()
                .unwrap();

        let origins = [
            MatrixOrigin::TopLeft,
            MatrixOrigin::BottomLeft,
            MatrixOrigin::TopRight,
            MatrixOrigin::BottomRight,
        ];
        let layouts = [MatrixLayout::RowMajor, MatrixLayout::ColumnMajor];
        let wiring_orders = [MatrixWiringOrder::Linear, MatrixWiringOrder::Serpentine];

        let brightness_limits = [
            BrightnessLimit::default(),
            BrightnessLimit::new(BrightnessLimitMode::Clamp, RGB8::new(128, 128, 128)),
            BrightnessLimit::new(BrightnessLimitMode::Clamp, RGB8::new(255, 100, 50)),
            BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(200, 200, 200)),
            BrightnessLimit::new(BrightnessLimitMode::Rescale, RGB8::new(64, 128, 255)),
        ];

        let fixtures: Vec<Fixture, MAX_FIXTURE_COUNT> = (0..MAX_FIXTURE_COUNT)
            .map(|fixture_idx| {
                let channel = ChannelNumber::new((fixture_idx % 4) as u8);
                let base_start = ((fixture_idx / 4) * MAX_SPAN_PER_FIXTURE_COUNT) as u32;

                let layout = PixelLayout::Matrix2d(Matrix2d::new(
                    Size::new(2, 4),
                    origins[fixture_idx % origins.len()],
                    layouts[fixture_idx % layouts.len()],
                    wiring_orders[fixture_idx % wiring_orders.len()],
                ));

                let spans: Vec<PixelSpan, MAX_SPAN_PER_FIXTURE_COUNT> = (0
                    ..MAX_SPAN_PER_FIXTURE_COUNT)
                    .map(|span_idx| {
                        let limit =
                            brightness_limits[(fixture_idx + span_idx) % brightness_limits.len()];
                        PixelSpan::new(
                            name.clone(),
                            channel,
                            base_start + span_idx as u32,
                            1,
                            limit,
                        )
                    })
                    .collect();

                Fixture::new(name.clone(), layout, spans)
            })
            .collect();

        let config = DeviceConfig {
            name,
            ble_passkey: BlePasskey::try_new(123_456).unwrap(),
            fixtures: Fixtures::new(fixtures),
            default_effect: Some(default_effect),
        };

        config.fixtures().validate::<4>().unwrap();

        config
    }

    #[test]
    fn max_json_size_is_reasonable() {
        let config = biggest_config();
        let mut buf = [0; MAX_CONFIG_JSON_SIZE];
        let n = crate::io::encode_json(&config, &mut buf).unwrap();
        assert!(n < MAX_CONFIG_JSON_SIZE - 256);
    }

    #[test]
    fn max_postcard_size_is_reasonable() {
        let config = biggest_config();
        let mut buf = [0; MAX_CONFIG_POSTCARD_SIZE];
        let n = crate::io::encode_postcard(&config, &mut buf).unwrap();
        assert!(n < MAX_CONFIG_POSTCARD_SIZE);
    }
}
