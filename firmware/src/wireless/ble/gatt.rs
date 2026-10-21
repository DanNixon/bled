use crate::{
    api_transport::{
        config::{ConfigMode, ConfigSession},
        effect_manifest::{EffectManifestSession, Error as EffectManifestError},
        file_io::{FileIoMode, FileIoSession},
        led_buffer::{LedBufferMode, LedBufferSession},
    },
    effect, leds,
    sdcard::SdCardStorage,
};
use bled_core::{
    PathString,
    api::{ConfigControl, EffectManifestControl, FileIoControl, LedBufferControl},
    ble::{
        ACTIVE_EFFECT_COMMAND_DATA_UUID, ACTIVE_EFFECT_COMMAND_LENGTH_UUID,
        ACTIVE_EFFECT_MANIFEST_CONTROL_UUID, ACTIVE_EFFECT_MANIFEST_DATA_UUID,
        ACTIVE_EFFECT_PARAMETER_DATA_UUID, ACTIVE_EFFECT_PARAMETER_LENGTH_UUID, ACTIVE_EFFECT_UUID,
        COMMIT_UUID, CONFIG_CONTROL_UUID, CONFIG_DATA_UUID, DEVICE_INFO_UUID, FILE_IO_CONTROL_UUID,
        FILE_IO_DATA_UUID, LED_BUFFER_CONTROL_UUID, LED_BUFFER_DATA_UUID, MAX_ATTRIBUTE_VALUE_LEN,
        RESET_UUID, SERVICE_UUID,
    },
};
use defmt::{debug, error, info, warn};
use trouble_host::{
    Error, PacketPool,
    att::{AttClient, AttErrorCode, AttReq},
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

    #[characteristic(uuid = CONFIG_CONTROL_UUID.as_u128(), write)]
    config_control: (),

    #[characteristic(uuid = CONFIG_DATA_UUID.as_u128(), read)]
    config_data: (),

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

    #[characteristic(uuid = ACTIVE_EFFECT_UUID.as_u128(), read, write)]
    active_effect: (),

    #[characteristic(uuid = ACTIVE_EFFECT_MANIFEST_CONTROL_UUID.as_u128(), write)]
    effect_manifest_control: (),

    #[characteristic(uuid = ACTIVE_EFFECT_MANIFEST_DATA_UUID.as_u128(), read)]
    effect_manifest_data: (),

    #[characteristic(uuid = ACTIVE_EFFECT_COMMAND_LENGTH_UUID.as_u128(), read)]
    effect_command_length: (),

    #[characteristic(uuid = ACTIVE_EFFECT_COMMAND_DATA_UUID.as_u128(), read, write)]
    effect_command_data: (),

    #[characteristic(uuid = ACTIVE_EFFECT_PARAMETER_LENGTH_UUID.as_u128(), read)]
    effect_parameter_length: (),

    #[characteristic(uuid = ACTIVE_EFFECT_PARAMETER_DATA_UUID.as_u128(), read, write)]
    effect_parameter_data: (),
}

pub(super) async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    sd: SdCardStorage,
) {
    let mut file_io = FileIoSession::new(sd.clone());
    let mut led_buffer = LedBufferSession::new();
    let mut config = ConfigSession::new(crate::config::get());
    let mut effect_manifest = EffectManifestSession::new();

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let reply = match event {
                    GattEvent::Read(event) => {
                        if event.handle() == server.led_service.device_info.handle {
                            process_device_info(event).await
                        } else if event.handle() == server.led_service.active_effect.handle {
                            process_active_effect_read(event).await
                        } else if event.handle() == server.led_service.effect_manifest_data.handle {
                            process_effect_manifest_data_read(event, &mut effect_manifest).await
                        } else if event.handle() == server.led_service.effect_command_length.handle
                        {
                            process_effect_length_read(event, effect::effect_command_size().await)
                                .await
                        } else if event.handle() == server.led_service.effect_command_data.handle {
                            process_effect_data_read(event, true).await
                        } else if event.handle()
                            == server.led_service.effect_parameter_length.handle
                        {
                            process_effect_length_read(event, effect::effect_parameter_size().await)
                                .await
                        } else if event.handle() == server.led_service.effect_parameter_data.handle
                        {
                            process_effect_data_read(event, false).await
                        } else if event.handle() == server.led_service.config_data.handle {
                            process_config_data_read(event, &mut config).await
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
                        } else if event.handle() == server.led_service.active_effect.handle {
                            process_active_effect_write(event, &sd, &mut effect_manifest).await
                        } else if event.handle()
                            == server.led_service.effect_manifest_control.handle
                        {
                            process_effect_manifest_control_write(event, &mut effect_manifest).await
                        } else if event.handle() == server.led_service.effect_command_data.handle {
                            process_effect_data_write(event, true).await
                        } else if event.handle() == server.led_service.effect_parameter_data.handle
                        {
                            process_effect_data_write(event, false).await
                        } else if event.handle() == server.led_service.config_control.handle {
                            process_config_control_write(event, &mut config).await
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
    config.reset();
    effect_manifest.reset();
    info!("Disconnected: {:?}", reason);
}

async fn process_active_effect_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    let path = effect::active_effect_path().await;
    let bytes = path
        .as_ref()
        .map_or(&[][..], |path| path.as_str().as_bytes());
    event.accept_unprocessed(bytes)
}

async fn process_active_effect_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    sd: &SdCardStorage,
    effect_manifest: &mut EffectManifestSession,
) -> Result<Reply<'stack, P>, Error> {
    let mut bytes = [0; MAX_ATTRIBUTE_VALUE_LEN];
    let (offset, len) = event.with_data(|offset, data| {
        if data.len() <= bytes.len() {
            bytes[..data.len()].copy_from_slice(data);
        }
        (offset, data.len())
    });
    if offset != 0 {
        return event.reject(AttErrorCode::INVALID_OFFSET);
    }
    if len > bytes.len() {
        return event.reject(AttErrorCode::VALUE_NOT_ALLOWED);
    }

    let path = if len == 0 {
        None
    } else {
        let path = match core::str::from_utf8(&bytes[..len]) {
            Ok(path) => path,
            Err(_) => return event.reject(AttErrorCode::VALUE_NOT_ALLOWED),
        };
        match PathString::try_from(path) {
            Ok(path) => Some(path),
            Err(_) => return event.reject(AttErrorCode::VALUE_NOT_ALLOWED),
        }
    };

    effect_manifest.reset();
    match effect::activate_effect(sd, path).await {
        Ok(()) => event.accept_unprocessed(),
        Err(()) => event.reject(AttErrorCode::VALUE_NOT_ALLOWED),
    }
}

async fn process_effect_manifest_data_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    session: &mut EffectManifestSession,
) -> Result<Reply<'stack, P>, Error> {
    let mut bytes = [0; MAX_ATTRIBUTE_VALUE_LEN];
    let result =
        effect::with_active_effect_manifest(|manifest| session.read_data(manifest, &mut bytes))
            .await;
    match result {
        Ok(len) => {
            if let AttClient::Request(AttReq::ReadBlob { offset, .. }) = event.payload().incoming()
                && usize::from(offset) > len
            {
                return event.reject(AttErrorCode::INVALID_OFFSET);
            }
            event.accept_unprocessed(&bytes[..len])
        }
        Err(error) => {
            warn!("Failed to read effect manifest: {}", error);
            event.reject(match error {
                EffectManifestError::NoEffect
                | EffectManifestError::EffectChanged
                | EffectManifestError::NotReading => AttErrorCode::READ_NOT_PERMITTED,
                _ => AttErrorCode::VALUE_NOT_ALLOWED,
            })
        }
    }
}

async fn process_effect_manifest_control_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    session: &mut EffectManifestSession,
) -> Result<Reply<'stack, P>, Error> {
    let control = match event.with_data(|offset, data| {
        if offset != 0 {
            return Err(AttErrorCode::INVALID_OFFSET);
        }
        bled_core::io::decode_cbor::<EffectManifestControl>(data)
            .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
    }) {
        Ok(control) => control,
        Err(error) => return event.reject(error),
    };
    info!("Effect manifest control message: {:?}", control);

    let result = effect::with_active_effect_manifest(|manifest| match control {
        EffectManifestControl::Stat => session.prepare_stat(manifest),
        EffectManifestControl::Read { offset } => session.prepare_read(manifest, offset as usize),
        EffectManifestControl::Reset => {
            session.reset();
            Ok(())
        }
    })
    .await;
    match result {
        Ok(()) => event.accept_unprocessed(),
        Err(error) => {
            warn!("Failed to prepare effect manifest response: {}", error);
            event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
        }
    }
}

async fn process_effect_length_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    length: Result<usize, ()>,
) -> Result<Reply<'stack, P>, Error> {
    let length = match length {
        Ok(length) => match u32::try_from(length) {
            Ok(length) => length,
            Err(_) => return event.reject(AttErrorCode::VALUE_NOT_ALLOWED),
        },
        Err(()) => return event.reject(AttErrorCode::READ_NOT_PERMITTED),
    };
    event.accept_unprocessed(&length.to_le_bytes())
}

async fn process_effect_data_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    command: bool,
) -> Result<Reply<'stack, P>, Error> {
    let mut bytes = [0; MAX_ATTRIBUTE_VALUE_LEN];
    let result = if command {
        effect::read_effect_command(&mut bytes).await
    } else {
        effect::read_effect_parameters(&mut bytes).await
    };
    match result {
        Ok(len) => event.accept_unprocessed(&bytes[..len]),
        Err(()) => event.reject(AttErrorCode::READ_NOT_PERMITTED),
    }
}

async fn process_effect_data_write<'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    command: bool,
) -> Result<Reply<'stack, P>, Error> {
    let mut bytes = [0; MAX_ATTRIBUTE_VALUE_LEN];
    let (offset, len) = event.with_data(|offset, data| {
        if data.len() <= bytes.len() {
            bytes[..data.len()].copy_from_slice(data);
        }
        (offset, data.len())
    });
    if offset != 0 {
        return event.reject(AttErrorCode::INVALID_OFFSET);
    }
    if len > bytes.len() {
        return event.reject(AttErrorCode::VALUE_NOT_ALLOWED);
    }

    let result = if command {
        effect::write_effect_command(&bytes[..len]).await
    } else {
        effect::write_effect_parameters(&bytes[..len]).await
    };
    match result {
        Ok(()) => event.accept_unprocessed(),
        Err(()) => event.reject(AttErrorCode::VALUE_NOT_ALLOWED),
    }
}

async fn process_config_data_read<'config, 'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    config: &mut ConfigSession<'config>,
) -> Result<Reply<'stack, P>, Error> {
    match config.mode() {
        ConfigMode::Stat { response } => {
            let mut buf = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            let n = bled_core::io::encode_cbor(response, &mut buf)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;
            event.accept_unprocessed(&buf[..n])
        }
        ConfigMode::Reading { .. } => {
            let mut chunk = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            match config.read_chunk(&mut chunk) {
                Ok(n) => event.accept_unprocessed(&chunk[..n]),
                Err(e) => {
                    warn!("Failed to read config chunk: {:?}", e);
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

async fn process_config_control_write<'config, 'stack, P: PacketPool>(
    event: WriteEvent<'stack, '_, P>,
    config: &mut ConfigSession<'config>,
) -> Result<Reply<'stack, P>, Error> {
    let control: ConfigControl = match event.with_data(|offset, data| {
        if offset != 0 {
            return Err(AttErrorCode::INVALID_OFFSET);
        }
        bled_core::io::decode_cbor::<ConfigControl>(data)
            .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)
    }) {
        Ok(cmd) => cmd,
        Err(err) => return event.reject(err),
    };
    info!("Config control message: {:?}", control);

    match control {
        ConfigControl::Stat => match config.prepare_stat() {
            Ok(()) => {
                debug!("Prepared to stat config");
                event.accept_unprocessed()
            }
            Err(e) => {
                warn!("Failed to prepare stat config: {}", e);
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
        ConfigControl::Read { offset } => match config.prepare_read(offset as usize) {
            Ok(()) => {
                debug!("Prepared to read config");
                event.accept_unprocessed()
            }
            Err(e) => {
                warn!("Failed to prepare reading config: {}", e);
                event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
            }
        },
        ConfigControl::Reset => {
            config.reset();
            debug!("Reset config state machine");
            event.accept_unprocessed()
        }
    }
}

async fn process_device_info<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
) -> Result<Reply<'stack, P>, Error> {
    let device_info = crate::device_info();

    let mut b = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
    let n = bled_core::io::encode_cbor(&device_info, &mut b)
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
            let n = bled_core::io::encode_cbor(response, &mut buf)
                .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;
            event.accept_unprocessed(&buf[..n])
        }
        FileIoMode::ReadingDir { current_entry, .. } => {
            if let Some(entry) = current_entry {
                let mut buf = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
                let n = bled_core::io::encode_cbor(entry, &mut buf)
                    .map_err(|_| AttErrorCode::VALUE_NOT_ALLOWED)?;
                event.accept_unprocessed(&buf[..n])
            } else {
                event.accept_unprocessed(&[])
            }
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
        bled_core::io::decode_cbor::<FileIoControl>(data)
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
        FileIoControl::ReadDir { path, index } => {
            match file_io.prepare_read_dir(path, index as usize).await {
                Ok(()) => {
                    debug!("Prepared to read dir");
                    event.accept_unprocessed()
                }
                Err(e) => {
                    warn!("Failed to prepare reading dir: {}", e);
                    event.reject(AttErrorCode::VALUE_NOT_ALLOWED)
                }
            }
        }
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
    leds::draw().await;
    event.accept_unprocessed()
}

async fn process_led_buffer_data_read<'stack, P: PacketPool>(
    event: ReadEvent<'stack, '_, P>,
    led_buffer: &mut LedBufferSession,
) -> Result<Reply<'stack, P>, Error> {
    match led_buffer.mode() {
        LedBufferMode::Info { response } => {
            let mut buf = [0u8; MAX_ATTRIBUTE_VALUE_LEN];
            let n = bled_core::io::encode_cbor(response, &mut buf)
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
        bled_core::io::decode_cbor::<LedBufferControl>(data)
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
