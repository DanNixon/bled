use bled_api::ble::SERVICE_UUID;
use defmt::info;
use trouble_host::prelude::*;

pub(super) async fn advertise<'a, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
) -> Result<Connection<'a, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let mut scan_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids128(&[SERVICE_UUID.as_u128().to_le_bytes()]),
        ],
        &mut advertiser_data[..],
    )?;
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(name.as_bytes())],
        &mut scan_data,
    )?;

    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &scan_data[..scan_len],
            },
        )
        .await?;
    info!("BLE advertising");

    let conn = advertiser.accept().await?;
    info!("BLE connection established");

    Ok(conn)
}
