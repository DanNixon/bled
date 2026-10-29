//! Emulated serial over BLE.
//!
//! Exposes a Nordic UART Service (NUS) style GATT service that carries a
//! bidirectional byte stream over BLE. The device API RPC server
//! ([`crate::api`]) runs over this stream.

use crate::{api, sdcard::SdCardStorage};
use bled_core::{
    api::RPC_FRAME_BUF_LEN,
    ble::{SERIAL_RX_UUID, SERIAL_SERVICE_UUID, SERIAL_TX_UUID},
};
use core::time::Duration;
use defmt::{info, warn};
use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe};
use embassy_time::{Instant, Timer};
use static_cell::StaticCell;
use teeny_rpc::{Error as RpcError, transport::Transport};
use trouble_host::prelude::*;

/// Size of the serial receive pipe in bytes.
const PIPE_SIZE: usize = 1024;

/// Maximum size of a single GATT write payload.
///
/// The default packet pool MTU limits ATT payloads to 247 bytes, hence a
/// maximum write payload of 244 bytes.
const MAX_WRITE_PAYLOAD: usize = 244;

/// Maximum size of a single notification payload.
const MAX_NOTIFY_PAYLOAD: usize = 244;

/// Maximum time to wait for the client to subscribe to notifications before
/// failing a transmission.
const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

#[gatt_server]
pub(super) struct Server {
    serial_service: SerialService,
}

#[gatt_service(uuid = SERIAL_SERVICE_UUID.as_u128())]
pub(super) struct SerialService {
    /// Serial data from device to host.
    #[characteristic(uuid = SERIAL_TX_UUID.as_u128(), notify)]
    tx: (),

    /// Serial data from host to device.
    #[characteristic(uuid = SERIAL_RX_UUID.as_u128(), write, write_without_response)]
    rx: (),
}

/// The static serial receive pipe, filled from GATT writes by the GATT event
/// task and drained by the RPC transport.
type SerialPipe = Pipe<CriticalSectionRawMutex, PIPE_SIZE>;

static RX_PIPE: StaticCell<SerialPipe> = StaticCell::new();

/// Creates the serial receive pipe. Must be called at most once.
pub(super) fn init_rx_pipe() -> &'static SerialPipe {
    RX_PIPE.init(Pipe::new())
}

/// Runs the RPC server over the emulated serial connection until the client
/// disconnects.
pub(super) async fn run_connection<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    rx: &'static SerialPipe,
    sd: SdCardStorage,
) {
    rx.clear();
    info!("Serial connection opened");

    let events = gatt_events_task(conn, server, rx);
    let rpc = api::serve(
        BleSerialTransport::new(rx, conn, server.serial_service.tx),
        sd,
    );

    match select(events, rpc).await {
        Either::First(_) => {}
        Either::Second(_) => conn.raw().disconnect(),
    }
}

/// Dispatches GATT write events into the serial receive pipe until the
/// connection is closed.
async fn gatt_events_task<P: PacketPool>(
    conn: &GattConnection<'_, '_, P>,
    server: &Server<'_>,
    rx: &SerialPipe,
) {
    let rx_handle = server.serial_service.rx.handle;

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let reply = match event {
                    GattEvent::Write(event) if event.handle() == rx_handle => {
                        process_rx_write(event, rx).await
                    }
                    _ => event.accept(),
                };

                match reply {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("Error sending response: {:?}", e),
                }
            }
            _ => {}
        }
    };

    info!("Serial connection closed: {:?}", reason);
}

async fn process_rx_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    rx: &SerialPipe,
) -> Result<Reply<'stack, P>, Error> {
    let mut scratch = [0u8; MAX_WRITE_PAYLOAD];

    let outcome = event.with_data(|offset, data| {
        if offset != 0 {
            return Err(AttErrorCode::INVALID_OFFSET);
        }
        if data.len() > scratch.len() {
            return Err(AttErrorCode::VALUE_NOT_ALLOWED);
        }
        scratch[..data.len()].copy_from_slice(data);
        Ok(data.len())
    });

    match outcome {
        Ok(length) => {
            let _ = rx.write_all(&scratch[..length]).await;
            event.accept_unprocessed()
        }
        Err(code) => event.reject(code),
    }
}

/// `teeny-rpc` transport carrying RPC frames over the emulated serial stream.
struct BleSerialTransport<'a, 'conn, P: PacketPool> {
    rx: &'a SerialPipe,
    conn: &'a GattConnection<'conn, 'conn, P>,
    tx_char: Characteristic<()>,
    frame: heapless::Vec<u8, RPC_FRAME_BUF_LEN>,
}

impl<'a, 'conn, P: PacketPool> BleSerialTransport<'a, 'conn, P> {
    fn new(
        rx: &'a SerialPipe,
        conn: &'a GattConnection<'conn, 'conn, P>,
        tx_char: Characteristic<()>,
    ) -> Self {
        Self {
            rx,
            conn,
            tx_char,
            frame: heapless::Vec::new(),
        }
    }
}

impl<'a, 'conn, P: PacketPool> Transport for BleSerialTransport<'a, 'conn, P> {
    async fn flush(&mut self, _timeout: Duration) -> Result<usize, RpcError> {
        let discarded = self.rx.len();
        self.rx.clear();
        Ok(discarded)
    }

    async fn receive_frame(&mut self, timeout: Duration) -> Result<&mut [u8], RpcError> {
        self.frame.clear();

        let timeout = embassy_time::Duration::from_micros(timeout.as_micros() as u64);
        let deadline = Instant::now() + timeout;
        let mut byte = [0u8; 1];

        loop {
            let received = match self.rx.try_read(&mut byte) {
                Ok(1) => true,
                Ok(_) => false,
                Err(_) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(RpcError::Timeout);
                    }

                    match embassy_time::with_timeout(deadline - now, self.rx.read(&mut byte)).await
                    {
                        Ok(received) => received == 1,
                        Err(_) => return Err(RpcError::Timeout),
                    }
                }
            };

            if !received {
                continue;
            }

            // The frame buffer keeps the trailing zero delimiter, as required
            // by the COBS decoder.
            if self.frame.push(byte[0]).is_err() {
                return Err(RpcError::FrameTooLong);
            }

            if byte[0] == 0 {
                return Ok(&mut self.frame[..]);
            }
        }
    }

    async fn transmit_frame(&mut self, frame: &[u8]) -> Result<(), RpcError> {
        // Notifications are silently dropped until the client subscribes to
        // the TX characteristic, so wait for the subscription first.
        let subscribe_timeout =
            embassy_time::Duration::from_micros(SUBSCRIBE_TIMEOUT.as_micros() as u64);
        let deadline = Instant::now() + subscribe_timeout;
        while !self.tx_char.should_notify(self.conn) {
            if Instant::now() >= deadline {
                warn!("Client failed to subscribe to serial notifications");
                return Err(RpcError::TransportError);
            }
            Timer::after_millis(10).await;
        }

        let mtu = usize::from(self.conn.raw().att_mtu());
        let payload_len = mtu.saturating_sub(3).clamp(20, MAX_NOTIFY_PAYLOAD);

        for chunk in frame.chunks(payload_len) {
            self.tx_char
                .notify_raw(self.conn, chunk, false)
                .await
                .map_err(|_| RpcError::TransportError)?;
        }

        Ok(())
    }
}
