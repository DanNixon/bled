use bled_core::{
    PathString,
    api::{
        Chunk, DeviceInfo, DirEntry, LedBufferInformation, MAX_CHUNK_SIZE, RPC_ACK_TIMEOUT,
        RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN, RPC_LOAD_TIMEOUT, RPC_RESPONSE_TIMEOUT, Request,
        Response,
    },
    config::DeviceConfig,
    effect::EffectManifest,
};
use core::time::Duration;
use heapless::Vec as HeaplessVec;
use js_sys::{Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

type Rpc =
    teeny_rpc::Client<JsSerialTransport, Request, Response, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN>;

pub struct JsSerialTransport {
    handle: JsValue,
    read_frame_fn: Function,
    write_frame_fn: Function,
    flush_rx_fn: Function,
    frame: HeaplessVec<u8, RPC_FRAME_BUF_LEN>,
}

impl JsSerialTransport {
    pub fn new(handle: JsValue) -> Result<Self, String> {
        let read_frame_fn = Reflect::get(&handle, &JsValue::from_str("readFrame"))
            .map_err(|e| format!("{e:?}"))?
            .dyn_into::<Function>()
            .map_err(|_| "readFrame is not a function")?;
        let write_frame_fn = Reflect::get(&handle, &JsValue::from_str("writeFrame"))
            .map_err(|e| format!("{e:?}"))?
            .dyn_into::<Function>()
            .map_err(|_| "writeFrame is not a function")?;
        let flush_rx_fn = Reflect::get(&handle, &JsValue::from_str("flushRx"))
            .map_err(|e| format!("{e:?}"))?
            .dyn_into::<Function>()
            .map_err(|_| "flushRx is not a function")?;

        Ok(Self {
            handle,
            read_frame_fn,
            write_frame_fn,
            flush_rx_fn,
            frame: HeaplessVec::new(),
        })
    }
}

impl teeny_rpc::transport::Transport for JsSerialTransport {
    async fn flush(&mut self, _timeout: Duration) -> Result<usize, teeny_rpc::Error> {
        self.flush_rx_fn
            .call0(&self.handle)
            .map_err(|_| teeny_rpc::Error::TransportError)?
            .as_f64()
            .map(|count| count as usize)
            .ok_or(teeny_rpc::Error::TransportError)
    }

    async fn receive_frame(&mut self, timeout: Duration) -> Result<&mut [u8], teeny_rpc::Error> {
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        let result = self
            .read_frame_fn
            .call1(&self.handle, &JsValue::from_f64(f64::from(timeout_ms)))
            .map_err(|_| teeny_rpc::Error::TransportError)?;
        let promise = result
            .dyn_into::<Promise>()
            .map_err(|_| teeny_rpc::Error::TransportError)?;
        let value = JsFuture::from(promise)
            .await
            .map_err(|_| teeny_rpc::Error::Timeout)?;
        let bytes = value
            .dyn_into::<Uint8Array>()
            .map_err(|_| teeny_rpc::Error::TransportError)?;

        self.frame.clear();
        self.frame
            .extend_from_slice(&bytes.to_vec())
            .map_err(|_| teeny_rpc::Error::FrameTooLong)?;
        Ok(&mut self.frame)
    }

    async fn transmit_frame(&mut self, frame: &[u8]) -> Result<(), teeny_rpc::Error> {
        let bytes = Uint8Array::from(frame);
        let result = self
            .write_frame_fn
            .call1(&self.handle, &bytes.into())
            .map_err(|_| teeny_rpc::Error::TransportError)?;
        let promise = result
            .dyn_into::<Promise>()
            .map_err(|_| teeny_rpc::Error::TransportError)?;
        JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|_| teeny_rpc::Error::TransportError)
    }
}

pub struct RpcClient {
    handle: JsValue,
    client: Rpc,
}

impl RpcClient {
    pub fn new(handle: JsValue) -> Result<Self, String> {
        let transport = JsSerialTransport::new(handle.clone())?;
        Ok(Self {
            handle,
            client: Rpc::new(transport, RPC_ACK_TIMEOUT),
        })
    }

    pub fn id(&self) -> String {
        let f = Reflect::get(&self.handle, &JsValue::from_str("id"))
            .ok()
            .and_then(|v| v.dyn_into::<Function>().ok());
        f.and_then(|func| func.call0(&self.handle).ok())
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "unknown-id".into())
    }

    pub fn name(&self) -> String {
        let f = Reflect::get(&self.handle, &JsValue::from_str("name"))
            .ok()
            .and_then(|v| v.dyn_into::<Function>().ok());
        f.and_then(|func| func.call0(&self.handle).ok())
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "unknown-device".into())
    }

    pub fn is_ble(&self) -> bool {
        let f = Reflect::get(&self.handle, &JsValue::from_str("isBle"))
            .ok()
            .and_then(|v| v.dyn_into::<Function>().ok());
        f.and_then(|func| func.call0(&self.handle).ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub fn disconnect(&self) {
        if let Ok(func) = Reflect::get(&self.handle, &JsValue::from_str("disconnect"))
            .and_then(|v| v.dyn_into::<Function>())
        {
            let _ = func.call0(&self.handle);
        }
    }

    async fn call(&mut self, request: Request, timeout: Duration) -> Result<Response, String> {
        let response = self
            .client
            .call(request, timeout)
            .await
            .map_err(|error| format!("RPC failed: {error}"))?;
        match response {
            Response::Error(error) => Err(format!("Device API error: {error:?}")),
            response => Ok(response),
        }
    }

    pub async fn read_device_info(&mut self) -> Result<DeviceInfo, String> {
        match self
            .call(Request::GetDeviceInfo, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::DeviceInfo(info) => Ok(info),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn reboot(&mut self) -> Result<(), String> {
        match self.call(Request::Reboot, RPC_RESPONSE_TIMEOUT).await? {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn commit(&mut self) -> Result<(), String> {
        match self.call(Request::Commit, RPC_RESPONSE_TIMEOUT).await? {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn config_stat(&mut self) -> Result<u64, String> {
        match self.call(Request::ConfigStat, RPC_RESPONSE_TIMEOUT).await? {
            Response::Size(size) => Ok(size),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn config_read(&mut self, offset: u32, length: u32) -> Result<Chunk, String> {
        match self
            .call(Request::ConfigRead { offset, length }, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Data(chunk) => Ok(chunk),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn read_config<F>(
        &mut self,
        mut on_progress: Option<F>,
    ) -> Result<DeviceConfig, String>
    where
        F: FnMut(u8),
    {
        let size = self.config_stat().await?;
        let mut bytes = Vec::new();
        let mut offset = 0u32;
        let total = size as u32;

        while offset < total {
            let wanted = (MAX_CHUNK_SIZE as u32).min(total - offset);
            let chunk = self.config_read(offset, wanted).await?;
            if chunk.is_empty() {
                break;
            }
            let len = chunk.len() as u32;
            bytes.extend_from_slice(chunk.as_slice());
            offset += len;
            if let Some(ref mut cb) = on_progress
                && let Some(pct) = (offset * 100).checked_div(total)
            {
                cb(pct.min(100) as u8);
            }
        }

        // Try postcard decode first, then JSON
        match bled_core::io::decode_postcard::<DeviceConfig>(&bytes) {
            Ok(c) => Ok(c),
            Err(_) => bled_core::io::decode_json::<DeviceConfig>(&bytes)
                .map_err(|e| format!("Failed to decode DeviceConfig: {e}")),
        }
    }

    pub async fn file_stat(&mut self, path: &str) -> Result<u64, String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        match self
            .call(Request::FileStat { path: p }, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Size(size) => Ok(size),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn file_read(
        &mut self,
        path: &str,
        offset: u32,
        length: u32,
    ) -> Result<Chunk, String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        match self
            .call(
                Request::FileRead {
                    path: p,
                    offset,
                    length,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Data(chunk) => Ok(chunk),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn file_write(&mut self, path: &str, offset: u32, data: &[u8]) -> Result<(), String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        let chunk: Chunk = data.try_into().map_err(|_| "Data exceeds chunk size")?;
        match self
            .call(
                Request::FileWrite {
                    path: p,
                    offset,
                    data: chunk,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn file_create(&mut self, path: &str, size: u32) -> Result<(), String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        match self
            .call(Request::FileCreate { path: p, size }, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn file_delete(&mut self, path: &str) -> Result<(), String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        match self
            .call(Request::FileDelete { path: p }, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn read_dir_entry(
        &mut self,
        path: &str,
        index: u32,
    ) -> Result<Option<DirEntry>, String> {
        let p: PathString = path
            .try_into()
            .map_err(|_| "Remote path too long or invalid")?;
        match self
            .call(
                Request::ReadDirEntry { path: p, index },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::DirEntry(entry) => Ok(entry),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn list_directory(&mut self, path: &str) -> Result<Vec<DirEntry>, String> {
        let mut entries = Vec::new();
        let mut index = 0u32;
        while let Some(entry) = self.read_dir_entry(path, index).await? {
            entries.push(entry);
            index += 1;
        }
        Ok(entries)
    }

    pub async fn read_file_chunked<F>(
        &mut self,
        path: &str,
        mut on_progress: Option<F>,
    ) -> Result<Vec<u8>, String>
    where
        F: FnMut(u8),
    {
        let mut bytes = Vec::new();
        let mut offset = 0u32;
        loop {
            let chunk = self.file_read(path, offset, MAX_CHUNK_SIZE as u32).await?;
            if chunk.is_empty() {
                break;
            }
            let len = chunk.len();
            bytes.extend_from_slice(chunk.as_slice());
            offset += len as u32;
            if let Some(ref mut cb) = on_progress {
                cb(((bytes.len() * 10) % 95).min(95) as u8);
            }
            if len < MAX_CHUNK_SIZE {
                break;
            }
        }
        if let Some(ref mut cb) = on_progress {
            cb(100);
        }
        Ok(bytes)
    }

    pub async fn write_file_chunked<F>(
        &mut self,
        path: &str,
        data: &[u8],
        mut on_progress: Option<F>,
    ) -> Result<(), String>
    where
        F: FnMut(u8),
    {
        let total = data.len();
        self.file_create(path, total as u32).await?;
        let chunks = data.chunks(MAX_CHUNK_SIZE);
        let count = chunks.len();

        for (idx, slice) in chunks.enumerate() {
            let offset = (idx * MAX_CHUNK_SIZE) as u32;
            self.file_write(path, offset, slice).await?;
            if let Some(ref mut cb) = on_progress {
                cb((((idx + 1) * 100) / count) as u8);
            }
        }
        Ok(())
    }

    pub async fn led_buffer_info(&mut self) -> Result<LedBufferInformation, String> {
        match self
            .call(Request::LedBufferInfo, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::LedBufferInfo(info) => Ok(info),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn led_buffer_read(&mut self, offset: u32, length: u32) -> Result<Chunk, String> {
        match self
            .call(
                Request::LedBufferRead { offset, length },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Data(chunk) => Ok(chunk),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn led_buffer_write(&mut self, offset: u32, data: &[u8]) -> Result<(), String> {
        let chunk: Chunk = data.try_into().map_err(|_| "Data exceeds chunk size")?;
        match self
            .call(
                Request::LedBufferWrite {
                    offset,
                    data: chunk,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn read_led_buffer_chunked<F>(
        &mut self,
        offset: u32,
        size: u32,
        mut on_progress: Option<F>,
    ) -> Result<Vec<u8>, String>
    where
        F: FnMut(u8),
    {
        let mut bytes = Vec::new();
        let mut curr = offset;
        let end = offset + size;

        while curr < end {
            let wanted = (MAX_CHUNK_SIZE as u32).min(end - curr);
            let chunk = self.led_buffer_read(curr, wanted).await?;
            if chunk.is_empty() {
                break;
            }
            let len = chunk.len() as u32;
            bytes.extend_from_slice(chunk.as_slice());
            curr += len;
            if let Some(ref mut cb) = on_progress
                && let Some(pct) = ((curr - offset) * 100).checked_div(size)
            {
                cb(pct.min(100) as u8);
            }
        }
        if let Some(ref mut cb) = on_progress {
            cb(100);
        }
        Ok(bytes)
    }

    pub async fn write_led_buffer_chunked<F>(
        &mut self,
        offset: u32,
        data: &[u8],
        mut on_progress: Option<F>,
    ) -> Result<(), String>
    where
        F: FnMut(u8),
    {
        let chunks = data.chunks(MAX_CHUNK_SIZE);
        let count = chunks.len();

        for (idx, slice) in chunks.enumerate() {
            let chunk_offset = offset + (idx * MAX_CHUNK_SIZE) as u32;
            self.led_buffer_write(chunk_offset, slice).await?;
            if let Some(ref mut cb) = on_progress {
                cb((((idx + 1) * 100) / count) as u8);
            }
        }
        Ok(())
    }

    pub async fn get_active_effect(&mut self) -> Result<Option<String>, String> {
        match self
            .call(Request::GetActiveEffect, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::ActiveEffect(p) => Ok(p.map(|s| s.to_string())),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn set_active_effect(&mut self, path: Option<&str>) -> Result<(), String> {
        let p = match path {
            Some(s) if !s.is_empty() => Some(s.try_into().map_err(|_| "Path too long")?),
            _ => None,
        };
        match self
            .call(Request::SetActiveEffect { path: p }, RPC_LOAD_TIMEOUT)
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_manifest_stat(&mut self) -> Result<u64, String> {
        match self
            .call(Request::EffectManifestStat, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Size(size) => Ok(size),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_manifest_read(
        &mut self,
        offset: u32,
        length: u32,
    ) -> Result<Chunk, String> {
        match self
            .call(
                Request::EffectManifestRead { offset, length },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Data(chunk) => Ok(chunk),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn read_effect_manifest(&mut self) -> Result<EffectManifest, String> {
        let size = self.effect_manifest_stat().await?;
        let mut bytes = Vec::new();
        let mut offset = 0u32;
        let total = size as u32;

        while offset < total {
            let wanted = (MAX_CHUNK_SIZE as u32).min(total - offset);
            let chunk = self.effect_manifest_read(offset, wanted).await?;
            if chunk.is_empty() {
                break;
            }
            let len = chunk.len() as u32;
            bytes.extend_from_slice(chunk.as_slice());
            offset += len;
        }

        bled_core::io::decode_json::<EffectManifest>(&bytes)
            .map_err(|e| format!("Failed to decode EffectManifest: {e}"))
    }

    pub async fn effect_command_stat(&mut self) -> Result<u64, String> {
        match self
            .call(Request::EffectCommandStat, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Size(size) => Ok(size),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_command_read(&mut self) -> Result<Vec<u8>, String> {
        match self
            .call(Request::EffectCommandRead, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Data(chunk) => Ok(chunk.to_vec()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_command_write(&mut self, data: &[u8]) -> Result<(), String> {
        let chunk: Chunk = data
            .try_into()
            .map_err(|_| "Command data exceeds chunk size")?;
        match self
            .call(
                Request::EffectCommandWrite { data: chunk },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_parameter_stat(&mut self) -> Result<u64, String> {
        match self
            .call(Request::EffectParameterStat, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Size(size) => Ok(size),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_parameter_read(&mut self) -> Result<Vec<u8>, String> {
        match self
            .call(Request::EffectParameterRead, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::Data(chunk) => Ok(chunk.to_vec()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }

    pub async fn effect_parameter_write(&mut self, data: &[u8]) -> Result<(), String> {
        let chunk: Chunk = data
            .try_into()
            .map_err(|_| "Parameter data exceeds chunk size")?;
        match self
            .call(
                Request::EffectParameterWrite { data: chunk },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::Ok => Ok(()),
            res => Err(format!("Unexpected response: {res:?}")),
        }
    }
}
