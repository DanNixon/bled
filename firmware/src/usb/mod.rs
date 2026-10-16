pub(super) mod msc;

use embassy_rp::{
    Peri, bind_interrupts,
    peripherals::USB,
    usb::{Driver, InterruptHandler},
};
use embassy_usb::Builder;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

pub(super) fn init(usb: Peri<'static, USB>) -> Builder<'static, Driver<'static, USB>> {
    let driver = Driver::new(usb, Irqs);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Dan Nixon");
    config.product = Some("BLED");
    config.serial_number = Some(embassy_rp::uid::uid_hex());
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    let config_descriptor = CONFIG_DESCRIPTOR.init([0; 256]);

    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    let bos_descriptor = BOS_DESCRIPTOR.init([0; 256]);

    let control_buf = CONTROL_BUF.init([0; 64]);
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    Builder::new(
        driver,
        config,
        config_descriptor,
        bos_descriptor,
        &mut [],
        control_buf,
    )
}

#[embassy_executor::task]
pub(super) async fn task(usb: Builder<'static, Driver<'static, USB>>) -> ! {
    let mut usb = usb.build();
    usb.run().await;
}
