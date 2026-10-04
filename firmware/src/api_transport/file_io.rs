use crate::sdcard::{File, FsError, OpenOptions, SdCardStorage};
use bled_core::{FileStatResponse, MAX_PATH_LEN};
use defmt::{Format, debug, info};
use heapless::String;

#[derive(Debug, Format)]
pub(crate) enum ReadChunkError {
    NotReading,
    Fs(FsError),
}

#[derive(Debug, Format)]
pub(crate) enum WriteChunkError {
    NotWriting,
    Fs(FsError),
}

#[derive(Default)]
pub(crate) enum FileIoMode {
    #[default]
    Idle,

    Stat {
        response: FileStatResponse,
    },

    Reading {
        path: String<MAX_PATH_LEN>,
        offset: usize,
        file: File,
    },

    Writing {
        path: String<MAX_PATH_LEN>,
        offset: usize,
        file: File,
    },
}

pub(crate) struct FileIoSession {
    sd: SdCardStorage,
    mode: FileIoMode,
}

impl FileIoSession {
    pub(crate) fn new(sd: SdCardStorage) -> Self {
        Self {
            sd,
            mode: Default::default(),
        }
    }

    pub(crate) async fn reset(&mut self) {
        let old_mode = core::mem::replace(&mut self.mode, FileIoMode::Idle);

        match old_mode {
            FileIoMode::Reading { path, file, .. } => {
                debug!("closing active read file: {}", path);
                let _ = self.sd.close_file(file).await;
            }
            FileIoMode::Writing { path, file, .. } => {
                debug!("closing active write file: {}", path);
                let _ = self.sd.close_file(file).await;
            }
            _ => {}
        }
    }

    pub(crate) fn mode(&self) -> &FileIoMode {
        &self.mode
    }

    pub(crate) async fn prepare_stat(&mut self, path: String<MAX_PATH_LEN>) -> Result<(), FsError> {
        info!("Prepare stat file response for {}", path);

        self.reset().await;

        let size = self.sd.stat_file(path.as_str()).await?;
        self.mode = FileIoMode::Stat {
            response: FileStatResponse::new(size as u32),
        };

        Ok(())
    }

    pub(crate) async fn prepare_read(
        &mut self,
        path: String<MAX_PATH_LEN>,
        offset: usize,
    ) -> Result<(), FsError> {
        info!(
            "Prepare read file response for {} at offset {}",
            path, offset
        );

        match &self.mode {
            FileIoMode::Reading {
                path: current_path, ..
            } if *current_path == path => {}
            _ => {
                self.reset().await;
            }
        }

        let file = match core::mem::take(&mut self.mode) {
            FileIoMode::Reading { file, .. } => file,
            _ => {
                self.sd
                    .open_file(path.as_str(), OpenOptions::new().read(true))
                    .await?
            }
        };

        self.mode = FileIoMode::Reading { path, offset, file };

        Ok(())
    }

    pub(crate) async fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, ReadChunkError> {
        info!("Reading chunk (len = {})", buf.len());

        if let FileIoMode::Reading { offset, file, .. } = &mut self.mode {
            debug!("Seeking to offset {}", offset);
            self.sd
                .seek_file(file, *offset as u64)
                .await
                .map_err(ReadChunkError::Fs)?;

            let bytes_read = self
                .sd
                .read_open_file(file, buf)
                .await
                .map_err(ReadChunkError::Fs)?;

            Ok(bytes_read)
        } else {
            Err(ReadChunkError::NotReading)
        }
    }

    pub(crate) async fn create_file(
        &mut self,
        path: String<MAX_PATH_LEN>,
        size: usize,
    ) -> Result<(), FsError> {
        info!("Creating file: {} (size = {})", path, size);

        self.reset().await;

        self.sd.create_file(path.as_str(), size as u64).await
    }

    pub(crate) async fn prepare_write(
        &mut self,
        path: String<MAX_PATH_LEN>,
        offset: usize,
    ) -> Result<(), FsError> {
        info!(
            "Prepare write file request for {} at offset {}",
            path, offset
        );

        match &self.mode {
            FileIoMode::Writing {
                path: current_path, ..
            } if *current_path == path => {}
            _ => {
                self.reset().await;
            }
        }

        let file = match core::mem::take(&mut self.mode) {
            FileIoMode::Writing { file, .. } => file,
            _ => {
                self.sd
                    .open_file(path.as_str(), OpenOptions::new().read(true).write(true))
                    .await?
            }
        };

        self.mode = FileIoMode::Writing { path, offset, file };

        Ok(())
    }

    pub(crate) async fn write_chunk(
        &mut self,
        chunk_offset: u64,
        data: &[u8],
    ) -> Result<(), WriteChunkError> {
        info!("Writing chunk (len = {})", data.len());

        if let FileIoMode::Writing { offset, file, .. } = &mut self.mode {
            let target_offset = (*offset as u64) + chunk_offset;

            debug!("Seeking to offset {}", target_offset);
            self.sd
                .seek_file(file, target_offset)
                .await
                .map_err(WriteChunkError::Fs)?;

            self.sd
                .write_open_file(file, data)
                .await
                .map_err(WriteChunkError::Fs)?;

            Ok(())
        } else {
            Err(WriteChunkError::NotWriting)
        }
    }

    pub(crate) async fn delete_file(&mut self, path: &str) -> Result<(), FsError> {
        info!("Deleting file: {}", path);

        self.reset().await;

        self.sd.delete_file(path).await
    }
}
