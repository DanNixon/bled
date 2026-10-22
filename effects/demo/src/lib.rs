#![no_std]

use bled_core::{
    config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE},
    effect::abi::export::{InitResult, StepResult},
    wasm_effect,
};
use core::ptr::{addr_of_mut, write_bytes};

wasm_effect!(
    br#"{
        "name": "Pixel Sweep Demo",
        "command_descriptors": [],
        "parameter_descriptors": [
            { "name": "Red", "start": 0, "end": 7 },
            { "name": "Green", "start": 8, "end": 15 },
            { "name": "Blue", "start": 16, "end": 23 }
        ]
    }"#
);

const MAX_PIXELS: u32 = 4096;
const FRAME_BYTES: usize = MAX_PIXELS as usize * 3;
const COMMAND_BYTES: usize = 1;
const PARAMETER_BYTES: usize = 3;

static mut FRAME: [u8; FRAME_BYTES] = [0; FRAME_BYTES];
static mut COMMAND: [u8; COMMAND_BYTES] = [0; COMMAND_BYTES];
static mut PARAMETERS: [u8; PARAMETER_BYTES] = [0; PARAMETER_BYTES];
static mut CONFIG: [u8; MAX_CONFIG_CBOR_SIZE] = [0; MAX_CONFIG_CBOR_SIZE];

static mut LOGICAL_PIXEL_COUNT: u32 = 0;
static mut LAST_PIXEL: u32 = u32::MAX;
static mut READY: bool = false;

#[unsafe(no_mangle)]
pub extern "C" fn bled_init() -> i32 {
    let size = unsafe { bled_config_size() };
    if size <= 0 || size as usize > MAX_CONFIG_CBOR_SIZE {
        return InitResult::ConfigParseError.into();
    }

    let config_ptr = addr_of_mut!(CONFIG).cast::<u8>();
    let bytes_read = unsafe { bled_config_read(config_ptr as usize as i32) };
    if bytes_read != size {
        return InitResult::ConfigParseError.into();
    }

    let config_bytes = unsafe { core::slice::from_raw_parts(config_ptr, size as usize) };
    let config = match bled_core::io::decode_cbor::<DeviceConfig>(config_bytes) {
        Ok(config) => config,
        Err(_) => return InitResult::ConfigParseError.into(),
    };

    let configured_pixel_count = config.fixtures().iter().try_fold(0u32, |total, fixture| {
        fixture
            .spans()
            .iter()
            .try_fold(total, |total, span| total.checked_add(*span.length()))
    });
    let Some(configured_pixel_count) = configured_pixel_count else {
        return InitResult::ConfigParseError.into();
    };

    let host_pixel_count = config.total_length::<4>();
    if configured_pixel_count != host_pixel_count.try_into().unwrap() {
        return InitResult::FrameBufferError.into();
    }
    if configured_pixel_count > MAX_PIXELS {
        return InitResult::FrameBufferError.into();
    }

    unsafe {
        let param_ptr = addr_of_mut!(PARAMETERS).cast::<u8>();
        write_bytes(param_ptr, 255, PARAMETER_BYTES);

        LOGICAL_PIXEL_COUNT = configured_pixel_count;
        LAST_PIXEL = u32::MAX;
        READY = true;
        write_bytes(addr_of_mut!(FRAME).cast::<u8>(), 0, FRAME_BYTES);
    }

    InitResult::Success.into()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step_time() -> i32 {
    250
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step(_time_ms: i32) -> i32 {
    let (pixel_count, ready, last_pixel) = unsafe { (LOGICAL_PIXEL_COUNT, READY, LAST_PIXEL) };
    if pixel_count == 0 {
        return StepResult::Unchanged.into();
    }
    if !ready {
        return StepResult::Error.into();
    }

    let mut current_pixel = last_pixel.saturating_add(1);
    if current_pixel >= pixel_count {
        current_pixel = 0;
    }

    unsafe {
        let param_ptr = addr_of_mut!(PARAMETERS).cast::<u8>();
        let r = param_ptr.read();
        let g = param_ptr.add(1).read();
        let b = param_ptr.add(2).read();

        let frame_ptr = addr_of_mut!(FRAME).cast::<u8>();
        write_bytes(frame_ptr, 0, pixel_count as usize * 3);
        frame_ptr.add(current_pixel as usize * 3).write(r);
        frame_ptr.add(current_pixel as usize * 3 + 1).write(g);
        frame_ptr.add(current_pixel as usize * 3 + 2).write(b);
        LAST_PIXEL = current_pixel;
    }

    StepResult::Changed.into()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_frame_ptr() -> i32 {
    addr_of_mut!(FRAME) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_frame_len() -> i32 {
    (unsafe { LOGICAL_PIXEL_COUNT } * 3).try_into().unwrap()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_command_ptr() -> i32 {
    addr_of_mut!(COMMAND) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_command_len() -> i32 {
    COMMAND_BYTES as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_parameter_ptr() -> i32 {
    addr_of_mut!(PARAMETERS) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_parameter_len() -> i32 {
    PARAMETER_BYTES as i32
}
