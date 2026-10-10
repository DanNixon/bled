use bled_core::{
    RGB8,
    pixel_data::{PixelDataBuffer, PixelDataChannelSize},
};
use defmt::{debug, info, unwrap};
use embassy_rp::{
    pio::Pio,
    pio_programs::ws2812::{PioWs2812, PioWs2812Program},
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
    mutex::{Mutex, MutexGuard},
    signal::Signal,
};
use heapless::Vec;
use smart_leds::SmartLedsWriteAsync;

pub(crate) const LED_MEMORY: usize = 16 * 1024;

static CHANNEL_DATA: Mutex<CriticalSectionRawMutex, PixelDataBuffer<LED_MEMORY>> =
    Mutex::new(PixelDataBuffer::new());

static DRAW_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
static DRAW_REQUEST: Channel<CriticalSectionRawMutex, (), 1> = Channel::new();
static DRAW_COMPLETE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub(crate) async fn channel_data()
-> MutexGuard<'static, CriticalSectionRawMutex, PixelDataBuffer<LED_MEMORY>> {
    CHANNEL_DATA.lock().await
}

/// Renders the selected channels and returns after the physical write completes.
pub(crate) async fn draw() {
    debug!("Draw requested");
    let _guard = DRAW_MUTEX.lock().await;
    DRAW_REQUEST.send(()).await;
    DRAW_COMPLETE.wait().await;
    debug!("Draw complete");
}

#[embassy_executor::task]
pub(super) async fn task(
    r: super::LedResources,
    config: Vec<ChannelConfig, { MAX_CHANNEL_COUNT }>,
) -> ! {
    let channel_sizes: Vec<PixelDataChannelSize, { MAX_CHANNEL_COUNT }> = config
        .iter()
        .map(|c| PixelDataChannelSize::new::<RGB8>(c.len() as usize))
        .collect();
    unwrap!(CHANNEL_DATA.lock().await.try_reshape(&channel_sizes));

    let mut pio = Pio::new(r.pio, super::Irqs);
    let program = PioWs2812Program::new(&mut pio.common);

    let mut channel_0 = PioWs2812::new(
        &mut pio.common,
        pio.sm0,
        r.channel_0_dma,
        crate::Irqs,
        r.channel_0_pin,
        &program,
    );
    let mut channel_1 = PioWs2812::new(
        &mut pio.common,
        pio.sm1,
        r.channel_1_dma,
        crate::Irqs,
        r.channel_1_pin,
        &program,
    );
    let mut channel_2 = PioWs2812::new(
        &mut pio.common,
        pio.sm2,
        r.channel_2_dma,
        crate::Irqs,
        r.channel_2_pin,
        &program,
    );
    let mut channel_3 = PioWs2812::new(
        &mut pio.common,
        pio.sm3,
        r.channel_3_dma,
        crate::Irqs,
        r.channel_3_pin,
        &program,
    );

    loop {
        let mask = DRAW_REQUEST.receive().await;
        info!("Draw request: {}", mask);

        let data = CHANNEL_DATA.lock().await;

        for channel in 0..4 {
            if let Ok(pixels) = data.try_channel::<RGB8>(channel) {
                match channel {
                    0 => channel_0.write(pixels.iter().copied()).await.unwrap(),
                    1 => channel_1.write(pixels.iter().copied()).await.unwrap(),
                    2 => channel_2.write(pixels.iter().copied()).await.unwrap(),
                    3 => channel_3.write(pixels.iter().copied()).await.unwrap(),
                    _ => unreachable!(),
                }
            }
        }

        DRAW_COMPLETE.signal(());
    }
}
