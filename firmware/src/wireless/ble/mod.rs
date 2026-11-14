mod advertise;
mod bonds;
mod serial;

use super::BtController;
use crate::sdcard::SdCardStorage;
use cyw43::Control;
use defmt::{error, info, warn};
use embassy_futures::join::join;
use trouble_host::prelude::*;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

#[embassy_executor::task]
pub(crate) async fn task(controller: BtController, address: Address, sd: SdCardStorage) {
    info!("Our BT address = {:?}", address);

    let device_name = crate::config::device_name().as_str();

    let mut resources: HostResources<
        DefaultPacketPool,
        CONNECTIONS_MAX,
        L2CAP_CHANNELS_MAX,
        1,
        { bonds::BONDS_MAX },
    > = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .set_io_capabilities(IoCapabilities::DisplayOnly)
        .build();

    bonds::load(&stack, &sd).await;

    let mut runner = stack.runner();
    let mut peripheral = stack.peripheral();

    let server = serial::Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: device_name,
        appearance: &appearance::UNKNOWN,
    }))
    .unwrap();

    let rx_pipe = serial::init_rx_pipe();

    let runner_fut = async {
        loop {
            if let Err(e) = runner.run().await {
                error!("BLE runner error: {}", e);
            }
        }
    };

    let srv_fut = async {
        loop {
            match advertise::advertise(device_name, &mut peripheral).await {
                Ok(conn) => {
                    let passkey = *crate::config::load_ble_passkey(&sd).await;
                    stack.set_passkey(Some(passkey));

                    let conn = conn.with_attribute_server(&server).unwrap();
                    if let Err(e) = conn.raw().set_bondable(true) {
                        warn!("Failed to mark BLE connection as bondable: {:?}", e);
                    }
                    serial::run_connection(&stack, &server, &conn, rx_pipe, &sd).await;
                }
                Err(e) => error!("BLE advertising error: {}", e),
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
