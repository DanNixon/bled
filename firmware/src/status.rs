use crate::status_led::StatusLed;
use embassy_futures::select::{Either, select};
use embassy_rp::{Peri, peripherals::WATCHDOG, watchdog::Watchdog};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Ticker, Timer};

static RESET: Channel<CriticalSectionRawMutex, (), 1> = Channel::new();

/// Trigger a reset.
pub(crate) fn reset() {
    let _ = RESET.try_send(());
}

#[embassy_executor::task]
pub(super) async fn task(mut led: StatusLed, wdt: Peri<'static, WATCHDOG>) -> ! {
    let mut wdt = Watchdog::new(wdt);
    wdt.start(Duration::from_secs(3));

    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        led.set(true).await;
        Timer::after_millis(50).await;
        led.set(false).await;

        wdt.feed(Duration::from_secs(2));

        match select(RESET.receive(), ticker.next()).await {
            Either::First(_) => {
                Timer::after_millis(500).await;
                wdt.trigger_reset();
            }
            Either::Second(_) => {}
        }
    }
}
