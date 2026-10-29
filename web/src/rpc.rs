//! Web Bluetooth stream transport and typed RPC client.

use bled_core::{
    PathString,
    api::{
        Chunk, MAX_CHUNK_SIZE, RPC_ACK_TIMEOUT, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN,
        RPC_LOAD_TIMEOUT, RPC_RESPONSE_TIMEOUT, Request, Response,
    },
};
use core::time::Duration;
use heapless::Vec as HeaplessVec;
use js_sys::{Function, Promise, Uint8Array};
use serde::Serialize;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

type Rpc =
    teeny_rpc::Client<JsSerialTransport, Request, Response, RPC_CHANNEL_ID, RPC_FRAME_BUF_LEN>;

/// Byte-stream adapter whose callbacks are implemented by the Web Bluetooth
/// service. JavaScript supplies complete zero-delimited frames to the RPC
/// client while retaining any trailing bytes for the next call.
struct JsSerialTransport {
    read_frame: Function,
    write_frame: Function,
    flush_rx: Function,
    frame: HeaplessVec<u8, RPC_FRAME_BUF_LEN>,
}

impl JsSerialTransport {
    async fn invoke_write(&self, frame: &[u8]) -> Result<(), teeny_rpc::Error> {
        let bytes = Uint8Array::from(frame);
        let result = self
            .write_frame
            .call1(&JsValue::NULL, &bytes.into())
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

impl teeny_rpc::transport::Transport for JsSerialTransport {
    async fn flush(&mut self, _timeout: Duration) -> Result<usize, teeny_rpc::Error> {
        self.flush_rx
            .call0(&JsValue::NULL)
            .map_err(|_| teeny_rpc::Error::TransportError)?
            .as_f64()
            .map(|count| count as usize)
            .ok_or(teeny_rpc::Error::TransportError)
    }

    async fn receive_frame(&mut self, timeout: Duration) -> Result<&mut [u8], teeny_rpc::Error> {
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        let result = self
            .read_frame
            .call1(&JsValue::NULL, &JsValue::from_f64(f64::from(timeout_ms)))
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
        self.invoke_write(frame).await
    }
}

/// Stateful RPC client for one connected emulated-serial device.
#[wasm_bindgen]
pub struct RpcClient {
    client: Rpc,
}

#[wasm_bindgen]
impl RpcClient {
    #[wasm_bindgen(constructor)]
    pub fn new(read_frame: Function, write_frame: Function, flush_rx: Function) -> Self {
        let transport = JsSerialTransport {
            read_frame,
            write_frame,
            flush_rx,
            frame: HeaplessVec::new(),
        };
        Self {
            client: Rpc::new(transport, RPC_ACK_TIMEOUT),
        }
    }

    pub async fn read_device_info(&mut self) -> Result<JsValue, JsError> {
        match self
            .call(Request::GetDeviceInfo, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::DeviceInfo(info) => to_js(&info),
            response => unexpected_response(response),
        }
    }

    pub async fn reboot(&mut self) -> Result<(), JsError> {
        expect_ok(self.call(Request::Reboot, RPC_RESPONSE_TIMEOUT).await?)
    }

    pub async fn config_stat(&mut self) -> Result<f64, JsError> {
        size(self.call(Request::ConfigStat, RPC_RESPONSE_TIMEOUT).await?)
    }

    pub async fn config_read(&mut self, offset: u32, length: u32) -> Result<Uint8Array, JsError> {
        data(
            self.call(Request::ConfigRead { offset, length }, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn commit(&mut self) -> Result<(), JsError> {
        expect_ok(self.call(Request::Commit, RPC_RESPONSE_TIMEOUT).await?)
    }

    pub async fn file_stat(&mut self, path: &str) -> Result<f64, JsError> {
        size(
            self.call(
                Request::FileStat {
                    path: path_string(path)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn file_read(
        &mut self,
        path: &str,
        offset: u32,
        length: u32,
    ) -> Result<Uint8Array, JsError> {
        data(
            self.call(
                Request::FileRead {
                    path: path_string(path)?,
                    offset,
                    length,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn file_write(
        &mut self,
        path: &str,
        offset: u32,
        bytes: &[u8],
    ) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::FileWrite {
                    path: path_string(path)?,
                    offset,
                    data: chunk(bytes)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn file_create(&mut self, path: &str, size: u32) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::FileCreate {
                    path: path_string(path)?,
                    size,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn file_delete(&mut self, path: &str) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::FileDelete {
                    path: path_string(path)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn read_dir_entry(&mut self, path: &str, index: u32) -> Result<JsValue, JsError> {
        match self
            .call(
                Request::ReadDirEntry {
                    path: path_string(path)?,
                    index,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?
        {
            Response::DirEntry(entry) => to_js(&entry),
            response => unexpected_response(response),
        }
    }

    pub async fn led_buffer_info(&mut self) -> Result<JsValue, JsError> {
        match self
            .call(Request::LedBufferInfo, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::LedBufferInfo(info) => to_js(&info),
            response => unexpected_response(response),
        }
    }

    pub async fn led_buffer_read(
        &mut self,
        offset: u32,
        length: u32,
    ) -> Result<Uint8Array, JsError> {
        data(
            self.call(
                Request::LedBufferRead { offset, length },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn led_buffer_write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::LedBufferWrite {
                    offset,
                    data: chunk(bytes)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn get_active_effect(&mut self) -> Result<JsValue, JsError> {
        match self
            .call(Request::GetActiveEffect, RPC_RESPONSE_TIMEOUT)
            .await?
        {
            Response::ActiveEffect(path) => to_js(&path),
            response => unexpected_response(response),
        }
    }

    pub async fn set_active_effect(&mut self, path: Option<String>) -> Result<(), JsError> {
        let path = path.as_deref().map(path_string).transpose()?;
        expect_ok(
            self.call(Request::SetActiveEffect { path }, RPC_LOAD_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_manifest_stat(&mut self) -> Result<f64, JsError> {
        size(
            self.call(Request::EffectManifestStat, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_manifest_read(
        &mut self,
        offset: u32,
        length: u32,
    ) -> Result<Uint8Array, JsError> {
        data(
            self.call(
                Request::EffectManifestRead { offset, length },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn effect_command_stat(&mut self) -> Result<f64, JsError> {
        size(
            self.call(Request::EffectCommandStat, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_command_read(&mut self) -> Result<Uint8Array, JsError> {
        data(
            self.call(Request::EffectCommandRead, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_command_write(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::EffectCommandWrite {
                    data: chunk(bytes)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }

    pub async fn effect_parameter_stat(&mut self) -> Result<f64, JsError> {
        size(
            self.call(Request::EffectParameterStat, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_parameter_read(&mut self) -> Result<Uint8Array, JsError> {
        data(
            self.call(Request::EffectParameterRead, RPC_RESPONSE_TIMEOUT)
                .await?,
        )
    }

    pub async fn effect_parameter_write(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        expect_ok(
            self.call(
                Request::EffectParameterWrite {
                    data: chunk(bytes)?,
                },
                RPC_RESPONSE_TIMEOUT,
            )
            .await?,
        )
    }
}

impl RpcClient {
    async fn call(&mut self, request: Request, timeout: Duration) -> Result<Response, JsError> {
        let response = self
            .client
            .call(request, timeout)
            .await
            .map_err(|error| JsError::new(&format!("RPC failed: {error}")))?;
        match response {
            Response::Error(error) => Err(JsError::new(&format!("device API error: {error:?}"))),
            response => Ok(response),
        }
    }
}

fn path_string(path: &str) -> Result<PathString, JsError> {
    if path.is_empty() {
        return Err(JsError::new("remote path cannot be empty"));
    }
    path.try_into()
        .map_err(|_| JsError::new("remote path exceeds maximum length or is invalid"))
}

fn chunk(bytes: &[u8]) -> Result<Chunk, JsError> {
    if bytes.len() > MAX_CHUNK_SIZE {
        return Err(JsError::new("RPC data chunk exceeds maximum size"));
    }
    Chunk::try_from(bytes).map_err(|_| JsError::new("RPC data chunk exceeds maximum size"))
}

fn expect_ok(response: Response) -> Result<(), JsError> {
    match response {
        Response::Ok => Ok(()),
        response => unexpected_response(response),
    }
}

fn size(response: Response) -> Result<f64, JsError> {
    match response {
        Response::Size(size) => Ok(size as f64),
        response => unexpected_response(response),
    }
}

fn data(response: Response) -> Result<Uint8Array, JsError> {
    match response {
        Response::Data(bytes) => Ok(Uint8Array::from(bytes.as_slice())),
        response => unexpected_response(response),
    }
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|error| JsError::new(&format!("failed to convert RPC response: {error}")))
}

fn unexpected_response<T>(response: Response) -> Result<T, JsError> {
    Err(JsError::new(&format!(
        "unexpected RPC response: {response:?}"
    )))
}
