use crate::{api, sdcard::SdCardStorage};
use bled_core::api::RPC_FRAME_BUF_LEN;
use core::cmp::min;
use defmt::info;
use embassy_futures::select::{Either, select};
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_usb::{
    Builder,
    class::cdc_acm::device::{
        BufferedReceiver, CdcAcmClass, CdcAcmError, ControlChanged, Sender, State,
    },
};
use embedded_io_async::{ErrorType, Read, Write};
use static_cell::StaticCell;
use teeny_rpc::transport::EioTransport;

const MAX_PACKET_SIZE: usize = 64;

pub(crate) fn init(
    builder: &mut Builder<'static, Driver<'static, USB>>,
) -> (
    Sender<'static, Driver<'static, USB>>,
    BufferedReceiver<'static, Driver<'static, USB>>,
    ControlChanged<'static>,
) {
    static STATE: StaticCell<State<'static>> = StaticCell::new();
    static RX_BUFFER: StaticCell<[u8; MAX_PACKET_SIZE]> = StaticCell::new();

    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(builder, state, MAX_PACKET_SIZE as u16);
    let (sender, receiver, control) = class.split_with_control();
    let receiver = receiver.into_buffered(RX_BUFFER.init([0; MAX_PACKET_SIZE]));

    (sender, receiver, control)
}

#[embassy_executor::task]
pub(crate) async fn task(
    sender: Sender<'static, Driver<'static, USB>>,
    receiver: BufferedReceiver<'static, Driver<'static, USB>>,
    control: ControlChanged<'static>,
    sd: SdCardStorage,
) {
    let mut io = CdcIo { sender, receiver };

    loop {
        wait_for_dtr(&control, true).await;
        info!("USB CDC RPC session opened");

        let serving = api::serve(
            EioTransport::<_, RPC_FRAME_BUF_LEN>::new(&mut io),
            sd.clone(),
        );
        let disconnected = wait_for_dtr(&control, false);

        match select(serving, disconnected).await {
            Either::First(()) => wait_for_dtr(&control, false).await,
            Either::Second(()) => {}
        }

        info!("USB CDC RPC session closed");
    }
}

async fn wait_for_dtr(control: &ControlChanged<'_>, connected: bool) {
    while control.dtr() != connected {
        control.control_changed().await;
    }
}

struct CdcIo {
    sender: Sender<'static, Driver<'static, USB>>,
    receiver: BufferedReceiver<'static, Driver<'static, USB>>,
}

impl ErrorType for CdcIo {
    type Error = CdcAcmError;
}

impl Read for CdcIo {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        // USB ZLPs carry no stream data, so keep reading instead of exposing EOF.
        loop {
            let read = self.receiver.read(buf).await?;
            if read != 0 {
                return Ok(read);
            }
        }
    }
}

impl Write for CdcIo {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        let max_packet_size = self.sender.max_packet_size() as usize;
        let length = min(buf.len(), max_packet_size);
        self.sender
            .write_packet(&buf[..length])
            .await
            .map_err(|_| CdcAcmError::NotConnected)?;

        // A full final packet needs a ZLP to delimit the USB bulk transfer.
        if buf.len() == length && length == max_packet_size {
            self.sender
                .write_packet(&[])
                .await
                .map_err(|_| CdcAcmError::NotConnected)?;
        }

        Ok(length)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
