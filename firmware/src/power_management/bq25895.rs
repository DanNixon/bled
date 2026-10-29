//! Support for the BQ25895 BMS and boost converter on the BLED Stamp board.
//!
//! TODO: actual integration with the rest of the firmware

use crate::{Bq25895Resources, Irqs};
use bq25895::{AdcConversionRate, Bq25895, Interface};
use defmt::{error, info};
use embassy_rp::{
    gpio::{Input, Level, Output, Pull},
    i2c::{Config, I2c},
};
use embassy_time::{Duration, Ticker};

#[embassy_executor::task]
pub(crate) async fn task(r: Bq25895Resources) -> ! {
    let i2c = I2c::new(r.i2c, r.scl, r.sda, Irqs, Config::default());
    let _interrupt = Input::new(r.int, Pull::Up);

    // Enable OTG/boost converter to supply the LEDs with 5V when on battery power.
    let _otg_enable = Output::new(r.otg, Level::High);

    let mut bms = Bq25895::new(Interface::new(i2c));

    if let Err(e) = bms
        .reg_02()
        .modify_async(|r| r.set_conv_rate(AdcConversionRate::Continuous))
        .await
    {
        error!("Failed to configure readout: {}", e);
    }

    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        ticker.next().await;

        let charge_state = bms.reg_0_b().read_async().await.map(|r| r.chrg_stat().ok());
        let battery_voltage = bms.reg_0_e().read_async().await.map(|r| r.batv().ok());
        let system_voltage = bms.reg_0_f().read_async().await.map(|r| r.sysv().ok());
        let charge_current = bms.reg_12().read_async().await.map(|r| r.ichgr().ok());

        info!(
            "BMS: state={}, bat={}mV, sys={}mV, ichg={}mA",
            charge_state, battery_voltage, system_voltage, charge_current
        );
    }
}
