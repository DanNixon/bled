use embassy_rp::{
    Peri, dma,
    peripherals::{self, DMA_CH0, DMA_CH1, DMA_CH2, DMA_CH3, DMA_CH4, DMA_CH5, PIO0, PIO1},
    pio,
};

assign_resources::assign_resources! {
    wireless: WirelessResources {
        pio: PIO0,
        tx_dma: DMA_CH0,
        rx_dma: DMA_CH1,
        pwr: PIN_23,
        cs: PIN_25,
        dio: PIN_24,
        clk: PIN_29,
    },
    led: LedResources {
        pio: PIO1,
        channel_0_dma: DMA_CH2,
        channel_0_pin: PIN_21,
        channel_1_dma: DMA_CH3,
        channel_1_pin: PIN_20,
        channel_2_dma: DMA_CH4,
        channel_2_pin: PIN_8,
        channel_3_dma: DMA_CH5,
        channel_3_pin: PIN_7,
    },
    buttons: ButtonResources {
        a: PIN_17,
        b: PIN_16,
        c: PIN_15,
    },
    sdcard: SdCardResources {
        spi: SPI1,
        sck: PIN_10,
        mosi: PIN_11,
        miso: PIN_12,
        cs: PIN_9,
        detect: PIN_22,
    },
}

embassy_rp::bind_interrupts!(pub(crate) struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>,
                 dma::InterruptHandler<DMA_CH1>,
                 dma::InterruptHandler<DMA_CH2>,
                 dma::InterruptHandler<DMA_CH3>,
                 dma::InterruptHandler<DMA_CH4>,
                 dma::InterruptHandler<DMA_CH5>;
});
