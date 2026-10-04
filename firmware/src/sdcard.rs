use crate::SdCardResources;
use aligned::{A4, Aligned};
use defmt::{Format, info, unwrap, warn};
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    spi::{Blocking, Config, Spi},
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{Block, BlockCount, BlockDevice as _, BlockIdx, SdCard as SpiSdCard};
use exfat_embedded::{BlockDevice as FormatBlockDevice, Scratch};
pub(crate) use exfat_slim::blocking::file::{File, OpenOptions};
use exfat_slim::blocking::{BlockDevice, error::ExFatError};
use mbr_nostd::{MasterBootRecord, PartitionTable};
use static_cell::StaticCell;

#[derive(Debug, Format)]
pub(crate) enum FormatError {
    Device(SdBlockDeviceError),
    Formatter,
}

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
    pub(super) fn new(r: SdCardResources) -> SdCardStorage {
        let sdcard = SdBlockDevice::new(r);
        let fs = FileSystem::new(sdcard);
        let fs = SD_FS.init(Mutex::new(Some(fs)));
        SdCardStorage { fs }
    }

    #[inline(never)]
    pub(crate) async fn format(&self) -> Result<(), FormatError> {
        info!("formatting sdcard");
        let mut guard = self.fs.lock().await;

        let fs = guard.take().unwrap();
        let mut device = fs.into_inner();

        device.block_count = Some(
            device
                .sd
                .num_blocks()
                .map_err(|e| FormatError::Device(SdBlockDeviceError::Sd(e)))?,
        );
        let mut scratch = [0u8; 512];
        let result = exfat_embedded::format_exfat(&mut device, &mut Scratch::new(&mut scratch))
            .map_err(|error| match error {
                exfat_embedded::Error::Device(error) => FormatError::Device(error),
                _ => FormatError::Formatter,
            })
            // exfat-embedded does not write a volume label directory entry, but
            // exfat-slim refuses to mount volumes without one, so add it ourselves.
            .and_then(|()| write_volume_label(&mut device, "BLED"));

        // The partition layout may have changed; re-detect it on next access.
        device.partition_offset = None;

        let fs = FileSystem::new(device);
        *guard = Some(fs);

        info!("formatting sdcard done");
        result
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
}

type FileSystem = exfat_slim::blocking::file_system::FileSystem<SdBlockDevice, 512, 4>;

pub(crate) type FsError = ExFatError<SdBlockDeviceError>;

pub(crate) type SdCard =
    SpiSdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>, Delay>;

#[derive(Debug, Format)]
pub(crate) enum SdBlockDeviceError {
    /// No SD card is inserted in the slot.
    CardNotInserted,

    /// Error while accessing the SD card.
    Sd(<SdCard as embedded_sdmmc::BlockDevice>::Error),
}

struct SdBlockDevice {
    detect: Input<'static>,
    sd: SdCard,
    partition_offset: Option<u32>,
    block_count: Option<BlockCount>,
}

impl SdBlockDevice {
    fn new(r: SdCardResources) -> SdBlockDevice {
        let detect = Input::new(r.detect, Pull::Up);

        let mut config = Config::default();
        config.frequency = 16_000_000;
        let spi = Spi::new_blocking(r.spi, r.sck, r.mosi, r.miso, config);
        let cs = Output::new(r.cs, Level::High);
        let spi = unwrap!(ExclusiveDevice::new(spi, cs, Delay));
        let sd = SpiSdCard::new(spi, Delay);

        SdBlockDevice {
            detect,
            sd,
            partition_offset: None,
            block_count: None,
        }
    }

    fn is_card_inserted(&self) -> bool {
        self.detect.is_low()
    }

    fn ensure_init(&mut self) -> Result<(), SdBlockDeviceError> {
        if !self.is_card_inserted() {
            return Err(SdBlockDeviceError::CardNotInserted);
        }

        if self.partition_offset.is_some() && self.block_count.is_some() {
            return Ok(());
        }

        let mut block = Block::new();
        self.sd
            .read(core::slice::from_mut(&mut block), BlockIdx(0))
            .map_err(SdBlockDeviceError::Sd)?;

        let mbr_offset = match MasterBootRecord::from_bytes(&block.contents) {
            Ok(mbr) => mbr
                .partition_table_entries()
                .first()
                .map(|entry| entry.logical_block_address)
                .unwrap_or(0),
            Err(_) => {
                warn!("Failed to parse MBR, assuming exFAT boot sector at sector 0");
                0
            }
        };

        // The MBR parse can "succeed" on garbage (e.g. an exFAT boot sector at
        // sector 0 also carries the 0x55AA signature, making its boot code parse
        // as a partition table), so verify that an exFAT boot sector actually
        // exists at the chosen offset before trusting it.
        let offset = if self.is_exfat_boot_sector(mbr_offset)? {
            mbr_offset
        } else if mbr_offset != 0 && self.is_exfat_boot_sector(0)? {
            warn!(
                "no exFAT boot sector at MBR partition offset {}, falling back to sector 0",
                mbr_offset
            );
            0
        } else {
            warn!(
                "no exFAT boot sector found at offset {} or sector 0; card may be unformatted",
                mbr_offset
            );
            mbr_offset
        };

        if offset == 0 {
            warn!("No MBR partition entry found, assuming exFAT boot sector at sector 0");
        } else {
            info!("exFAT partition offset: {}", offset);
        }

        self.partition_offset = Some(offset);

        let block_count = self.sd.num_blocks().map_err(SdBlockDeviceError::Sd)?;
        self.block_count = Some(block_count);

        Ok(())
    }

    /// Checks for the "EXFAT   " signature of an exFAT boot sector at `lba`.
    fn is_exfat_boot_sector(&mut self, lba: u32) -> Result<bool, SdBlockDeviceError> {
        let mut block = Block::new();
        self.sd
            .read(core::slice::from_mut(&mut block), BlockIdx(lba))
            .map_err(SdBlockDeviceError::Sd)?;
        Ok(&block.contents[3..11] == b"EXFAT   ")
    }
}

impl BlockDevice<512> for SdBlockDevice {
    type Error = SdBlockDeviceError;
    type Align = A4;

    fn read(
        &mut self,
        block_address: u32,
        data: &mut [Aligned<Self::Align, [u8; 512]>],
    ) -> Result<(), Self::Error> {
        self.ensure_init()?;
        let offset = self.partition_offset.unwrap();
        for (i, buf) in data.iter_mut().enumerate() {
            let mut block = Block::new();
            self.sd
                .read(
                    core::slice::from_mut(&mut block),
                    BlockIdx(offset + block_address + i as u32),
                )
                .map_err(SdBlockDeviceError::Sd)?;
            buf.copy_from_slice(&block.contents);
        }
        Ok(())
    }

    fn write(
        &mut self,
        block_address: u32,
        data: &[Aligned<Self::Align, [u8; 512]>],
    ) -> Result<(), Self::Error> {
        self.ensure_init()?;
        let offset = self.partition_offset.unwrap();
        for (i, buf) in data.iter().enumerate() {
            let mut block = Block::new();
            block.contents.copy_from_slice(&buf[..]);
            self.sd
                .write(
                    core::slice::from_ref(&block),
                    BlockIdx(offset + block_address + i as u32),
                )
                .map_err(SdBlockDeviceError::Sd)?;
        }
        Ok(())
    }

    fn size(&mut self) -> Result<u64, Self::Error> {
        self.ensure_init()?;
        let count = self.block_count.unwrap();
        Ok(u64::from(count.0) * 512)
    }
}

impl FormatBlockDevice for SdBlockDevice {
    type Error = SdBlockDeviceError;

    fn sector_size(&self) -> usize {
        512
    }

    fn sector_count(&self) -> u64 {
        u64::from(self.block_count.unwrap().0)
    }

    fn read_sector(&mut self, lba: u64, out: &mut [u8]) -> Result<(), Self::Error> {
        if !self.is_card_inserted() {
            return Err(SdBlockDeviceError::CardNotInserted);
        }

        let mut block = Block::new();
        self.sd
            .read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
            .map_err(SdBlockDeviceError::Sd)?;
        out.copy_from_slice(&block.contents);
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), Self::Error> {
        if !self.is_card_inserted() {
            return Err(SdBlockDeviceError::CardNotInserted);
        }

        let mut block = Block::new();
        block.contents.copy_from_slice(data);
        self.sd
            .write(core::slice::from_ref(&block), BlockIdx(lba as u32))
            .map_err(SdBlockDeviceError::Sd)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Writes an exFAT Volume Label directory entry (type 0x83) into the root
/// directory of a freshly formatted volume.
///
/// `exfat_embedded::format_exfat` creates a single partition at LBA 1 and a
/// root directory containing only the allocation bitmap (0x81) and up-case
/// table (0x82) entries. The Volume Label entry is optional per the exFAT
/// specification (section 7.3: "the valid number of Volume Label directory
/// entries ranges from 0 to 1"), but exfat-slim requires one to mount.
fn write_volume_label(device: &mut SdBlockDevice, label: &str) -> Result<(), FormatError> {
    // exfat-embedded always places the partition (and thus the boot sector) at LBA 1
    const PARTITION_LBA: u64 = 1;
    const ENTRY_LEN: usize = 32;

    let mut sector = [0u8; 512];
    device
        .read_sector(PARTITION_LBA, &mut sector)
        .map_err(FormatError::Device)?;

    // boot sector field offsets per exFAT specification, table 4
    let cluster_heap_offset = u32::from_le_bytes(unwrap!(sector[88..92].try_into()));
    let root_cluster = u32::from_le_bytes(unwrap!(sector[96..100].try_into()));
    let sectors_per_cluster_shift = sector[109];

    if &sector[3..11] != b"EXFAT   " || root_cluster < 2 {
        warn!("freshly formatted volume has an invalid boot sector");
        return Err(FormatError::Formatter);
    }

    let root_dir_lba = PARTITION_LBA
        + u64::from(cluster_heap_offset)
        + (u64::from(root_cluster - 2) << sectors_per_cluster_shift);

    device
        .read_sector(root_dir_lba, &mut sector)
        .map_err(FormatError::Device)?;

    // find the first free slot: end-of-directory (0x00) or unused (0x01..=0x7f) entry
    let Some(slot) = sector
        .chunks_exact(ENTRY_LEN)
        .position(|entry| entry[0] < 0x80)
    else {
        warn!("no free directory entry for the volume label");
        return Err(FormatError::Formatter);
    };

    let entry = &mut sector[slot * ENTRY_LEN..(slot + 1) * ENTRY_LEN];
    entry.fill(0);
    entry[0] = 0x83; // Volume Label directory entry
    let mut character_count = 0;
    for (i, unit) in label.encode_utf16().take(11).enumerate() {
        entry[2 + i * 2..4 + i * 2].copy_from_slice(&unit.to_le_bytes());
        character_count += 1;
    }
    entry[1] = character_count;

    device
        .write_sector(root_dir_lba, &sector)
        .map_err(FormatError::Device)
}
