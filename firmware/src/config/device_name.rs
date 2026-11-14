use super::TextConfigFile;
use crate::sdcard::SdCardStorage;
use bled_core::DeviceName;
use defmt::info;
use embassy_sync::once_lock::OnceLock;

static DEVICE_NAME: OnceLock<DeviceName> = OnceLock::new();

#[inline(never)]
pub(super) async fn init(sd: &SdCardStorage) {
    let name: DeviceName = super::load(sd).await;
    info!("Device name: {}", name);
    DEVICE_NAME
        .init(name)
        .expect("device name initialized more than once");
}

pub(crate) fn device_name() -> &'static DeviceName {
    DEVICE_NAME
        .try_get()
        .expect("device name accessed before initialization")
}

impl TextConfigFile<64> for DeviceName {
    const PATH: &'static str = "/device-name.txt";
    const OLD_PATH: &'static str = "/device-name-OLD.txt";
}
