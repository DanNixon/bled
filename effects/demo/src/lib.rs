#![no_std]

use bled_core::{config::DeviceConfig, config::MAX_CONFIG_CBOR_SIZE, io::decode_cbor};
use core::ptr::{addr_of_mut, write_bytes};

const ABI_VERSION: i32 = 1;
const MAX_PIXELS: u32 = 4096;
const FRAME_BYTES: usize = MAX_PIXELS as usize * 3;
const FRAME_INTERVAL_US: u64 = 250_000;
const ERROR_LAYOUT_SIZE: i32 = -1;
const ERROR_LAYOUT_READ: i32 = -2;
const ERROR_INVALID_LAYOUT: i32 = -3;
const ERROR_PIXEL_COUNT: i32 = -4;
const ERROR_TOO_MANY_PIXELS: i32 = -5;
const ERROR_NOT_INITIALIZED: i32 = -6;

#[link(wasm_import_module = "led_v1")]
unsafe extern "C" {
    fn pixel_count() -> i32;
    fn layout_size() -> i32;
    fn layout_read(src_offset: i32, dst_ptr: i32, length: i32) -> i32;
}

static mut FRAME: [u8; FRAME_BYTES] = [0; FRAME_BYTES];
static mut LAYOUT: [u8; MAX_CONFIG_CBOR_SIZE] = [0; MAX_CONFIG_CBOR_SIZE];
static mut LOGICAL_PIXEL_COUNT: u32 = 0;
static mut LAST_PIXEL: u32 = u32::MAX;
static mut READY: bool = false;

#[unsafe(no_mangle)]
pub extern "C" fn led_abi_version() -> i32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn led_init() -> i32 {
    let size = unsafe { layout_size() };
    if size <= 0 || size as usize > MAX_CONFIG_CBOR_SIZE {
        return ERROR_LAYOUT_SIZE;
    }

    let layout_ptr = addr_of_mut!(LAYOUT).cast::<u8>();
    let bytes_read = unsafe { layout_read(0, layout_ptr as usize as i32, size) };
    if bytes_read != size {
        return ERROR_LAYOUT_READ;
    }

    let layout_bytes = unsafe { core::slice::from_raw_parts(layout_ptr, size as usize) };
    let config = match decode_cbor::<DeviceConfig>(layout_bytes) {
        Ok(config) => config,
        Err(_) => return ERROR_INVALID_LAYOUT,
    };

    let configured_pixel_count = config.fixtures().iter().try_fold(0u32, |total, fixture| {
        fixture
            .spans()
            .iter()
            .try_fold(total, |total, span| total.checked_add(*span.length()))
    });
    let Some(configured_pixel_count) = configured_pixel_count else {
        return ERROR_INVALID_LAYOUT;
    };

    let host_pixel_count = unsafe { pixel_count() };
    if host_pixel_count < 0 || configured_pixel_count != host_pixel_count as u32 {
        return ERROR_PIXEL_COUNT;
    }
    if configured_pixel_count > MAX_PIXELS {
        return ERROR_TOO_MANY_PIXELS;
    }

    unsafe {
        LOGICAL_PIXEL_COUNT = configured_pixel_count;
        LAST_PIXEL = u32::MAX;
        READY = true;
        write_bytes(addr_of_mut!(FRAME).cast::<u8>(), 0, FRAME_BYTES);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn led_step(time_lo: i32, time_hi: i32, _dt_us: i32) -> i32 {
    let (pixel_count, ready) = unsafe { (LOGICAL_PIXEL_COUNT, READY) };
    if !ready {
        return ERROR_NOT_INITIALIZED;
    }

    if pixel_count == 0 {
        let is_initial_frame = unsafe { LAST_PIXEL == u32::MAX };
        unsafe { LAST_PIXEL = 0 };
        return if is_initial_frame { 0 } else { 1 };
    }

    let elapsed_us = ((time_hi as u32 as u64) << 32) | time_lo as u32 as u64;
    let current_pixel = ((elapsed_us / FRAME_INTERVAL_US) % pixel_count as u64) as u32;
    if unsafe { LAST_PIXEL == current_pixel } {
        return 1;
    }

    unsafe {
        let frame_ptr = addr_of_mut!(FRAME).cast::<u8>();
        write_bytes(frame_ptr, 0, pixel_count as usize * 3);
        frame_ptr.add(current_pixel as usize * 3).write(255);
        frame_ptr.add(current_pixel as usize * 3 + 1).write(255);
        frame_ptr.add(current_pixel as usize * 3 + 2).write(255);
        LAST_PIXEL = current_pixel;
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn led_frame_ptr() -> i32 {
    addr_of_mut!(FRAME) as usize as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn led_frame_len() -> i32 {
    (unsafe { LOGICAL_PIXEL_COUNT } * 3) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn led_input_ptr() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn led_input_capacity() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn led_apply_input(_kind: i32, length: i32) -> i32 {
    if length == 0 { 0 } else { -1 }
}
