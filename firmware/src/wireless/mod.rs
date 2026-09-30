pub(super) mod ble;

use crate::{Irqs, WirelessResources};
use bt_hci::controller::ExternalController;
use cyw43::{Control, aligned_bytes, bluetooth::BtDriver};
#[cfg(feature = "picow")]
use cyw43_pio::DEFAULT_CLOCK_DIVIDER as CLOCK_DIVIDER;
use cyw43_pio::PioSpi;
#[cfg(feature = "pico2w")]
use cyw43_pio::RM2_CLOCK_DIVIDER as CLOCK_DIVIDER;
use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_rp::{
    dma,
    gpio::{Level, Output},
    peripherals::PIO0,
    pio::Pio,
};
use static_cell::StaticCell;

pub(super) async fn init(
    r: WirelessResources,
    spawner: Spawner,
) -> (Control<'static>, BtController) {
    let fw = aligned_bytes!("../../cyw43-firmware/43439A0.bin");
    let clm = aligned_bytes!("../../cyw43-firmware/43439A0_clm.bin");
    let btfw = aligned_bytes!("../../cyw43-firmware/43439A0_btfw.bin");
    let nvram = aligned_bytes!("../../cyw43-firmware/nvram_rp2040.bin");

    let pwr = Output::new(r.pwr, Level::Low);
    let cs = Output::new(r.cs, Level::High);
    let mut pio = Pio::new(r.pio, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        CLOCK_DIVIDER,
        pio.irq0,
        cs,
        r.dio,
        r.clk,
        dma::Channel::new(r.tx_dma, Irqs),
        dma::Channel::new(r.rx_dma, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let bt_controller: ExternalController<_, 10> = ExternalController::new(bt_device);

    (control, bt_controller)
}

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
        cyw43::Cyw43439,
    >,
) -> ! {
    runner.run().await
}

pub(crate) type BtController = ExternalController<BtDriver<'static>, 10>;
