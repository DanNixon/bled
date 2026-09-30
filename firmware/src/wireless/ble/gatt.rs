use crate::{
    api_transport::{
        file_io::{FileIoMode, FileIoSession},
        led_buffer::{LedBufferMode, LedBufferSession},
    },
    leds,
    sdcard::SdCardStorage,
};
use bled_api::{
    ChannelMask, ChannelRanges, FileIoControl, LedBufferControl, PixelRanges, RGB8, Rgb8Ranges,
    ble::{
        COMMIT_UUID, DEVICE_INFO_UUID, FILE_IO_CONTROL_UUID, FILE_IO_DATA_UUID,
        LED_BUFFER_CONTROL_UUID, LED_BUFFER_DATA_UUID, LED_RANGE_UUID, MAX_ATTRIBUTE_VALUE_LEN,
        RESET_UUID, SERVICE_UUID,
    },
};
use defmt::{debug, error, info, warn};
use trouble_host::{
    Error, PacketPool,
    att::AttErrorCode,
    gatt::{GattConnection, GattConnectionEvent, GattEvent, ReadEvent, Reply, WriteEvent},
    prelude::{FromGatt, gatt_server, gatt_service},
};

#[gatt_server]
pub(super) struct Server {
    led_service: LedService,
}

#[gatt_service(uuid = SERVICE_UUID.as_u128())]
pub(super) struct LedService {
    #[characteristic(uuid = DEVICE_INFO_UUID.as_u128(), read)]
    device_info: (),

    #[characteristic(uuid = RESET_UUID.as_u128(), write)]
    reset: (),

    #[characteristic(uuid = FILE_IO_CONTROL_UUID.as_u128(), write)]
    file_io_control: (),

    #[characteristic(uuid = FILE_IO_DATA_UUID.as_u128(), read, write)]
    file_io_data: (),

    #[characteristic(uuid = COMMIT_UUID.as_u128(), write)]
    commit: (),

    #[characteristic(uuid = LED_BUFFER_CONTROL_UUID.as_u128(), write)]
    led_buffer_control: (),

    #[characteristic(uuid = LED_BUFFER_DATA_UUID.as_u128(), read, write)]
    led_buffer_data: (),

    #[characteristic(
        uuid = LED_RANGE_UUID.as_u128(),
        write,
        write_without_response
    )]
    range_data: (),
}

pub(super) async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    sd: SdCardStorage,
) {
    let mut file_io = FileIoSession::new(sd);
    let mut led_buffer = LedBufferSession::new();

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let reply = match event {
                    GattEvent::Read(event) => {
                        if event.handle() == server.led_service.device_info.handle {
                            process_device_info(event).await
                        } else if event.handle() == server.led_service.file_io_data.handle {
                            process_file_io_data_read(event, &mut file_io).await
                        } else if event.handle() == server.led_service.led_buffer_data.handle {
                            process_led_buffer_data_read(event, &mut led_buffer).await
                        } else {
                            event.accept()
                        }
                    }
                    GattEvent::Write(event) => {
                        if event.handle() == server.led_service.reset.handle {
                            process_reset(event).await
                        } else if event.handle() == server.led_service.file_io_control.handle {
                            process_file_io_control_write(event, &mut file_io).await
                        } else if event.handle() == server.led_service.file_io_data.handle {
                            process_file_io_data_write(event, &mut file_io).await
                        } else if event.handle() == server.led_service.commit.handle {
                            process_commit(event).await
                        } else if event.handle() == server.led_service.led_buffer_control.handle {
                            process_led_buffer_control_write(event, &mut led_buffer).await
                        } else if event.handle() == server.led_service.led_buffer_data.handle {
                            process_led_buffer_data_write(event, &mut led_buffer).await
                        } else if event.handle() == server.led_service.range_data.handle {
                            process_range_data(event).await
                        } else {
                            event.accept()
                        }
                    }
                    _ => event.accept(),
                };

                match reply {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("Error sending response: {:?}", e),
                };
            }
            _ => {}
        }
    };

    file_io.reset().await;
    info!("Disconnected: {:?}", reason);
}

async fn process_device_info<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    let device_info = crate::device_info();

    let mut b = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
    let n = bled_api::io::encode_cbor(&device_info, &mut b)
        .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;

    event.accept_unprocessed(&b[..n])
}

async fn process_file_io_data_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    file_io: &mut FileIoSession,
) -> Result<Reply<'stack, P>, Error> {
    match file_io.mode() {
        FileIoMode::Stat { response } => {
            let mut buf = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            let n = bled_api::io::encode_cbor(response, &mut buf)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;
            event.accept_unprocessed(&buf[..n])
        }
        FileIoMode::Reading { .. } => {
            let mut chunk = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            match file_io.read_chunk(&mut chunk).await {
                Ok(n) => {
                    debug!("Read chunk of size {}", n);
                    event.accept_unprocessed(&chunk[..n])
                }
                Err(e) => {
                    warn!("Failed to read file chunk: {}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
        _ => {
            error!("not in correct state");
            event.reject(AttErrorCode::READ_NOT_PERMITTED)
        }
    }
}

async fn process_file_io_control_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    file_io: &mut FileIoSession,
) -> Result<Reply<'stack, P>, Error> {
    let control: FileIoControl = match event.with_data(|offset, data| {
        if offset != 0 {
            return Err(AttErrorCode::INVALID_OFFSET);
        }
        bled_api::io::decode_cbor::<FileIoControl>(data)
            .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
    }) {
        Ok(cmd) => cmd,
        Err(err) => return event.reject(err),
    };
    info!("File IO control message: {}", control);

    match control {
        FileIoControl::Read { path, offset } => {
            match file_io.prepare_read(path, offset as usize).await {
                Ok(()) => {
                    debug!("Prepared to read file");
                    event.accept_unprocessed()
                }
                Err(e) => {
                    warn!("Failed to prepare reading file: {}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
        FileIoControl::Write { path, offset } => {
            match file_io.prepare_write(path, offset as usize).await {
                Ok(()) => {
                    debug!("Prepared to write file");
                    event.accept_unprocessed()
                }
                Err(e) => {
                    warn!("Failed to prepare writing file: {}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
        FileIoControl::Reset => {
            file_io.reset().await;
            debug!("Reset file IO state machine");
            event.accept_unprocessed()
        }
        FileIoControl::Stat { path } => match file_io.prepare_stat(path).await {
            Ok(()) => {
                debug!("Prepared to stat file");
                event.accept_unprocessed()
            }
            Err(e) => {
                warn!("Failed to prepare stat for file: {}", e);
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
        FileIoControl::Create { path, size } => {
            match file_io.create_file(path, size as usize).await {
                Ok(()) => {
                    debug!("Created file");
                    event.accept_unprocessed()
                }
                Err(e) => {
                    warn!("Failed to create file: {}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
        FileIoControl::Delete { path } => match file_io.delete_file(path.as_str()).await {
            Ok(()) => {
                debug!("Deleted file {}", path.as_str());
                event.accept_unprocessed()
            }
            Err(e) => {
                warn!("Failed to delete file {}: {}", path.as_str(), e);
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
    }
}

async fn process_file_io_data_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    file_io: &mut FileIoSession,
) -> Result<Reply<'stack, P>, Error> {
    if !matches!(file_io.mode(), FileIoMode::Writing { .. }) {
        error!("not in correct state");
        return event.reject(AttErrorCode::WRITE_NOT_PERMITTED);
    }

    let mut chunk = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
    let (offset, n) = event.with_data(|offset, data| {
        chunk[..data.len()].copy_from_slice(data);
        (offset, data.len())
    });

    match file_io.write_chunk(offset as u64, &chunk[..n]).await {
        Ok(()) => {
            debug!("Wrote {} bytes to file", n);
            event.accept_unprocessed()
        }
        Err(e) => {
            warn!("Failed to write chunk: {}", e);
            event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
        }
    }
}

async fn process_commit<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    match event.with_data(|offset, data| {
        if offset != 0 {
            Err(AttErrorCode::INVALID_OFFSET)
        } else {
            bled_api::io::decode_cbor::<ChannelMask>(data)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
        }
    }) {
        Ok(mask) => {
            leds::draw(mask).await;
            debug!("led draw 0x{:x}", mask.bits());
            event.accept_unprocessed()
        }
        Err(err) => event.reject(err),
    }
}

async fn process_led_buffer_data_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    led_buffer: &mut LedBufferSession,
) -> Result<Reply<'stack, P>, Error> {
    match led_buffer.mode() {
        LedBufferMode::Info { response } => {
            let mut buf = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            let n = bled_api::io::encode_cbor(response, &mut buf)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;
            event.accept_unprocessed(&buf[..n])
        }
        LedBufferMode::Reading { .. } => {
            let mut chunk = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            match led_buffer.read_chunk(&mut chunk).await {
                Ok(n) => event.accept_unprocessed(&chunk[..n]),
                Err(e) => {
                    warn!("Failed to read LED buffer chunk: {:?}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
        _ => {
            error!("not in correct state");
            event.reject(AttErrorCode::READ_NOT_PERMITTED)
        }
    }
}

async fn process_led_buffer_control_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    led_buffer: &mut LedBufferSession,
) -> Result<Reply<'stack, P>, Error> {
    let control: LedBufferControl = match event.with_data(|offset, data| {
        if offset != 0 {
            return Err(AttErrorCode::INVALID_OFFSET);
        }
        bled_api::io::decode_cbor::<LedBufferControl>(data)
            .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
    }) {
        Ok(cmd) => cmd,
        Err(err) => return event.reject(err),
    };
    info!("LED buffer control message: {:?}", control);

    match control {
        LedBufferControl::Info => {
            led_buffer.prepare_info().await;
            debug!("Prepared LED buffer info");
            event.accept_unprocessed()
        }
        LedBufferControl::Read { offset } => match led_buffer.prepare_read(offset as usize).await {
            Ok(()) => {
                debug!("Prepared to read LED buffer");
                event.accept_unprocessed()
            }
            Err(()) => {
                warn!("Failed to prepare reading LED buffer");
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
        LedBufferControl::Write { offset } => match led_buffer.prepare_write(offset as usize).await
        {
            Ok(()) => {
                debug!("Prepared to write LED buffer");
                event.accept_unprocessed()
            }
            Err(()) => {
                warn!("Failed to prepare writing LED buffer");
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
        LedBufferControl::Reset => {
            led_buffer.reset();
            debug!("Reset LED buffer state machine");
            event.accept_unprocessed()
        }
    }
}

async fn process_led_buffer_data_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    led_buffer: &mut LedBufferSession,
) -> Result<Reply<'stack, P>, Error> {
    if !matches!(led_buffer.mode(), LedBufferMode::Writing { .. }) {
        error!("not in correct state");
        return event.reject(AttErrorCode::WRITE_NOT_PERMITTED);
    }

    let mut chunk = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
    let (offset, n) = event.with_data(|offset, data| {
        chunk[..data.len()].copy_from_slice(data);
        (offset, data.len())
    });

    match led_buffer.write_chunk(offset, &chunk[..n]).await {
        Ok(()) => {
            debug!("Wrote {} bytes to LED buffer", n);
            event.accept_unprocessed()
        }
        Err(e) => {
            warn!("Failed to write LED buffer chunk: {:?}", e);
            event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
        }
    }
}

async fn process_reset<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    crate::status::reset();
    event.accept()
}

async fn process_range_data<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    let result: Result<(), AttErrorCode> = async {
        let ranges = event.with_data(|offset, data| {
            if offset != 0 {
                return Err(AttErrorCode::INVALID_OFFSET);
            }
            bled_api::io::decode_cbor::<ChannelRanges<RGB8, Rgb8Ranges<24>>>(data)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
        })?;

        let mut channel_data = leds::channel_data().await;

        let channel = usize::from(*ranges.channel());
        let data = channel_data
            .try_channel_mut::<RGB8>(channel)
            .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;

        // Validate the complete message before staging any changes.
        for (range, _) in ranges.ranges().iter() {
            if range.end > data.len() {
                return Err(AttErrorCode::VALUE_NOT_ALLOWED);
            }
        }

        for (range, colour) in ranges.ranges().iter() {
            data[range].fill(colour);
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => {
            debug!("led range data");
            event.accept_unprocessed()
        }
        Err(err) => event.reject(err),
    }
}
