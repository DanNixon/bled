//! Remote file I/O operations, command encoding, and write slicing.

use wasm_bindgen::prelude::*;

/// A sliced chunk of file write data and its associated control packet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileWriteChunk {
    pub offset: u32,
    #[serde(with = "serde_bytes")]
    pub control: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Converts a path string slice into a fixed-capacity heapless string, validating bounds.
fn to_heapless_path(path: &str) -> Result<heapless::String<{ bled_core::MAX_PATH_LEN }>, String> {
    if path.is_empty() {
        return Err("remote path cannot be empty".to_string());
    }
    path.try_into().map_err(|_| {
        format!(
            "remote path exceeds maximum length of {} bytes",
            bled_core::MAX_PATH_LEN
        )
    })
}

/// Converts a directory path string slice into a fixed-capacity heapless string, validating bounds.
/// Empty path is normalized to "/".
fn to_heapless_dir_path(
    path: &str,
) -> Result<heapless::String<{ bled_core::MAX_PATH_LEN }>, String> {
    let p = if path.is_empty() { "/" } else { path };
    p.try_into().map_err(|_| {
        format!(
            "remote path exceeds maximum length of {} bytes",
            bled_core::MAX_PATH_LEN
        )
    })
}

/// Pure Rust encoding of a `FileIoControl` command into CBOR.
pub fn internal_encode_file_io_control(cmd: &bled_core::FileIoControl) -> Result<Vec<u8>, String> {
    let mut buffer = [0u8; bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN];
    let written = bled_core::io::encode_cbor(cmd, &mut buffer)
        .map_err(|e| format!("failed to encode File IO control: {e}"))?;
    Ok(buffer[..written].to_vec())
}

/// Pure Rust encoding of a File IO Stat command for `path`.
pub fn internal_encode_file_io_stat(path: &str) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::Stat {
        path: heapless_path,
    })
}

/// Pure Rust encoding of a File IO Read command for `path` at `offset`.
pub fn internal_encode_file_io_read(path: &str, offset: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::Read {
        path: heapless_path,
        offset,
    })
}

/// Pure Rust encoding of a File IO Create command for `path` with allocated `size`.
pub fn internal_encode_file_io_create(path: &str, size: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::Create {
        path: heapless_path,
        size,
    })
}

/// Pure Rust encoding of a File IO Write command for `path` at `offset`.
pub fn internal_encode_file_io_write(path: &str, offset: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::Write {
        path: heapless_path,
        offset,
    })
}

/// Pure Rust encoding of a File IO Delete command for `path`.
pub fn internal_encode_file_io_delete(path: &str) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::Delete {
        path: heapless_path,
    })
}

/// Pure Rust encoding of a File IO Reset command.
pub fn internal_encode_file_io_reset() -> Result<Vec<u8>, String> {
    internal_encode_file_io_control(&bled_core::FileIoControl::Reset)
}

/// Pure Rust decoding of a CBOR payload into `FileStatResponse`.
pub fn internal_decode_file_stat(data: &[u8]) -> Result<bled_core::FileStatResponse, String> {
    bled_core::io::decode_cbor(data).map_err(|e| format!("failed to decode FileStatResponse: {e}"))
}

/// Pure Rust encoding of a File IO ReadDir command for `path` at `index`.
pub fn internal_encode_file_io_read_dir(path: &str, index: u32) -> Result<Vec<u8>, String> {
    let heapless_path = to_heapless_dir_path(path)?;
    internal_encode_file_io_control(&bled_core::FileIoControl::ReadDir {
        path: heapless_path,
        index,
    })
}

/// Pure Rust decoding of a CBOR payload into `DirEntry`.
pub fn internal_decode_dir_entry(data: &[u8]) -> Result<Option<bled_core::DirEntry>, String> {
    if data.is_empty() {
        return Ok(None);
    }
    bled_core::io::decode_cbor(data)
        .map(Some)
        .map_err(|e| format!("failed to decode DirEntry: {e}"))
}

/// Slices file write data into chunks fitting `payload_budget` with corresponding Write control packets.
pub fn internal_slice_file_write(
    path: &str,
    data: &[u8],
    payload_budget: usize,
) -> Result<Vec<FileWriteChunk>, String> {
    let chunk_size = payload_budget.min(bled_core::ble::MAX_ATTRIBUTE_VALUE_LEN);
    if chunk_size == 0 {
        return Err("payload budget must be greater than 0".into());
    }
    let heapless_path = to_heapless_path(path)?;
    let mut chunks = Vec::new();
    let mut offset: u32 = 0;
    for chunk in data.chunks(chunk_size) {
        let control = internal_encode_file_io_control(&bled_core::FileIoControl::Write {
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

/// Encodes a `FileIoControl::ReadDir { path, index }` command into CBOR.
#[wasm_bindgen]
pub fn encode_file_io_read_dir(path: &str, index: u32) -> Result<Vec<u8>, JsError> {
    internal_encode_file_io_read_dir(path, index).map_err(|e| JsError::new(&e))
}

/// Decodes the CBOR payload from `FILE_IO_DATA_UUID` for a stat request into `{ size: number }`.
#[wasm_bindgen]
pub fn decode_file_stat(data: &[u8]) -> Result<JsValue, JsError> {
    let stat = internal_decode_file_stat(data).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&stat)
        .map_err(|e| JsError::new(&format!("failed to serialize FileStatResponse: {e}")))
}

/// Decodes the CBOR payload from `FILE_IO_DATA_UUID` for a read dir request into `{ name, size, is_dir }` or `null`.
#[wasm_bindgen]
pub fn decode_dir_entry(data: &[u8]) -> Result<JsValue, JsError> {
    let entry = internal_decode_dir_entry(data).map_err(|e| JsError::new(&e))?;
    match entry {
        Some(e) => serde_wasm_bindgen::to_value(&e)
            .map_err(|err| JsError::new(&format!("failed to serialize DirEntry: {err}"))),
        None => Ok(JsValue::NULL),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_io_control_round_trip() {
        // Stat
        let stat_bytes = internal_encode_file_io_stat("/config.json").unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&stat_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Stat {
                path: "/config.json".try_into().unwrap()
            }
        );

        // Read
        let read_bytes = internal_encode_file_io_read("/test.txt", 128).unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&read_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Read {
                path: "/test.txt".try_into().unwrap(),
                offset: 128,
            }
        );

        // Create
        let create_bytes = internal_encode_file_io_create("/new.bin", 1024).unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&create_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Create {
                path: "/new.bin".try_into().unwrap(),
                size: 1024,
            }
        );

        // Write
        let write_bytes = internal_encode_file_io_write("/write.bin", 64).unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&write_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Write {
                path: "/write.bin".try_into().unwrap(),
                offset: 64,
            }
        );

        // Delete
        let delete_bytes = internal_encode_file_io_delete("/del.bin").unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&delete_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Delete {
                path: "/del.bin".try_into().unwrap()
            }
        );

        // Reset
        let reset_bytes = internal_encode_file_io_reset().unwrap();
        let decoded: bled_core::FileIoControl = bled_core::io::decode_cbor(&reset_bytes).unwrap();
        assert_eq!(decoded, bled_core::FileIoControl::Reset);

        // ReadDir
        let read_dir_bytes = internal_encode_file_io_read_dir("/", 3).unwrap();
        let decoded: bled_core::FileIoControl =
            bled_core::io::decode_cbor(&read_dir_bytes).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::ReadDir {
                path: "/".try_into().unwrap(),
                index: 3,
            }
        );
    }

    #[test]
    fn test_file_io_path_validation() {
        assert!(internal_encode_file_io_stat("").is_err());
        let long_path = "a".repeat(bled_core::MAX_PATH_LEN + 1);
        assert!(internal_encode_file_io_stat(&long_path).is_err());
        let exact_path = "a".repeat(bled_core::MAX_PATH_LEN);
        assert!(internal_encode_file_io_stat(&exact_path).is_ok());

        // ReadDir normalizes empty path to "/"
        assert!(internal_encode_file_io_read_dir("", 0).is_ok());
        assert!(internal_encode_file_io_read_dir(&long_path, 0).is_err());
    }

    #[test]
    fn test_decode_file_stat() {
        let stat = bled_core::FileStatResponse::new(4096);
        let mut buffer = [0u8; 64];
        let written = bled_core::io::encode_cbor(&stat, &mut buffer).unwrap();
        let decoded = internal_decode_file_stat(&buffer[..written]).unwrap();
        assert_eq!(*decoded.size(), 4096);
    }

    #[test]
    fn test_decode_dir_entry() {
        let entry = bled_core::DirEntry::new(
            "test.txt".try_into().unwrap(),
            bled_core::DirEntryType::File { size: 512 },
        );
        let mut buffer = [0u8; 64];
        let written = bled_core::io::encode_cbor(&entry, &mut buffer).unwrap();
        let decoded = internal_decode_dir_entry(&buffer[..written]).unwrap();
        assert_eq!(decoded, Some(entry));

        let dir = bled_core::DirEntry::new(
            "subdir".try_into().unwrap(),
            bled_core::DirEntryType::Directory,
        );
        let written_dir = bled_core::io::encode_cbor(&dir, &mut buffer).unwrap();
        let decoded_dir = internal_decode_dir_entry(&buffer[..written_dir]).unwrap();
        assert_eq!(decoded_dir, Some(dir));

        // Empty data returns None
        let empty_decoded = internal_decode_dir_entry(&[]).unwrap();
        assert_eq!(empty_decoded, None);
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

        let decoded: bled_core::FileIoControl =
            bled_core::io::decode_cbor(&chunks[1].control).unwrap();
        assert_eq!(
            decoded,
            bled_core::FileIoControl::Write {
                path: "/out.bin".try_into().unwrap(),
                offset: 100,
            }
        );
    }
}
