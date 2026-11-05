use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    i2c::{Config, I2c},
};
use embassy_time::Timer;

#[embassy_executor::task]
pub(crate) async fn task(r: super::BmsResources) -> ! {
    let i2c = I2c::new(r.i2c, r.scl, r.sda, super::Irqs, Config::default());
    let interrupt = Input::new(r.int, Pull::Up);
    let otg_enable = Output::new(r.otg, Level::Low);

    // TODO
    loop {
        Timer::after_secs(10).await;
    }
}
