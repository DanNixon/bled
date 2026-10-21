use bled_core::api::EffectManifestStatResponse;
use defmt::Format;

#[derive(Debug, PartialEq, Eq, Format)]
pub(crate) enum Error {
    NoEffect,
    EffectChanged,
    IndexOutOfBounds,
    NotReading,
    Encode,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Format)]
enum EffectManifestMode {
    #[default]
    Idle,
    Stat {
        response: EffectManifestStatResponse,
    },
    Reading {
        offset: usize,
    },
}

#[derive(Default)]
pub(crate) struct EffectManifestSession {
    instance_id: Option<u32>,
    mode: EffectManifestMode,
}

impl EffectManifestSession {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reset(&mut self) {
        self.instance_id = None;
        self.mode = EffectManifestMode::Idle;
    }

    pub(crate) fn prepare_stat(&mut self, manifest: Option<(u32, &[u8])>) -> Result<(), Error> {
        self.reset();
        let (instance_id, bytes) = manifest.ok_or(Error::NoEffect)?;
        let size = u32::try_from(bytes.len()).map_err(|_| Error::IndexOutOfBounds)?;
        self.instance_id = Some(instance_id);
        self.mode = EffectManifestMode::Stat {
            response: EffectManifestStatResponse::new(size),
        };
        Ok(())
    }

    pub(crate) fn prepare_read(
        &mut self,
        manifest: Option<(u32, &[u8])>,
        offset: usize,
    ) -> Result<(), Error> {
        self.reset();
        let (instance_id, bytes) = manifest.ok_or(Error::NoEffect)?;
        if offset > bytes.len() {
            return Err(Error::IndexOutOfBounds);
        }
        self.instance_id = Some(instance_id);
        self.mode = EffectManifestMode::Reading { offset };
        Ok(())
    }

    pub(crate) fn read_data(
        &mut self,
        manifest: Option<(u32, &[u8])>,
        out: &mut [u8],
    ) -> Result<usize, Error> {
        if self.mode == EffectManifestMode::Idle {
            return Err(Error::NotReading);
        }
        let Some((instance_id, bytes)) = manifest else {
            self.reset();
            return Err(Error::NoEffect);
        };
        if self.instance_id != Some(instance_id) {
            self.reset();
            return Err(Error::EffectChanged);
        }
        match &self.mode {
            EffectManifestMode::Stat { response } => {
                bled_core::io::encode_cbor(response, out).map_err(|_| Error::Encode)
            }
            EffectManifestMode::Reading { offset } => {
                let available = bytes.get(*offset..).ok_or(Error::IndexOutOfBounds)?;
                let len = out.len().min(available.len());
                out[..len].copy_from_slice(&available[..len]);
                // Keep the selected window stable for repeated reads and ATT Read Blob requests.
                Ok(len)
            }
            EffectManifestMode::Idle => Err(Error::NotReading),
        }
    }
}
