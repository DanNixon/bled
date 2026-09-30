use heapless::Vec;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub fn encode_json<T: Serialize>(
    value: &T,
    data: &mut [u8],
) -> serde_json_core::ser::Result<usize> {
    serde_json_core::to_slice(value, data)
}

pub fn decode_json<'de, T: Deserialize<'de>>(
    data: &'de [u8],
) -> Result<T, serde_json_core::de::Error> {
    Ok(serde_json_core::from_slice(data)?.0)
}

pub fn encode_cbor<T: Serialize>(
    value: &T,
    data: &mut [u8],
) -> Result<usize, minicbor_serde::error::EncodeError<minicbor::encode::write::EndOfSlice>> {
    let mut s = minicbor_serde::Serializer::new(minicbor::encode::write::Cursor::new(data));
    value.serialize(&mut s)?;
    Ok(s.encoder().writer().position())
}

pub fn decode_cbor<T: DeserializeOwned>(
    data: &[u8],
) -> Result<T, minicbor_serde::error::DecodeError> {
    minicbor_serde::from_slice(data)
}

/// A buffer that can be used to split a payload over multiple messages.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MultipartBuffer<const N: usize, const C: usize> {
    data: Vec<u8, N>,
}

impl<const N: usize, const C: usize> Default for MultipartBuffer<N, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const C: usize> MultipartBuffer<N, C> {
    const _VALID_CHUNK_SIZE: () = assert!(C > 0, "chunk size C must be greater than zero");

    #[must_use]
    pub fn new() -> Self {
        let () = Self::_VALID_CHUNK_SIZE;
        Self { data: Vec::new() }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[must_use]
    pub const fn chunk_size(&self) -> usize {
        C
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn push(&mut self, data: &[u8]) -> Result<(), heapless::CapacityError> {
        self.data.extend_from_slice(data)
    }

    pub fn parts(&self) -> impl Iterator<Item = &[u8]> {
        let () = Self::_VALID_CHUNK_SIZE;
        self.data.chunks(C)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct TestValue {
        count: u8,
        enabled: bool,
    }

    #[test]
    fn json_round_trip() {
        let value = TestValue {
            count: 42,
            enabled: true,
        };
        let mut data = [0; 64];

        let length = encode_json(&value, &mut data).unwrap();
        assert_eq!(&data[..length], br#"{"count":42,"enabled":true}"#);

        let decoded = decode_json::<TestValue>(&data[..length]).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn cbor_round_trip() {
        let value = TestValue {
            count: 42,
            enabled: true,
        };
        let mut data = [0; 64];

        let length = encode_cbor(&value, &mut data).unwrap();
        assert_eq!(length, 18);
        assert_eq!(
            &data[..length],
            &[
                0xa2, 0x65, b'c', b'o', b'u', b'n', b't', 0x18, 0x2a, 0x67, b'e', b'n', b'a', b'b',
                b'l', b'e', b'd', 0xf5
            ]
        );

        let decoded = decode_cbor::<TestValue>(&data[..length]).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn multipart_buffer_new_and_properties() {
        let buffer = MultipartBuffer::<8, 4>::new();
        assert_eq!(buffer.capacity(), 8);
        assert_eq!(buffer.chunk_size(), 4);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn multipart_buffer_empty() {
        let buffer = MultipartBuffer::<8, 4>::new();

        assert!(buffer.data().is_empty());
        assert_eq!(buffer.parts().next(), None);
    }

    #[test]
    fn multipart_buffer_push_and_data() {
        let mut buffer = MultipartBuffer::<8, 4>::new();

        buffer.push(&[1, 2, 3]).unwrap();
        assert_eq!(buffer.data(), &[1, 2, 3]);

        buffer.push(&[4, 5]).unwrap();
        assert_eq!(buffer.data(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn multipart_buffer_push_capacity_error() {
        let mut buffer = MultipartBuffer::<4, 2>::new();

        buffer.push(&[1, 2, 3]).unwrap();
        assert_eq!(buffer.data(), &[1, 2, 3]);

        let result = buffer.push(&[4, 5]);
        assert!(result.is_err());
        assert_eq!(buffer.data(), &[1, 2, 3]);

        buffer.push(&[4]).unwrap();
        assert_eq!(buffer.data(), &[1, 2, 3, 4]);

        let overflow = buffer.push(&[5]);
        assert!(overflow.is_err());
    }

    #[test]
    fn multipart_buffer_data_mut() {
        let mut buffer = MultipartBuffer::<8, 4>::new();

        buffer.push(&[1, 2, 3, 4]).unwrap();
        buffer.data_mut()[1] = 42;
        buffer.data_mut()[3] = 99;
        assert_eq!(buffer.data(), &[1, 42, 3, 99]);
    }

    #[test]
    fn multipart_buffer_clear() {
        let mut buffer = MultipartBuffer::<8, 4>::new();

        buffer.push(&[1, 2, 3, 4]).unwrap();
        assert_eq!(buffer.data(), &[1, 2, 3, 4]);

        buffer.clear();
        assert!(buffer.data().is_empty());

        buffer.push(&[5, 6]).unwrap();
        assert_eq!(buffer.data(), &[5, 6]);
    }

    #[test]
    fn multipart_buffer_parts_exact_multiple() {
        let mut buffer = MultipartBuffer::<8, 2>::new();

        buffer.push(&[1, 2, 3, 4, 5, 6]).unwrap();

        let mut parts = buffer.parts();
        assert_eq!(parts.next(), Some(&[1, 2][..]));
        assert_eq!(parts.next(), Some(&[3, 4][..]));
        assert_eq!(parts.next(), Some(&[5, 6][..]));
        assert_eq!(parts.next(), None);
    }

    #[test]
    fn multipart_buffer_parts_with_remainder() {
        let mut buffer = MultipartBuffer::<8, 3>::new();

        buffer.push(&[1, 2, 3, 4, 5, 6, 7]).unwrap();

        let mut parts = buffer.parts();
        assert_eq!(parts.next(), Some(&[1, 2, 3][..]));
        assert_eq!(parts.next(), Some(&[4, 5, 6][..]));
        assert_eq!(parts.next(), Some(&[7][..]));
        assert_eq!(parts.next(), None);
    }

    #[test]
    fn multipart_buffer_parts_smaller_than_chunk_size() {
        let mut buffer = MultipartBuffer::<8, 5>::new();

        buffer.push(&[1, 2]).unwrap();

        let mut parts = buffer.parts();
        assert_eq!(parts.next(), Some(&[1, 2][..]));
        assert_eq!(parts.next(), None);
    }
}
