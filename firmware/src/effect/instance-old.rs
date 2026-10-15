use crate::leds::CHANNEL_COUNT;
use alloc::vec::Vec;
use bled_core::{
    config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE},
    effect::abi,
};
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

const MAX_TABLE_ELEMENTS: usize = 1024;
const INVALID_IMPORT_RESULT: i32 = -1;

fn encode_device_config(config: &DeviceConfig) -> Result<(Vec<u8>, i32), EffectError> {
    let pixel_count = config
        .total_length::<{ CHANNEL_COUNT }>()
        .try_into()
        .unwrap();

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_CONFIG_CBOR_SIZE)
        .map_err(|_| EffectError::AllocationFailed)?;
    bytes.resize(MAX_CONFIG_CBOR_SIZE, 0);
    let length = bled_core::io::encode_cbor(config, &mut bytes)
        .map_err(|_| EffectError::ConfigurationTooLarge)?;
    bytes.truncate(length);

    Ok((bytes, pixel_count))
}

/// Result of advancing an effect by one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    /// The guest reports that the frame buffer has changed.
    Changed,
    /// The guest reports that the frame buffer has not changed.
    Unchanged,
}

#[derive(Debug)]
struct HostState {
    config: Vec<u8>,
    limits: StoreLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BufferDescriptors {
    frame_ptr: u32,
    frame_len: u32,
    command_ptr: u32,
    parameter_ptr: u32,
}

impl EffectInstance {
    /// Advances the effect with elapsed time in milliseconds.
    ///
    /// If the guest traps or violates the buffer contract, this instance is
    /// marked inactive and should be dropped.
    pub fn step(&mut self, time_ms: i32) -> Result<StepResult, EffectError> {
        self.ensure_active()?;
        let status = match call_guest(&mut self.store, &mut self.active, &self.step, (time_ms,)) {
            Ok(status) => status,
            Err(error) => {
                self.active = false;
                return Err(error);
            }
        };
        let result = self.validate_step_result(status)?;
        if let Err(error) = self.ensure_descriptors_stable() {
            self.active = false;
            return Err(error);
        }
        if result == StepResult::Changed
            && let Err(error) = self.copy_frame()
        {
            self.active = false;
            return Err(error);
        }
        Ok(result)
    }

    /// Returns the guest's requested interval until the next step, in milliseconds.
    pub fn step_time(&mut self) -> Result<i32, EffectError> {
        self.ensure_active()?;
        let time_ms = call_guest(&mut self.store, &mut self.active, &self.step_time, ())?;
        if time_ms <= 0 {
            self.active = false;
            return Err(EffectError::InvalidStepTime(time_ms));
        }
        Ok(time_ms)
    }

    fn validate_step_result(&mut self, status: i32) -> Result<StepResult, EffectError> {
        match status {
            -1 => {
                self.active = false;
                Err(EffectError::StepFailed)
            }
            0 => Ok(StepResult::Unchanged),
            1 => Ok(StepResult::Changed),
            status => {
                self.active = false;
                Err(EffectError::InvalidStepResult(status))
            }
        }
    }

    /// Returns the most recently copied host-owned packed RGB8 framebuffer.
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    /// Returns the logical number of RGB pixels in this effect's framebuffer.
    pub fn pixel_count(&self) -> u32 {
        self.buffers.frame_len / 3
    }

    fn read_buffer_descriptors(&mut self) -> Result<BufferDescriptors, EffectError> {
        let frame_ptr = call_guest(&mut self.store, &mut self.active, &self.frame_ptr, ())? as u32;
        let frame_len = call_guest(&mut self.store, &mut self.active, &self.frame_len, ())? as u32;
        let command_ptr =
            call_guest(&mut self.store, &mut self.active, &self.command_ptr, ())? as u32;
        let parameter_ptr =
            call_guest(&mut self.store, &mut self.active, &self.parameter_ptr, ())? as u32;
        Ok(BufferDescriptors {
            frame_ptr,
            frame_len,
            command_ptr,
            parameter_ptr,
        })
    }

    fn copy_frame(&mut self) -> Result<(), EffectError> {
        self.memory
            .read(
                &self.store,
                self.buffers.frame_ptr as usize,
                &mut self.frame,
            )
            .map_err(|_| EffectError::InvalidBuffer)
    }
}

fn call_guest<Params, Results>(
    store: &mut Store<HostState>,
    active: &mut bool,
    function: &TypedFunc<Params, Results>,
    params: Params,
) -> Result<Results, EffectError>
where
    Params: wasmi::WasmParams,
    Results: wasmi::WasmResults,
{
    if !*active {
        return Err(EffectError::Inactive);
    }
    if let Err(error) = store.set_fuel(FUEL_PER_CALL) {
        *active = false;
        return Err(EffectError::Wasm(error));
    }
    function.call(store, params).map_err(|error| {
        *active = false;
        EffectError::Wasm(error)
    })
}

fn contains_start_section(wasm: &[u8]) -> Result<bool, EffectError> {
    if wasm.get(..8) != Some(b"\0asm\x01\0\0\0") {
        return Err(EffectError::InvalidWasmBinary);
    }

    let mut offset = 8usize;
    while offset < wasm.len() {
        let section_id = *wasm.get(offset).ok_or(EffectError::InvalidWasmBinary)?;
        offset += 1;
        let section_size = read_leb_u32(wasm, &mut offset).ok_or(EffectError::InvalidWasmBinary)?;
        let section_end = offset
            .checked_add(section_size as usize)
            .filter(|end| *end <= wasm.len())
            .ok_or(EffectError::InvalidWasmBinary)?;
        if section_id == 8 {
            return Ok(true);
        }
        offset = section_end;
    }
    Ok(false)
}

fn read_leb_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let mut result = 0u32;
    for shift in [0, 7, 14, 21, 28] {
        let byte = *bytes.get(*offset)?;
        *offset = offset.checked_add(1)?;
        if shift == 28 && byte & 0xf0 != 0 {
            return None;
        }
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
    }
    None
}
