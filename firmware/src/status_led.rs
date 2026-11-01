#[cfg(feature = "_status_light_cyw43_gpio0")]
use cyw43::Control;
#[cfg(feature = "_status_light_gpio25")]
use embassy_rp::{
    Peri,
    gpio::{Level, Output},
    peripherals::PIN_25,
};

#[cfg(feature = "_status_light_gpio25")]
pub(crate) struct StatusLed {
    pin: Output<'static>,
}

#[cfg(feature = "_status_light_cyw43_gpio0")]
pub(crate) struct StatusLed {
    control: Control<'static>,
}

#[cfg(feature = "_status_light_gpio25")]
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

#[cfg(feature = "_status_light_cyw43_gpio0")]
impl StatusLed {
    pub(crate) fn new(control: Control<'static>) -> Self {
        Self { control }
    }

    pub(crate) async fn set(&mut self, on: bool) {
        self.control.gpio_set(0, on).await;
    }
}
