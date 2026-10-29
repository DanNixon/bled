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
        pwr: PIN_13,
        cs: PIN_14,
        dio: PIN_15,
        clk: PIN_12,
    },
    led: LedResources {
        pio: PIO1,
        channel_0_dma: DMA_CH2,
        channel_0_pin: PIN_8,
        channel_1_dma: DMA_CH3,
        channel_1_pin: PIN_9,
        channel_2_dma: DMA_CH4,
        channel_2_pin: PIN_10,
        channel_3_dma: DMA_CH5,
        channel_3_pin: PIN_11,
    },
    buttons: ButtonResources {
        a: PIN_24,
        b: PIN_23,
        c: PIN_22,
    },
    sdcard: SdCardResources {
        spi: SPI0,
        sck: PIN_6,
        mosi: PIN_7,
        miso: PIN_4,
        cs: PIN_5,
        detect: PIN_27, // TODO: make SD driver optionally require a detect pin
    },
    bms: BmsResources {
       i2c: I2C0,
       sda: PIN_0,
       scl: PIN_1,
       int: PIN_2,
       otg: PIN_3,
    },
}

embassy_rp::bind_interrupts!(pub(crate) struct Irqs {
    I2C0_IRQ => embassy_rp::i2c::InterruptHandler<peripherals::I2C0>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>,
                 dma::InterruptHandler<DMA_CH1>,
                 dma::InterruptHandler<DMA_CH2>,
                 dma::InterruptHandler<DMA_CH3>,
                 dma::InterruptHandler<DMA_CH4>,
                 dma::InterruptHandler<DMA_CH5>;
});
