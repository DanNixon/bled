use crate::leds;
use bled_core::LedBufferInformation;
use defmt::{Format, info};

#[derive(Debug, Format)]
pub(crate) enum ReadChunkError {
    NotReading,
}

#[derive(Debug, Format)]
pub(crate) enum WriteChunkError {
    NotWriting,
    OutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Format)]
pub(crate) enum LedBufferMode {
    #[default]
    Idle,

    Info {
        response: LedBufferInformation,
    },

    Reading {
        offset: usize,
    },

    Writing {
        offset: usize,
    },
}

#[derive(Clone, Default)]
pub(crate) struct LedBufferSession {
    mode: LedBufferMode,
}

impl LedBufferSession {
    pub(crate) fn new() -> Self {
        Self {
            mode: Default::default(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.mode = LedBufferMode::Idle;
    }

    pub(crate) fn mode(&self) -> &LedBufferMode {
        &self.mode
    }

    pub(crate) async fn prepare_info(&mut self) {
        info!("Prepare LED buffer info response");

        let channel_data = leds::channel_data().await;

        self.mode = LedBufferMode::Info {
            response: channel_data.info(),
        };
    }

    pub(crate) async fn prepare_read(&mut self, offset: usize) -> Result<(), ()> {
        info!("Prepare read LED buffer response at offset {}", offset);

        let channel_data = leds::channel_data().await;
        if offset > channel_data.size() {
            return Err(());
        }

        self.mode = LedBufferMode::Reading { offset };
        Ok(())
    }

    pub(crate) async fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, ReadChunkError> {
        info!("Reading LED buffer chunk (len = {})", buf.len());

        let offset = match &self.mode {
            LedBufferMode::Reading { offset } => *offset,
            _ => return Err(ReadChunkError::NotReading),
        };

        let channel_data = leds::channel_data().await;
        let size = channel_data.size();
        if offset >= size {
            return Ok(0);
        }

        let available = &channel_data.inner()[offset..size];
        let bytes_to_read = buf.len().min(available.len());
        buf[..bytes_to_read].copy_from_slice(&available[..bytes_to_read]);

        Ok(bytes_to_read)
    }

    pub(crate) async fn prepare_write(&mut self, offset: usize) -> Result<(), ()> {
        info!("Prepare write LED buffer request at offset {}", offset);

        let channel_data = leds::channel_data().await;
        if offset > channel_data.size() {
            return Err(());
        }

        self.mode = LedBufferMode::Writing { offset };
        Ok(())
    }

    pub(crate) async fn write_chunk(
        &mut self,
        chunk_offset: usize,
        data: &[u8],
    ) -> Result<(), WriteChunkError> {
        info!("Writing LED buffer chunk (len = {})", data.len());

        let base_offset = match &self.mode {
            LedBufferMode::Writing { offset } => *offset,
            _ => return Err(WriteChunkError::NotWriting),
        };

        let start = base_offset + chunk_offset;
        let end = start
            .checked_add(data.len())
            .ok_or(WriteChunkError::OutOfBounds)?;

        let mut channel_data = leds::channel_data().await;
        if end > channel_data.size() {
            return Err(WriteChunkError::OutOfBounds);
        }

        channel_data.inner_mut()[start..end].copy_from_slice(data);
        Ok(())
    }
}
