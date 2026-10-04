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
}
