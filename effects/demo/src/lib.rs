#![no_std]

use bled_core::{
    config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE},
    effect::abi::{
        self,
        export::{InitResult, StepResult},
    },
    io::decode_cbor,
};
use core::{
    panic::PanicInfo,
    ptr::{addr_of_mut, write_bytes},
};

const MAX_PIXELS: u32 = 4096;
const FRAME_BYTES: usize = MAX_PIXELS as usize * 3;
const FRAME_INTERVAL_US: u64 = 250_000;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}

#[link(wasm_import_module = "bled_v1")]
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
pub extern "C" fn bled_abi_version() -> i32 {
    abi::VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_init() -> i32 {
    let size = unsafe { layout_size() };
    if size <= 0 || size as usize > MAX_CONFIG_CBOR_SIZE {
        return InitResult::LayoutParseError.into();
    }

    let layout_ptr = addr_of_mut!(LAYOUT).cast::<u8>();
    let bytes_read = unsafe { layout_read(0, layout_ptr as usize as i32, size) };
    if bytes_read != size {
        return InitResult::LayoutParseError.into();
    }

    let layout_bytes = unsafe { core::slice::from_raw_parts(layout_ptr, size as usize) };
    let config = match decode_cbor::<DeviceConfig>(layout_bytes) {
        Ok(config) => config,
        Err(_) => return InitResult::LayoutParseError.into(),
    };

    let configured_pixel_count = config.fixtures().iter().try_fold(0u32, |total, fixture| {
        fixture
            .spans()
            .iter()
            .try_fold(total, |total, span| total.checked_add(*span.length()))
    });
    let Some(configured_pixel_count) = configured_pixel_count else {
        return InitResult::LayoutParseError.into();
    };

    let host_pixel_count = unsafe { pixel_count() };
    if host_pixel_count < 0 || configured_pixel_count != host_pixel_count as u32 {
        return InitResult::FrameBufferError.into();
    }
    if configured_pixel_count > MAX_PIXELS {
        return InitResult::FrameBufferError.into();
    }

    unsafe {
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
pub extern "C" fn bled_step(time_lo: i32, time_hi: i32, _dt_us: i32) -> i32 {
    let (pixel_count, ready) = unsafe { (LOGICAL_PIXEL_COUNT, READY) };
    if !ready {
        return StepResult::Error.into();
    }

    if pixel_count == 0 {
        let is_initial_frame = unsafe { LAST_PIXEL == u32::MAX };
        unsafe { LAST_PIXEL = 0 };
        return if is_initial_frame {
            StepResult::Changed.into()
        } else {
            StepResult::Unchanged.into()
        };
    }

    let elapsed_us = ((time_hi as u32 as u64) << 32) | time_lo as u32 as u64;
    let current_pixel = ((elapsed_us / FRAME_INTERVAL_US) % pixel_count as u64) as u32;
    if unsafe { LAST_PIXEL == current_pixel } {
        return StepResult::Unchanged.into();
    }

    unsafe {
        let frame_ptr = addr_of_mut!(FRAME).cast::<u8>();
        write_bytes(frame_ptr, 0, pixel_count as usize * 3);
        frame_ptr.add(current_pixel as usize * 3).write(255);
        frame_ptr.add(current_pixel as usize * 3 + 1).write(255);
        frame_ptr.add(current_pixel as usize * 3 + 2).write(255);
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
pub extern "C" fn bled_parameter_ptr() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn bled_parameter_capacity() -> i32 {
    0
}
