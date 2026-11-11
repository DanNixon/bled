use super::device::ConnectedDevice;
use super::logger::{LogLevel, log};
use crate::transport::{RpcClient, bridge};
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeSelection {
    Device {
        device_id: String,
    },
    SdCard {
        device_id: String,
    },
    Effect {
        device_id: String,
    },
    Fixture {
        device_id: String,
        fixture_idx: usize,
    },
    Span {
        device_id: String,
        fixture_idx: usize,
        span_idx: usize,
    },
    Channel {
        device_id: String,
        channel_idx: usize,
    },
}

impl TreeSelection {
    pub fn device_id(&self) -> &str {
        match self {
            Self::Device { device_id }
            | Self::SdCard { device_id }
            | Self::Effect { device_id }
            | Self::Fixture { device_id, .. }
            | Self::Span { device_id, .. }
            | Self::Channel { device_id, .. } => device_id,
        }
    }
}

#[derive(Clone, Copy)]
pub struct DeviceRegistry {
    pub devices: RwSignal<Vec<ConnectedDevice>>,
    pub selection: RwSignal<Option<TreeSelection>>,
    pub pending: RwSignal<usize>,
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn js_error_to_string(err: wasm_bindgen::JsValue) -> String {
    js_sys::Object::from(err)
        .to_string()
        .as_string()
        .unwrap_or_default()
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: RwSignal::new(Vec::new()),
            selection: RwSignal::new(None),
            pending: RwSignal::new(0),
        }
    }

    pub fn selected_device(&self) -> Option<ConnectedDevice> {
        let sel = self.selection.get()?;
        let dev_id = sel.device_id();
        self.devices.get().into_iter().find(|d| d.id == dev_id)
    }

    pub fn disconnect_device(&self, device_id: &str) {
        if let Some(dev) = self.devices.get().into_iter().find(|d| d.id == device_id)
            && let Some(rpc) = dev.rpc.try_lock()
        {
            rpc.disconnect();
        }
        self.remove_device(device_id);
    }

    pub fn remove_device(&self, device_id: &str) {
        self.devices
            .update(|list| list.retain(|d| d.id != device_id));
        if self
            .selection
            .get_untracked()
            .map(|s| s.device_id() == device_id)
            .unwrap_or(false)
        {
            self.selection.set(None);
        }
    }

    fn attach_connected_device(
        &self,
        handle: wasm_bindgen::JsValue,
        dev_id_slot: &std::rc::Rc<std::cell::RefCell<Option<String>>>,
        success_msg_prefix: &str,
    ) {
        match RpcClient::new(handle) {
            Ok(rpc) => {
                let dev = ConnectedDevice::new(rpc);
                let dev_id = dev.id.clone();
                let dev_clone = dev.clone();

                self.devices.update(|list| list.push(dev));
                self.selection.set(Some(TreeSelection::Device {
                    device_id: dev_id.clone(),
                }));
                *dev_id_slot.borrow_mut() = Some(dev_id.clone());

                log(
                    format!("{success_msg_prefix} ({})", dev_clone.name),
                    LogLevel::Success,
                );
                spawn_local(async move {
                    dev_clone.refresh_info().await;
                    dev_clone.load_config().await;
                    dev_clone.refresh_led_buf_info().await;
                });
            }
            Err(e) => log(
                format!("Failed to initialize RPC client: {e}"),
                LogLevel::Error,
            ),
        }
    }

    pub async fn add_ble_device(&self) {
        let service_uuid = bled_core::ble::SERIAL_SERVICE_UUID.to_string();
        let rx_uuid = bled_core::ble::SERIAL_RX_UUID.to_string();
        let tx_uuid = bled_core::ble::SERIAL_TX_UUID.to_string();

        let registry = *self;
        let dev_id_slot = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
        let dev_id_clone = dev_id_slot.clone();
        let on_disconnected = Closure::wrap(Box::new(move || {
            log("Bluetooth device disconnected", LogLevel::Error);
            if let Some(id) = dev_id_clone.borrow().as_deref() {
                registry.remove_device(id);
            }
        }) as Box<dyn FnMut()>);

        self.pending.update(|p| *p += 1);
        let res = bridge::connect_ble_js(
            &service_uuid,
            &rx_uuid,
            &tx_uuid,
            on_disconnected.as_ref().unchecked_ref(),
        )
        .await;
        on_disconnected.forget();

        self.pending.update(|p| *p -= 1);

        match res {
            Ok(handle) => {
                registry.attach_connected_device(handle, &dev_id_slot, "Connected to");
            }
            Err(e) => {
                let err_str = js_error_to_string(e);
                if !err_str.contains("User cancelled") && !err_str.contains("NotFoundError") {
                    log(
                        format!("Bluetooth connection failed: {err_str}"),
                        LogLevel::Error,
                    );
                }
            }
        }
    }

    pub async fn add_serial_device(&self) {
        let registry = *self;
        let dev_id_slot = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
        let dev_id_clone = dev_id_slot.clone();
        let on_disconnected = Closure::wrap(Box::new(move || {
            log("Serial device disconnected", LogLevel::Error);
            if let Some(id) = dev_id_clone.borrow().as_deref() {
                registry.remove_device(id);
            }
        }) as Box<dyn FnMut()>);

        self.pending.update(|p| *p += 1);
        let res =
            bridge::connect_serial_js(0xc0de, 0xcafe, on_disconnected.as_ref().unchecked_ref())
                .await;
        on_disconnected.forget();

        self.pending.update(|p| *p -= 1);

        match res {
            Ok(handle) => {
                registry.attach_connected_device(handle, &dev_id_slot, "Connected over USB Serial");
            }
            Err(e) => {
                let err_str = js_error_to_string(e);
                if !err_str.contains("User cancelled") && !err_str.contains("NotFoundError") {
                    log(
                        format!("Serial connection failed: {err_str}"),
                        LogLevel::Error,
                    );
                }
            }
        }
    }
}
