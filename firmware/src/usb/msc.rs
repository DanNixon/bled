use crate::SdBlockDevice;
use defmt::info;
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_time::Timer;
use embassy_usb::{
    Builder,
    class::msc::device::{Config as MscConfig, MscClass, State},
};
use static_cell::StaticCell;

pub(crate) fn init(
    builder: &mut Builder<'static, Driver<'static, USB>>,
) -> MscClass<'static, Driver<'static, USB>> {
    let msc_config = MscConfig::new(64)
        .vendor_id("Dan Nixon")
        .product_id("BLED")
        .product_revision_level("1.0")
        .serial_number(embassy_rp::uid::uid_hex())
        .double_buffered(true);

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());

    MscClass::new(builder, state, msc_config)
}

#[embassy_executor::task]
pub(crate) async fn task(
    mut msc: MscClass<'static, Driver<'static, USB>>,
    mut sd: SdBlockDevice,
) -> ! {
    info!("Starting USB MSC mode");

    while sd.ensure_init().is_err() {
        Timer::after_millis(100).await;
    }

    let mut block_buf = [0u8; 4 * 512];

    info!("USB MSC mode ready");
    loop {
        msc.run(&mut sd, &mut block_buf).await;
    }
}
