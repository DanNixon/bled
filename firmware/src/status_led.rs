#[cfg(feature = "board-pico2w")]
use cyw43::Control;
#[cfg(feature = "board-pico2")]
use embassy_rp::{
    Peri,
    gpio::{Level, Output},
    peripherals::PIN_25,
};

#[cfg(feature = "board-pico2")]
pub(crate) struct StatusLed {
    pin: Output<'static>,
}

#[cfg(feature = "board-pico2w")]
pub(crate) struct StatusLed {
    control: Control<'static>,
}

#[cfg(feature = "board-pico2")]
impl StatusLed {
    pub(crate) fn new(pin: Peri<'static, PIN_25>) -> Self {
        Self {
            pin: Output::new(pin, Level::Low),
        }
    }

    pub(crate) async fn set(&mut self, on: bool) {
        self.pin
            .set_level(if on { Level::High } else { Level::Low });
    }
}

#[cfg(feature = "board-pico2w")]
impl StatusLed {
    pub(crate) fn new(control: Control<'static>) -> Self {
        Self { control }
    }

    pub(crate) async fn set(&mut self, on: bool) {
        self.control.gpio_set(0, on).await;
    }
}
