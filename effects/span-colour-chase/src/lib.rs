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
        "name": "Span Col. Chase",
        "parameter_descriptors": [
            { "name": "Delay", "start": 0, "end": 15 },
            { "name": "Brightness", "start": 16, "end": 23 }
        ]
    }"#
);

const MAX_PIXELS: usize = 4096;
const FRAME_BYTES: usize = MAX_PIXELS * 3;
const PARAMETER_BYTES: usize = 3;

static mut FRAME: [u8; FRAME_BYTES] = [0; FRAME_BYTES];
static mut FRAME_LEN: usize = 0;
static mut PARAMETERS: [u8; PARAMETER_BYTES] = [0; PARAMETER_BYTES];
static mut CONFIG_CBOR: [u8; MAX_CONFIG_CBOR_SIZE] = [0; MAX_CONFIG_CBOR_SIZE];

static mut CONFIG: Option<DeviceConfig> = None;
static mut SPAN_INDEX: usize = 0;
static mut COLOUR_INDEX: usize = 0;

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

    // Set default parameters
    let delay_bytes = 500u16.to_le_bytes();
    unsafe {
        PARAMETERS[0..2].copy_from_slice(&delay_bytes);
        PARAMETERS[2] = 0x0F; // Dim enough you can actually look at the LEDs
    }

    // Clear frame buffer
    unsafe { FRAME.fill(0) }

    // Store config and reset the chase position
    unsafe {
        CONFIG = Some(config);
        SPAN_INDEX = 0;
        COLOUR_INDEX = 0;
    }

    InitResult::Success.into()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step_time() -> i32 {
    let bytes: [u8; 2] = unsafe { PARAMETERS[0..2].try_into().unwrap() };
    u16::from_le_bytes(bytes).try_into().unwrap()
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_step(_time_ms: i32) -> i32 {
    // Fail if init was not done
    let ready = unsafe { CONFIG.is_some() };
    if !ready {
        return StepResult::Error.into();
    }

    let brightness = unsafe { PARAMETERS[2] };
    let colours = [
        (brightness, 0, 0),                   // Red
        (0, brightness, 0),                   // Green
        (0, 0, brightness),                   // Blue
        (0, brightness, brightness),          // Cyan
        (brightness, 0, brightness),          // Magenta
        (brightness, brightness, 0),          // Yellow
        (brightness, brightness, brightness), // White
    ];

    let (mut span_index, mut colour_index) = unsafe { (SPAN_INDEX, COLOUR_INDEX) };
    let Some((total_spans, pixel_range)) = (unsafe {
        let config = CONFIG.as_ref().unwrap();
        let total_spans: usize = config
            .fixtures()
            .iter()
            .map(|fixture| fixture.spans().len())
            .sum();
        if total_spans == 0 {
            return StepResult::Unchanged.into();
        }

        let target_span = span_index % total_spans;
        let mut span_position = 0;
        let mut pixel_range = None;

        'fixtures: for fixture in config.fixtures() {
            for span in fixture.spans() {
                if span_position == target_span {
                    pixel_range = config.span_pixel_range::<4>(span);
                    break 'fixtures;
                }
                span_position += 1;
            }
        }

        pixel_range.map(|range| (total_spans, range))
    }) else {
        return StepResult::Error.into();
    };

    let (r, g, b) = colours[colour_index % colours.len()];
    for pixel in pixel_range {
        let offset = pixel as usize * 3;
        unsafe {
            FRAME[offset] = r;
            FRAME[offset + 1] = g;
            FRAME[offset + 2] = b;
        }
    }

    colour_index = (colour_index + 1) % colours.len();
    if colour_index == 0 {
        span_index = (span_index + 1) % total_spans;
    }

    unsafe {
        SPAN_INDEX = span_index;
        COLOUR_INDEX = colour_index;
    }

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
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_command_len() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_parameter_ptr() -> i32 {
    addr_of_mut!(PARAMETERS) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_parameter_len() -> i32 {
    PARAMETER_BYTES as i32
}
