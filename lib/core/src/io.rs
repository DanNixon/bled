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

pub fn encode_postcard<T: Serialize>(value: &T, data: &mut [u8]) -> Result<usize, postcard::Error> {
    postcard::to_slice(value, data).map(|encoded| encoded.len())
}

pub fn decode_postcard<T: DeserializeOwned>(data: &[u8]) -> Result<T, postcard::Error> {
    postcard::from_bytes(data)
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
    fn postcard_round_trip() {
        let value = TestValue {
            count: 42,
            enabled: true,
        };
        let mut data = [0; 64];

        let length = encode_postcard(&value, &mut data).unwrap();
        assert_eq!(&data[..length], &[42, 1]);

        let decoded = decode_postcard::<TestValue>(&data[..length]).unwrap();
        assert_eq!(decoded, value);
    }
}
