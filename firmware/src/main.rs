#![no_std]
#![no_main]
#![cfg_attr(not(feature = "_wireless"), allow(dead_code))]

extern crate alloc;

mod api;
mod boards;
mod buttons;
mod config;
mod effect;
mod leds;
mod sdcard;
mod status;
mod status_led;
mod usb;
#[cfg(feature = "_wireless")]
mod wireless;

use crate::boards::*;
use crate::sdcard::{SdBlockDevice, SdCardStorage};
use bled_core::api::{BootReason, DeviceInfo};
use defmt::{error, info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant};
use embedded_alloc::LlffHeap as Heap;
use panic_probe as _;
use portable_atomic as _;

const HEAP_SIZE: usize = 256 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    unsafe {
        embedded_alloc::init!(HEAP, HEAP_SIZE);
    }

    let p = embassy_rp::init(Default::default());
    let r = split_resources!(p);

    info!("{}", device_info());

    #[cfg(feature = "_wireless")]
    let (control, bt, bt_address) = {
        let (mut control, bt) = wireless::init(r.wireless, spawner).await;
        let bt_address = wireless::ble::derive_address(&mut control).await;
        (control, bt, bt_address)
    };

    #[cfg(feature = "_status_light_cyw43_gpio0")]
    let mut status_led = status_led::StatusLed::new(control);
    #[cfg(feature = "_status_light_gpio25")]
    let mut status_led = status_led::StatusLed::new(p.PIN_25);

    let mut button_events = buttons::button_events_subscriber();
    buttons::init(r.buttons, spawner);

    let mut sd = SdBlockDevice::new(r.sdcard);

    if buttons::is_held_at_boot(
        &mut button_events,
        buttons::Button::A,
        Duration::from_secs(1),
    )
    .await
    {
        // Status LED on while formatting
        status_led.set(true).await;

        if let Err(e) = sd.format().await {
            error!("Failed to format SD card: {}", e);
        }

        // Status LED off after formatting
        status_led.set(false).await;
    }

    let mut usb_builder = usb::init(p.USB);

    // Present SD card as USB MSC if button B is held at boot, otherwise run normally
    if buttons::is_held_at_boot(
        &mut button_events,
        buttons::Button::B,
        Duration::from_millis(100),
    )
    .await
    {
        // Status LED on when in MSC mode
        status_led.set(true).await;

        let msc = usb::msc::init(&mut usb_builder);
        spawner.spawn(unwrap!(usb::msc::task(msc, sd)));

        spawner.spawn(unwrap!(usb::task(usb_builder)));
    } else {
        #[cfg(any(feature = "board-stamp", feature = "board-stamp-rm2"))]
        spawner.spawn(unwrap!(stamp_bms::task(r.bms)));

        let sd = SdCardStorage::new(sd);

        config::init(&sd).await;

        effect::load_default_effect(&sd).await;

        let (sender, receiver, control) = usb::cdc_acm::init(&mut usb_builder);
        spawner.spawn(unwrap!(usb::cdc_acm::task(
            sender,
            receiver,
            control,
            sd.clone()
        )));

        spawner.spawn(unwrap!(usb::task(usb_builder)));

        #[cfg(feature = "_wireless")]
        spawner.spawn(unwrap!(wireless::ble::task(bt, bt_address, sd)));

        spawner.spawn(unwrap!(leds::task(r.led)));
        leds::draw().await;

        spawner.spawn(unwrap!(effect::task()));

        spawner.spawn(unwrap!(status::task(status_led, p.WATCHDOG)));
    }
}

fn device_info() -> DeviceInfo {
    DeviceInfo::new(
        git_version::git_version!().try_into().unwrap(),
        boot_reason(),
        Instant::now().as_millis(),
        leds::LED_MEMORY.try_into().unwrap(),
        leds::CHANNEL_COUNT.try_into().unwrap(),
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
