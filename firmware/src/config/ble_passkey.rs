use super::TextConfigFile;
use crate::sdcard::SdCardStorage;
use bled_core::ble::BlePasskey;
use defmt::info;

#[inline(never)]
pub(crate) async fn load_ble_passkey(sd: &SdCardStorage) -> BlePasskey {
    let passkey = super::load(sd).await;
    info!("BLE passkey: {}", passkey);
    passkey
}

impl TextConfigFile<32> for BlePasskey {
    const PATH: &'static str = "/ble-passkey.txt";
    const OLD_PATH: &'static str = "/ble-passkey-OLD.txt";
}
