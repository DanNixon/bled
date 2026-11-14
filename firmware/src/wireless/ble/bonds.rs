//! Persistent storage of BLE bond information on the SD card.

use crate::sdcard::{ReadFileError, SdCardStorage};
use defmt::{info, warn};
use exfat_slim::blocking::error::ExFatError;
use heapless::Vec;
use trouble_host::prelude::*;

/// Maximum number of bonds tracked by the BLE stack.
pub(super) const BONDS_MAX: usize = 10;

/// Maximum number of bonds persisted on the SD card, leaving one slot free in
/// the stack for a newly pairing peer before eviction.
const STORED_BONDS_MAX: usize = BONDS_MAX - 1;

/// File name of the bonds file on the SD card.
const BONDS_PATH: &str = "/ble_bonds.bin";

/// Maximum serialized size of the bonds file in bytes.
const MAX_BONDS_POSTCARD_SIZE: usize = 512;

/// Loads persisted BLE bonds from the SD card into the BLE stack.
#[inline(never)]
pub(super) async fn load<C: Controller, P: PacketPool>(
    stack: &Stack<'_, C, P>,
    sd: &SdCardStorage,
) {
    let mut bytes = [0u8; MAX_BONDS_POSTCARD_SIZE];
    let bonds: Vec<BondInformation, BONDS_MAX> = match sd.read_file(BONDS_PATH, &mut bytes).await {
        Ok(len) => match bled_core::io::decode_postcard(&bytes[..len]) {
            Ok(bonds) => bonds,
            Err(e) => {
                warn!("Failed to parse BLE bonds from SD card: {}", e);
                return;
            }
        },
        Err(ReadFileError::Open(ExFatError::FileNotFound)) => {
            info!("No BLE bonds file found on SD card");
            return;
        }
        Err(e) => {
            warn!("Failed to read BLE bonds file from SD card: {}", e);
            return;
        }
    };

    let start = bonds.len().saturating_sub(STORED_BONDS_MAX);
    let mut loaded = 0usize;
    for bond in &bonds[start..] {
        if !bond.is_bonded {
            continue;
        }
        match stack.add_bond_information(bond.clone()) {
            Ok(()) => loaded += 1,
            Err(e) => warn!("Failed to add BLE bond to stack: {:?}", e),
        }
    }

    info!("Loaded {} BLE bond(s) from SD card", loaded);
}

/// Persists all bonded peers from the BLE stack to the SD card, evicting the
/// oldest bond if necessary to keep room for future pairings.
#[inline(never)]
pub(super) async fn save<C: Controller, P: PacketPool>(
    stack: &Stack<'_, C, P>,
    sd: &SdCardStorage,
) {
    let mut all_bonds: Vec<BondInformation, BONDS_MAX> = stack.with_bond_information(|bonds| {
        let mut list = Vec::new();
        for bond in bonds {
            let _ = list.push(bond.clone());
        }
        list
    });

    // Remove any non-bonded session entries or oldest bonds exceeding capacity
    // so the stack always has room to accept a new pairing.
    for bond in &all_bonds {
        if !bond.is_bonded {
            let _ = stack.remove_bond_information(bond.identity);
        }
    }
    all_bonds.retain(|bond| bond.is_bonded);

    while all_bonds.len() > STORED_BONDS_MAX {
        let evicted = all_bonds.remove(0);
        info!("Evicting oldest BLE bond: {}", evicted.identity);
        let _ = stack.remove_bond_information(evicted.identity);
    }

    let mut bytes = [0u8; MAX_BONDS_POSTCARD_SIZE];
    match bled_core::io::encode_postcard(&all_bonds, &mut bytes) {
        Ok(len) => {
            if sd.write_file(BONDS_PATH, &bytes[..len]).await.is_ok() {
                info!("Saved {} BLE bond(s) to SD card", all_bonds.len());
            }
        }
        Err(e) => warn!("Failed to serialize BLE bonds: {}", e),
    }
}
