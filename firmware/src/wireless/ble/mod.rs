mod advertise;
mod gatt;

use super::BtController;
use crate::sdcard::SdCardStorage;
use bled_core::DeviceConfig;
use cyw43::Control;
use defmt::{error, info};
use embassy_futures::join::join;
use trouble_host::prelude::*;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

#[embassy_executor::task]
pub(crate) async fn task(
    controller: BtController,
    address: Address,
    config: DeviceConfig,
    sd: SdCardStorage,
) {
    info!("Our BT address = {:?}", address);

    let device_name = config.name.clone();

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .build();
    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();

    let server = gatt::Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: &device_name,
        appearance: &appearance::UNKNOWN,
    }))
    .unwrap();

    let runner_fut = async {
        loop {
            if let Err(e) = runner.run().await {
                error!("BLE runner error: {}", e);
            }
        }
    };

    let srv_fut = async {
        loop {
            let mut srv = async || -> Result<(), BleHostError<_>> {
                let conn = advertise::advertise(&device_name, &mut peripheral).await?;
                let conn = conn.with_attribute_server(&server)?;
                gatt::gatt_events_task(&server, &conn, sd.clone(), &config).await;
                Ok(())
            };

            if let Err(e) = srv().await {
                error!("error: {}", e);
            }
        }
    };

    let _ = join(runner_fut, srv_fut).await;
}

pub(crate) async fn derive_address(control: &mut Control<'static>) -> Address {
    let mut address = control.address().await;
    // Convert the Wi-Fi identity into a stable random-static BLE address.
    address.reverse();
    address[5] = (address[5] & 0x3f) | 0xc0;
    Address::random(address)
}
