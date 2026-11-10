use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/transport.js")]
extern "C" {
    #[wasm_bindgen(js_name = isBleSupported)]
    pub fn is_ble_supported() -> bool;

    #[wasm_bindgen(js_name = isSerialSupported)]
    pub fn is_serial_supported() -> bool;

    #[wasm_bindgen(js_name = isSecureContext)]
    pub fn is_secure_context() -> bool;

    #[wasm_bindgen(js_name = connectBle, catch)]
    pub async fn connect_ble_js(
        service_uuid: &str,
        rx_uuid: &str,
        tx_uuid: &str,
        on_disconnected: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = connectSerial, catch)]
    pub async fn connect_serial_js(
        vendor_id: u32,
        product_id: u32,
        on_disconnected: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = saveFileToDisk)]
    pub fn save_file_to_disk(data: &[u8], filename: &str);
}
