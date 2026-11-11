use bled_effect::bled_core::effect::EffectPreset;

fn main() {
    bled_effect::presets::write_presets(&[EffectPreset::new(
        "Pink Painter".try_into().unwrap(),
        "span_painter.wasm".try_into().unwrap(),
        [].into(),
        [255, 0, 255].into(),
    )])
    .unwrap();
}
