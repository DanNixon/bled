use bled_effect::bled_core::effect::EffectPreset;

fn main() {
    bled_effect::presets::write_presets(&[EffectPreset::new(
        "Dim GoL".try_into().unwrap(),
        "dim-game-of-life.wasm".try_into().unwrap(),
        [].into(),
        [0, 0xF0, 31, 31, 31].into(),
    )])
    .unwrap();
}
