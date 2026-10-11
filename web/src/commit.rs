//! Commit command encoding.

use wasm_bindgen::prelude::*;

/// Pure Rust encoding of a Commit command (empty payload).
pub fn internal_encode_commit() -> Vec<u8> {
    Vec::new()
}

/// Encodes a Commit command payload.
#[wasm_bindgen]
pub fn encode_commit() -> Vec<u8> {
    internal_encode_commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_commit() {
        assert_eq!(internal_encode_commit(), Vec::<u8>::new());
        assert_eq!(encode_commit(), Vec::<u8>::new());
    }
}
