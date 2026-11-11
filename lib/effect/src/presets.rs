use bled_core::{
    effect::{EffectPreset, MAX_EFFECT_PRESET_JSON_SIZE},
    io::encode_json,
};
use std::{
    fs,
    io::{self, ErrorKind},
    path::PathBuf,
    string::String,
};

/// Serialize presets into JSON files under `output_dir/presets`.
pub fn write_presets(presets: &[EffectPreset]) -> io::Result<()> {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let output_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("Cargo OUT_DIR should be under the target profile directory");
    let presets_dir = output_dir.join("presets");
    fs::create_dir_all(&presets_dir)?;

    let mut json = [0; MAX_EFFECT_PRESET_JSON_SIZE];
    for preset in presets {
        let file_name = preset
            .name()
            .as_str()
            .to_ascii_lowercase()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let json_len = encode_json(preset, &mut json)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, std::format!("{error:?}")))?;
        fs::write(
            presets_dir.join(std::format!("{file_name}.json")),
            &json[..json_len],
        )?;
    }

    Ok(())
}
