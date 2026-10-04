use bled_core::{ConfigStatResponse, DeviceConfig, MAX_CONFIG_CBOR_SIZE};
use defmt::{Format, info};

#[derive(Debug, Format)]
pub(crate) enum Error {
    EncodeError,
    IndexOutOfBounds,
    NotReading,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Format)]
pub(crate) enum ConfigMode {
    #[default]
    Idle,

    Stat {
        response: ConfigStatResponse,
    },

    Reading {
        offset: usize,
    },
}

pub(crate) struct ConfigSession<'a> {
    config: &'a DeviceConfig,
    mode: ConfigMode,
}

impl<'a> ConfigSession<'a> {
    pub(crate) fn new(config: &'a DeviceConfig) -> Self {
        Self {
            config,
            mode: ConfigMode::Idle,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.mode = ConfigMode::Idle;
    }

    pub(crate) fn mode(&self) -> &ConfigMode {
        &self.mode
    }

    pub(crate) fn prepare_stat(&mut self) -> Result<(), Error> {
        info!("Prepare config stat response");

        let mut buf = [0u8; MAX_CONFIG_CBOR_SIZE];
        let bytes =
            bled_core::io::encode_cbor(self.config, &mut buf).map_err(|_| Error::EncodeError)?;

        self.mode = ConfigMode::Stat {
            response: ConfigStatResponse::new(bytes as u32),
        };

        Ok(())
    }

    pub(crate) fn prepare_read(&mut self, offset: usize) -> Result<(), Error> {
        info!("Prepare read config response at offset {}", offset);

        let mut buf = [0u8; MAX_CONFIG_CBOR_SIZE];
        let bytes =
            bled_core::io::encode_cbor(self.config, &mut buf).map_err(|_| Error::EncodeError)?;

        if offset > bytes {
            return Err(Error::IndexOutOfBounds);
        }

        self.mode = ConfigMode::Reading { offset };

        Ok(())
    }

    pub(crate) fn read_chunk(&mut self, out: &mut [u8]) -> Result<usize, Error> {
        info!("Reading config chunk (len = {})", out.len());

        let offset = match &self.mode {
            ConfigMode::Reading { offset } => *offset,
            _ => return Err(Error::NotReading),
        };

        let mut buf = [0u8; MAX_CONFIG_CBOR_SIZE];
        let bytes =
            bled_core::io::encode_cbor(self.config, &mut buf).map_err(|_| Error::EncodeError)?;

        if offset >= bytes {
            return Ok(0);
        }

        let available = &buf[offset..bytes];
        let bytes_to_read = out.len().min(available.len());
        out[..bytes_to_read].copy_from_slice(&available[..bytes_to_read]);

        Ok(bytes_to_read)
    }
}
