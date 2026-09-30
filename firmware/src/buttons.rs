use crate::ButtonResources;
use defmt::{Format, info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_rp::gpio::{Input, Level, Pull};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    pubsub::{PubSubChannel, Subscriber},
};
use embassy_time::{Duration, Instant, Timer};
use getset::Getters;

static BUTTON_EVENTS: PubSubChannel<CriticalSectionRawMutex, ButtonEvent, 8, 4, 3> =
    PubSubChannel::new();

pub(crate) type ButtonEventsSubscriber =
    Subscriber<'static, CriticalSectionRawMutex, ButtonEvent, 8, 4, 3>;

pub(crate) fn button_events_subscriber() -> ButtonEventsSubscriber {
    BUTTON_EVENTS.subscriber().unwrap()
}

#[derive(Format, Clone, Copy, PartialEq, Eq, Getters)]
#[getset(get = "pub")]
pub(crate) struct ButtonEvent {
    timestamp: Instant,
    button: Button,
    count: PressCount,
    kind: ButtonEventType,
}

#[derive(Format, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonEventType {
    Pressed,
    Held { duration: Option<Duration> },
    Released { duration: Option<Duration> },
}

#[derive(Format, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Button {
    A,
    B,
    C,
}

#[derive(Default, Format, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PressCount(pub(crate) usize);

impl PressCount {
    fn increment(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    pub(crate) fn is_first(&self) -> bool {
        self.0 == 0
    }
}

/// Waits to determine if `button` was held for at least `duration` at boot time.
///
/// Returns `true` if the button was held continuously for `duration` starting at boot.
/// Returns `false` immediately if the button was not held at boot or released before `duration`.
pub(crate) async fn is_held_at_boot(
    subscriber: &mut ButtonEventsSubscriber,
    button: Button,
    duration: Duration,
) -> bool {
    let min_duration = duration
        .checked_sub(Duration::from_millis(50))
        .unwrap_or(duration);
    let check = async {
        loop {
            let event = subscriber.next_message_pure().await;
            if event.button == button && event.count.is_first() {
                match event.kind {
                    ButtonEventType::Held { duration: Some(d) } if d >= min_duration => {
                        return true;
                    }
                    ButtonEventType::Released { .. } => {
                        return false;
                    }
                    _ => {}
                }
            }
        }
    };

    match select(check, Timer::after(duration + Duration::from_millis(500))).await {
        Either::First(held) => held,
        Either::Second(_) => false,
    }
}

pub(crate) fn init(r: ButtonResources, spawner: Spawner) {
    let btn_a = Input::new(r.a, Pull::Up);
    let btn_b = Input::new(r.b, Pull::Up);
    let btn_c = Input::new(r.c, Pull::Up);

    spawner.spawn(unwrap!(button_task(btn_a, Button::A)));
    spawner.spawn(unwrap!(button_task(btn_b, Button::B)));
    spawner.spawn(unwrap!(button_task(btn_c, Button::C)));
}

#[embassy_executor::task(pool_size = 3)]
async fn button_task(mut pin: Input<'static>, button: Button) -> ! {
    let publisher = BUTTON_EVENTS.publisher().unwrap();

    let mut press_count = PressCount::default();

    // Assume the button is held initially
    let mut last_level = Level::Low;
    let mut last_level_timestamp = Instant::now();

    loop {
        // Wait for the pin to change level
        let level = match last_level {
            Level::Low => {
                loop {
                    // Publish Held events at fixed intervals while the button is held
                    match select(pin.wait_for_high(), Timer::after_millis(500)).await {
                        Either::First(_) => break Level::High,
                        Either::Second(_) => {
                            let now = Instant::now();
                            let event = ButtonEvent {
                                timestamp: now,
                                button,
                                count: press_count,
                                kind: ButtonEventType::Held {
                                    duration: Some(now - last_level_timestamp),
                                },
                            };
                            info!("{}", event);
                            publisher.publish(event).await;
                        }
                    }
                }
            }
            Level::High => {
                pin.wait_for_low().await;
                Level::Low
            }
        };
        let now = Instant::now();

        let event = match (last_level, level) {
            (Level::Low, Level::High) => {
                let event = ButtonEvent {
                    timestamp: now,
                    button,
                    count: press_count,
                    kind: ButtonEventType::Released {
                        duration: Some(now - last_level_timestamp),
                    },
                };
                press_count.increment();
                event
            }
            (Level::High, Level::Low) => ButtonEvent {
                timestamp: now,
                button,
                count: press_count,
                kind: ButtonEventType::Pressed,
            },
            _ => unreachable!(),
        };

        info!("{}", event);
        publisher.publish(event).await;

        last_level = level;
        last_level_timestamp = now;

        // Wait for a little bit (as a crude debounce)
        Timer::after_millis(10).await;
    }
}
