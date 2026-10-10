#![no_std]
#![no_main]

mod api_transport;
mod buttons;
mod config;
mod leds;
mod sdcard;
mod status;
mod wireless;

use crate::sdcard::SdCardStorage;
use bled_core::api::{BootReason, DeviceInfo};
use defmt::{error, info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    Peri, dma,
    peripherals::{self, DMA_CH0, DMA_CH1, DMA_CH2, DMA_CH3, DMA_CH4, DMA_CH5, PIO0, PIO1},
    pio,
};
use embassy_time::{Duration, Instant};
use panic_probe as _;
use portable_atomic as _;

assign_resources::assign_resources! {
    wireless: WirelessResources {
        pio: PIO0,
        tx_dma: DMA_CH0,
        rx_dma: DMA_CH1,
        pwr: PIN_23,
        cs: PIN_25,
        dio: PIN_24,
        clk: PIN_29,
    },
    led: LedResources {
        pio: PIO1,
        channel_0_dma: DMA_CH2,
        channel_0_pin: PIN_21,
        channel_1_dma: DMA_CH3,
        channel_1_pin: PIN_20,
        channel_2_dma: DMA_CH4,
        channel_2_pin: PIN_8,
        channel_3_dma: DMA_CH5,
        channel_3_pin: PIN_7,
    },
    buttons: ButtonResources {
        a: PIN_17,
        b: PIN_16,
        c: PIN_15,
    },
    sdcard: SdCardResources {
        spi: SPI1,
        sck: PIN_10,
        mosi: PIN_11,
        miso: PIN_12,
        cs: PIN_9,
        detect: PIN_22,
    },
}

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>,
                 dma::InterruptHandler<DMA_CH1>,
                 dma::InterruptHandler<DMA_CH2>,
                 dma::InterruptHandler<DMA_CH3>,
                 dma::InterruptHandler<DMA_CH4>,
                 dma::InterruptHandler<DMA_CH5>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let r = split_resources!(p);

    info!("{}", device_info());

    let (mut control, bt) = wireless::init(r.wireless, spawner).await;
    let bt_address = wireless::ble::derive_address(&mut control).await;

    let mut button_events = buttons::button_events_subscriber();
    buttons::init(r.buttons, spawner);

    let sd = SdCardStorage::new(r.sdcard);
    if buttons::is_held_at_boot(
        &mut button_events,
        buttons::Button::A,
        Duration::from_secs(1),
    )
    .await
    {
        // Status LED on while formatting
        control.gpio_set(0, true).await;

        if let Err(e) = sd.format().await {
            error!("Failed to format SD card: {}", e);
        }

        // Status LED off after formatting
        control.gpio_set(0, false).await;
    }

    let config = config::boot_time_load(&sd).await;

    spawner.spawn(unwrap!(wireless::ble::task(
        bt,
        bt_address,
        config.clone(),
        sd.clone()
    )));

    spawner.spawn(unwrap!(leds::task(r.led, config.channels)));
    leds::draw().await;

    spawner.spawn(unwrap!(status::task(control, p.WATCHDOG)));
}

fn device_info() -> DeviceInfo {
    DeviceInfo::new(
        git_version::git_version!().try_into().unwrap(),
        boot_reason(),
        Instant::now().as_millis(),
        leds::LED_MEMORY.try_into().unwrap(),
        4, // TODO: this should be a constant
    )
}

fn boot_reason() -> BootReason {
    let reason = embassy_rp::pac::WATCHDOG.reason().read();

    if reason.force() {
        BootReason::WatchdogForced
    } else if reason.timer() {
        BootReason::WatchdogTimeout
    } else {
        BootReason::Normal
    }
}
