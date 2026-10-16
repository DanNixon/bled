pub(crate) mod block_device;

pub(super) use block_device::*;

use bled_core::api::{DirEntry, DirEntryType, MAX_PATH_LEN};
use defmt::{Format, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use exfat_slim::blocking::error::ExFatError;
pub(crate) use exfat_slim::blocking::file::{File, OpenOptions};
use heapless::String;
use static_cell::StaticCell;

pub(crate) type DirectoryIterator = exfat_slim::blocking::directory::DirectoryIterator<512>;

#[derive(Debug, Format)]
pub(crate) enum ReadFileError {
    Open(FsError),
    FileTooLarge { len: usize, capacity: usize },
    Read(FsError),
}

#[derive(Clone)]
pub(crate) struct SdCardStorage {
    fs: &'static Mutex<CriticalSectionRawMutex, Option<FileSystem>>,
}

static SD_FS: StaticCell<Mutex<CriticalSectionRawMutex, Option<FileSystem>>> = StaticCell::new();

impl SdCardStorage {
    pub(super) fn new(sdcard: SdBlockDevice) -> SdCardStorage {
        let fs = FileSystem::new(sdcard);
        let fs = SD_FS.init(Mutex::new(Some(fs)));
        SdCardStorage { fs }
    }

    #[inline(never)]
    pub(crate) async fn read_file(
        &self,
        filename: &str,
        bytes: &mut [u8],
    ) -> Result<usize, ReadFileError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        match fs.open(filename, OpenOptions::new().read(true)) {
            Ok(mut file) => {
                let len = file.metadata().len() as usize;
                if len > bytes.len() {
                    warn!(
                        "file {} is too large ({} > {} bytes)",
                        filename,
                        len,
                        bytes.len()
                    );
                    Err(ReadFileError::FileTooLarge {
                        len,
                        capacity: bytes.len(),
                    })
                } else {
                    match file.read(fs, &mut bytes[..len]) {
                        Ok(_) => Ok(len),
                        Err(e) => {
                            warn!("failed to read file {}: {}", filename, e);
                            Err(ReadFileError::Read(e))
                        }
                    }
                }
            }
            Err(e) => {
                warn!("failed to open file {}: {}", filename, e);
                Err(ReadFileError::Open(e))
            }
        }
    }

    #[inline(never)]
    pub(crate) async fn write_file(&self, filename: &str, bytes: &[u8]) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        fs.write(filename, bytes).map_err(|e| {
            warn!("failed to write file {}: {}", filename, e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn open_file(
        &self,
        filename: &str,
        options: OpenOptions,
    ) -> Result<File, FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        fs.open(filename, options).map_err(|e| {
            warn!("failed to open file {}: {}", filename, e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn seek_file(&self, file: &mut File, offset: u64) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        file.seek(fs, offset).map_err(|e| {
            warn!("failed to seek in file {}: {}", offset, e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn read_open_file(
        &self,
        file: &mut File,
        buf: &mut [u8],
    ) -> Result<usize, FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        match file.read(fs, buf) {
            Ok(Some(n)) => Ok(n),
            Ok(None) => Ok(0),
            Err(e) => {
                warn!("failed to read from file: {}", e);
                Err(e)
            }
        }
    }

    #[inline(never)]
    pub(crate) async fn write_open_file(&self, file: &mut File, buf: &[u8]) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        file.write(fs, buf).map_err(|e| {
            warn!("failed to write to file: {}", e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn close_file(&self, file: File) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        file.close(fs).map_err(|e| {
            warn!("failed to close file: {}", e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn stat_file(&self, filename: &str) -> Result<u64, FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        match fs.open(filename, OpenOptions::new().read(true).write(true)) {
            Ok(file) => Ok(file.metadata().len()),
            Err(e) => {
                warn!("failed to open file {}: {}", filename, e);
                Err(e)
            }
        }
    }

    #[inline(never)]
    pub(crate) async fn create_file(&self, filename: &str, size: u64) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        match fs.open(
            filename,
            OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(true)
                .create(true),
        ) {
            Ok(mut file) => {
                let size = size as usize;

                const CHUNK_SIZE: usize = 512;
                let zeros = [0u8; CHUNK_SIZE];

                let mut remaining = size;

                // Write zeros to the file up to the requested size
                while remaining != 0 {
                    let chunk_len = remaining.min(CHUNK_SIZE);

                    file.write(fs, &zeros[..chunk_len]).map_err(|e| {
                        warn!("failed to write to file {}: {}", filename, e);
                        e
                    })?;

                    remaining -= chunk_len;

                    embassy_futures::yield_now().await;
                }

                file.close(fs).map_err(|e| {
                    warn!("failed to close file {}: {}", filename, e);
                    e
                })?;

                Ok(())
            }
            Err(e) => {
                warn!("failed to open file {}: {}", filename, e);
                Err(e)
            }
        }
    }

    #[inline(never)]
    pub(crate) async fn delete_file(&self, filename: &str) -> Result<(), FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        fs.remove_file(filename).map_err(|e| {
            warn!("failed to delete file {}: {}", filename, e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn open_dir(&self, path: &str) -> Result<DirectoryIterator, FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        fs.read_dir(path).map_err(|e| {
            warn!("failed to open dir {}: {}", path, e);
            e
        })
    }

    #[inline(never)]
    pub(crate) async fn next_dir_entry(
        &self,
        dir: &mut DirectoryIterator,
    ) -> Result<Option<DirEntry>, FsError> {
        let mut guard = self.fs.lock().await;
        let fs = guard.as_mut().unwrap();

        let mut name_buf = [0u8; exfat_slim::blocking::directory::MAX_NAME_LEN];
        match dir.next_entry(fs, &mut name_buf) {
            Ok(Some(entry)) => {
                let mut heapless_name = String::<MAX_PATH_LEN>::new();
                for c in entry.name.chars() {
                    if heapless_name.len() + c.len_utf8() > MAX_PATH_LEN {
                        break;
                    }
                    let _ = heapless_name.push(c);
                }

                let kind = if entry.metadata.is_dir() {
                    DirEntryType::Directory
                } else {
                    DirEntryType::File {
                        size: entry.metadata.len(),
                    }
                };
                Ok(Some(DirEntry::new(heapless_name, kind)))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("failed to read next dir entry: {}", e);
                Err(e)
            }
        }
    }
}

type FileSystem = exfat_slim::blocking::file_system::FileSystem<SdBlockDevice, 512, 4>;

pub(crate) type FsError = ExFatError<SdBlockDeviceError>;
