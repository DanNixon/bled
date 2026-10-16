use crate::SdCardResources;
use aligned::{A4, Aligned};
use defmt::{Format, info, unwrap, warn};
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    mode::Blocking,
    spi::{Config, Spi},
    time::Hertz,
};
use embassy_time::Delay;
use embassy_usb::class::msc::device::BlockDevice as MscBlockDevice;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{Block, BlockCount, BlockDevice as _, BlockIdx, SdCard as SpiSdCard};
use exfat_embedded::{BlockDevice as FormatBlockDevice, Scratch};
use exfat_slim::blocking::BlockDevice;
use mbr_nostd::{MasterBootRecord, PartitionTable};

pub(crate) type SdCard =
    SpiSdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>, Delay>;

#[derive(Debug, Format)]
pub(crate) enum SdBlockDeviceError {
    /// No SD card is inserted in the slot.
    CardNotInserted,

    /// Error while accessing the SD card.
    Sd(<SdCard as embedded_sdmmc::BlockDevice>::Error),
}

#[derive(Debug, Format)]
pub(crate) enum FormatError {
    Device(SdBlockDeviceError),
    Formatter,
}

pub(crate) struct SdBlockDevice {
    detect: Input<'static>,
    sd: SdCard,
    block_count: Option<BlockCount>,
    partition_offset: Option<u32>,
}

impl SdBlockDevice {
    pub(crate) fn new(r: SdCardResources) -> SdBlockDevice {
        let detect = Input::new(r.detect, Pull::Up);

        let mut config = Config::default();
        config.frequency = Hertz(16_000_000);
        let spi = Spi::new_blocking(r.spi, r.sck, r.mosi, r.miso, config).unwrap();
        let cs = Output::new(r.cs, Level::High);
        let spi = unwrap!(ExclusiveDevice::new(spi, cs, Delay));
        let sd = SpiSdCard::new(spi, Delay);

        SdBlockDevice {
            detect,
            sd,
            block_count: None,
            partition_offset: None,
        }
    }

    fn is_card_inserted(&self) -> bool {
        self.detect.is_low()
    }

    pub(crate) fn ensure_init(&mut self) -> Result<(), SdBlockDeviceError> {
        if !self.is_card_inserted() {
            return Err(SdBlockDeviceError::CardNotInserted);
        }

        if self.partition_offset.is_some() && self.block_count.is_some() {
            return Ok(());
        }

        let block_count = self.sd.num_blocks().map_err(SdBlockDeviceError::Sd)?;
        self.block_count = Some(block_count);

        Ok(())
    }

    fn ensure_init_fs(&mut self) -> Result<(), SdBlockDeviceError> {
        self.ensure_init()?;

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

    #[inline(never)]
    pub(crate) async fn format(&mut self) -> Result<(), FormatError> {
        info!("formatting sdcard");

        self.ensure_init().map_err(FormatError::Device)?;

        let mut scratch = [0u8; 512];
        let result = exfat_embedded::format_exfat(self, &mut Scratch::new(&mut scratch))
            .map_err(|error| match error {
                exfat_embedded::Error::Device(error) => FormatError::Device(error),
                _ => FormatError::Formatter,
            })
            // exfat-embedded does not write a volume label directory entry, but
            // exfat-slim refuses to mount volumes without one, so add it ourselves.
            .and_then(|()| write_volume_label(self, "BLED"));

        // The partition layout may have changed; re-detect it on next access.
        self.partition_offset = None;

        info!("formatting sdcard done");
        result
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
        self.ensure_init_fs()?;
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
        self.ensure_init_fs()?;
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
        self.ensure_init_fs()?;
        let count = self.block_count.unwrap();
        Ok(u64::from(count.0) * 512)
    }
}

impl MscBlockDevice for SdBlockDevice {
    type Error = SdBlockDeviceError;

    fn block_size(&self) -> u32 {
        self.sector_size().try_into().unwrap()
    }

    fn block_count(&self) -> u32 {
        self.sector_count().try_into().unwrap()
    }

    fn read_blocks(&mut self, lba: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.ensure_init()?;

        let blocks = buf.len() / self.block_size() as usize;

        for block in 0..blocks {
            let mut block_data = Block::new();
            self.sd
                .read(
                    core::slice::from_mut(&mut block_data),
                    BlockIdx(lba + block as u32),
                )
                .map_err(SdBlockDeviceError::Sd)?;

            let start = block * self.block_size() as usize;
            let end = start + self.block_size() as usize;
            buf[start..end].copy_from_slice(&block_data.contents);
        }

        Ok(())
    }

    fn write_blocks(&mut self, lba: u32, buf: &[u8]) -> Result<(), Self::Error> {
        self.ensure_init()?;

        let blocks = buf.len() / self.block_size() as usize;

        for block in 0..blocks {
            let start = block * self.block_size() as usize;
            let end = start + self.block_size() as usize;

            let mut block_data = Block::new();
            block_data.contents.copy_from_slice(&buf[start..end]);
            self.sd
                .write(
                    core::slice::from_ref(&block_data),
                    BlockIdx(lba + block as u32),
                )
                .map_err(SdBlockDeviceError::Sd)?;
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
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
        self.ensure_init()?;

        let mut block = Block::new();
        self.sd
            .read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
            .map_err(SdBlockDeviceError::Sd)?;
        out.copy_from_slice(&block.contents);
        Ok(())
    }

    fn write_sector(&mut self, lba: u64, data: &[u8]) -> Result<(), Self::Error> {
        self.ensure_init()?;

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
        .as_chunks::<ENTRY_LEN>()
        .0
        .iter()
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
