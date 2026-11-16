use crate::PathString;
use heapless::CapacityError;

/// Tests if the given path ends with the given extension, ignoring case.
#[must_use]
pub fn has_extension(path: &str, ext: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case(ext))
}

/// Resolves a relative path against a base file path, returning the resolved path.
pub fn resolve_relative_path(
    base_file_path: &str,
    target_path: &str,
) -> Result<PathString, CapacityError> {
    let mut resolved = PathString::new();
    if target_path.starts_with('/') {
        resolved.push_str(target_path)?;
    } else {
        if let Some(parent_end) = base_file_path.rfind('/') {
            resolved.push_str(&base_file_path[..=parent_end])?;
        }
        resolved.push_str(target_path)?;
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MAX_PATH_LEN;

    #[test]
    fn has_extension_matches_case_insensitively() {
        assert!(has_extension("/effects/chase.wasm", "wasm"));
        assert!(has_extension("/effects/chase.WASM", "wasm"));
        assert!(has_extension("/effects/chase.Wasm", "WASM"));
        assert!(has_extension("preset.json", "json"));
        assert!(has_extension("my.effect.v1.wasm", "wasm"));
    }

    #[test]
    fn has_extension_rejects_non_matching() {
        assert!(!has_extension("/effects/chase.wasm", "json"));
        assert!(!has_extension("/effects/chase", "wasm"));
        assert!(!has_extension("/effects/chase.", "wasm"));
        assert!(!has_extension("", "wasm"));
        assert!(!has_extension("/dir.wasm/chase", "wasm"));
    }

    #[test]
    fn resolve_relative_path_with_relative_target() {
        let resolved = resolve_relative_path("/playlists/default.json", "chase.wasm").unwrap();
        assert_eq!(resolved.as_str(), "/playlists/chase.wasm");

        let nested = resolve_relative_path("/playlists/sub/default.json", "fx/chase.wasm").unwrap();
        assert_eq!(nested.as_str(), "/playlists/sub/fx/chase.wasm");
    }

    #[test]
    fn resolve_relative_path_with_root_base() {
        let resolved = resolve_relative_path("/default.json", "chase.wasm").unwrap();
        assert_eq!(resolved.as_str(), "/chase.wasm");
    }

    #[test]
    fn resolve_relative_path_without_base_directory() {
        let resolved = resolve_relative_path("default.json", "chase.wasm").unwrap();
        assert_eq!(resolved.as_str(), "chase.wasm");
    }

    #[test]
    fn resolve_relative_path_with_absolute_target() {
        let resolved =
            resolve_relative_path("/playlists/default.json", "/effects/chase.wasm").unwrap();
        assert_eq!(resolved.as_str(), "/effects/chase.wasm");
    }

    #[test]
    fn resolve_relative_path_errors_when_too_long() {
        let max_target = "a".repeat(MAX_PATH_LEN);
        assert_eq!(
            resolve_relative_path("", &max_target).unwrap().as_str(),
            max_target
        );

        let too_long_target = "a".repeat(MAX_PATH_LEN + 1);
        assert!(resolve_relative_path("/playlists/default.json", &too_long_target).is_err());

        let long_absolute = alloc_less_str_exceeding_capacity();
        assert!(resolve_relative_path("/a.json", long_absolute).is_err());

        // Base prefix ("/playlists/" = 11 bytes) + 54 bytes = 65 bytes (> MAX_PATH_LEN = 64)
        let target_54 = "a".repeat(MAX_PATH_LEN - 10);
        assert!(resolve_relative_path("/playlists/default.json", &target_54).is_err());
    }

    const fn alloc_less_str_exceeding_capacity() -> &'static str {
        "/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }
}
