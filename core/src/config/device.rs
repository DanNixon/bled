use crate::{NameString, PathString, config::Fixtures};
use getset::Getters;
use serde::{Deserialize, Serialize};

/// Maximum allowable size of the config file in bytes when encoded as JSON.
pub const MAX_CONFIG_JSON_SIZE: usize = 1024 * 5;

/// Maximum allowable size of the config file in bytes when encoded as CBOR.
pub const MAX_CONFIG_CBOR_SIZE: usize = 1024 * 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Getters)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[getset(get = "pub")]
pub struct DeviceConfig {
    /// Human-readable device name.
    pub name: NameString,

    /// Light fixtures controlled by this device.
    pub fixtures: Fixtures,

    /// Filename of the WASM binary or JSON preset for the effect to load on boot.
    pub default_effect: Option<PathString>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_have_known_size() {
        assert_eq!(core::mem::size_of::<DeviceConfig>(), 2992);
    }

    fn biggest_config() -> DeviceConfig {
        let config: DeviceConfig = crate::io::decode_json(
            br#"{
                "name":"0123456789abcdef",
                "fixtures":[
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":0,"start":0,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":1,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":2,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":3,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":4,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":5,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":6,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":7,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":1,"start":0,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":1,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":2,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":3,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":4,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":5,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":6,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":7,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":2,"start":0,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":1,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":2,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":3,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":4,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":5,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":6,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":7,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":3,"start":0,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":1,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":2,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":3,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":4,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":5,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":6,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":7,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":0,"start":8,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":9,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":10,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":11,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":12,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":13,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":14,"length":1},
                            {"name":"0123456789abcdef","channel":0,"start":15,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":1,"start":8,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":9,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":10,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":11,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":12,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":13,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":14,"length":1},
                            {"name":"0123456789abcdef","channel":1,"start":15,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":2,"start":8,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":9,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":10,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":11,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":12,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":13,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":14,"length":1},
                            {"name":"0123456789abcdef","channel":2,"start":15,"length":1}
                        ]
                    },
                    {
                        "name":"0123456789abcdef",
                        "layout":{
                            "linear_array":{}
                        },
                        "spans":[
                            {"name":"0123456789abcdef","channel":3,"start":8,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":9,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":10,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":11,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":12,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":13,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":14,"length":1},
                            {"name":"0123456789abcdef","channel":3,"start":15,"length":1}
                        ]
                    }
                ]
            }"#,
        )
        .unwrap();

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
    fn max_cbor_size_is_reasonable() {
        let config = biggest_config();
        let mut buf = [0; MAX_CONFIG_CBOR_SIZE];
        let n = crate::io::encode_cbor(&config, &mut buf).unwrap();
        assert!(n < MAX_CONFIG_CBOR_SIZE);
    }
}
