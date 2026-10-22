#![no_std]
#![allow(static_mut_refs)]

use bled_core::{
    config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE},
    effect::abi::export::{InitResult, StepResult},
    wasm_effect,
};
use core::ptr::addr_of_mut;

wasm_effect!(
    br#"{
        "name": "Span Painter",
        "command_descriptors": [
            { "name": "Draw", "start": 0, "end": 0 },
            { "name": "Fixture", "start": 8, "end": 15 },
            { "name": "Span", "start": 16, "end": 23 }
        ],
        "parameter_descriptors": [
            { "name": "Red", "start": 0, "end": 7 },
            { "name": "Green", "start": 8, "end": 15 },
            { "name": "Blue", "start": 16, "end": 23 }
        ]
    }"#
);

const MAX_PIXELS: usize = 4096;
const FRAME_BYTES: usize = MAX_PIXELS * 3;
const COMMAND_BYTES: usize = 3;
const PARAMETER_BYTES: usize = 3;

static mut FRAME: [u8; FRAME_BYTES] = [0; FRAME_BYTES];
static mut FRAME_LEN: usize = 0;
static mut COMMAND: [u8; COMMAND_BYTES] = [0; COMMAND_BYTES];
static mut PARAMETERS: [u8; PARAMETER_BYTES] = [0; PARAMETER_BYTES];
static mut CONFIG_CBOR: [u8; MAX_CONFIG_CBOR_SIZE] = [0; MAX_CONFIG_CBOR_SIZE];

static mut CONFIG: Option<DeviceConfig> = None;

#[unsafe(no_mangle)]
pub extern "C" fn bled_init() -> i32 {
    // Get config size
    let size = unsafe { bled_config_size() };
    if size <= 0 || size as usize > MAX_CONFIG_CBOR_SIZE {
        return InitResult::ConfigParseError.into();
    }

    // Read config CBOR data
    let config_ptr = addr_of_mut!(CONFIG_CBOR).cast::<u8>();
    let bytes_read = unsafe { bled_config_read(config_ptr as usize as i32) };
    if bytes_read != size {
        return InitResult::ConfigParseError.into();
    }

    // Parse config
    let config_bytes = unsafe { &CONFIG_CBOR[..size as usize] };
    let config = match bled_core::io::decode_cbor::<DeviceConfig>(config_bytes) {
        Ok(config) => config,
        Err(_) => return InitResult::ConfigParseError.into(),
    };

    // Ensure frame buffer is large enough
    let pixel_count = config.total_length::<4>();
    if pixel_count > MAX_PIXELS {
        return InitResult::FrameBufferError.into();
    }
    unsafe { FRAME_LEN = pixel_count * 3 }

    // Set default colour to white
    unsafe { PARAMETERS[0..3].fill(255) }

    // Clear frame buffer
    unsafe { FRAME.fill(0) }

    // Store config
    unsafe { CONFIG = Some(config) }

    InitResult::Success.into()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step_time() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step(_time_ms: i32) -> i32 {
    // Fail if init was not done
    let ready = unsafe { CONFIG.is_some() };
    if !ready {
        return StepResult::Error.into();
    }

    // Return early if no command is set
    let command = unsafe { COMMAND[0] };
    if command == 0 {
        return StepResult::Unchanged.into();
    }

    // Get fixture and span indices from command buffer
    let fixture_idx = unsafe { COMMAND[1] };
    let span_idx = unsafe { COMMAND[2] };

    // Get colour from parameter buffer
    let (r, g, b) = unsafe { (PARAMETERS[0], PARAMETERS[1], PARAMETERS[2]) };

    // Get the range of pixels in the span
    let Some(pixel_range) = (unsafe {
        let config = CONFIG.as_ref().unwrap();
        let span = config.fixtures()[fixture_idx as usize].spans()[span_idx as usize].clone();
        config.span_pixel_range::<4>(&span)
    }) else {
        return StepResult::Error.into();
    };

    // Set pixel colours in frame buffer
    for pixel in pixel_range {
        let pixel = pixel as usize;

        let r_offset = pixel * 3;
        let g_offset = r_offset + 1;
        let b_offset = r_offset + 2;

        unsafe {
            FRAME[r_offset] = r;
            FRAME[g_offset] = g;
            FRAME[b_offset] = b;
        }
    }

    // Reset the draw command byte
    unsafe { COMMAND[0] = 0 };

    StepResult::Changed.into()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_frame_ptr() -> i32 {
    addr_of_mut!(FRAME) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_frame_len() -> i32 {
    (unsafe { FRAME_LEN }).try_into().unwrap()
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
