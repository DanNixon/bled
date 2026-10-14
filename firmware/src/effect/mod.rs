//! WebAssembly effect ABI v1 host implementation.
//!
//! The host instantiates modules without an application start function, checks
//! the ABI version, invokes `led_init`, validates the guest-owned buffers, and
//! makes one initial `led_step(0, 0, 0)` call. Frames returned by [`EffectInstance`]
//! are host-owned RGB8 copies in logical pixel order; mapping them to physical
//! LED channels is the caller's responsibility. Frame channels are normalized
//! 8-bit LED drive values (not necessarily sRGB); any host gamma policy should
//! be applied consistently regardless of the pattern.
//!
//! The CBOR-encoded [`DeviceConfig`] is exposed to guests through
//! `led_v1.layout_size` and `led_v1.layout_read`. Logical pixels are ordered by
//! fixture and then by span.

use alloc::vec::Vec;
use bled_core::config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE};
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

/// Current WebAssembly effects ABI version.
pub const ABI_VERSION: i32 = 1;

/// Maximum linear memory available to an effect instance.
pub const MAX_MEMORY_BYTES: usize = 64 * 1024;

/// Instruction budget for each guest function call.
pub const FUEL_PER_CALL: u64 = 1_000_000;

const MAX_TABLE_ELEMENTS: usize = 1024;
const INVALID_IMPORT_RESULT: i32 = -1;

fn encode_device_config(config: &DeviceConfig) -> Result<(Vec<u8>, i32), EffectError> {
    let pixel_count = config.fixtures().iter().try_fold(0u32, |count, fixture| {
        fixture
            .spans()
            .iter()
            .try_fold(count, |count, span| count.checked_add(*span.length()))
    });
    let pixel_count = pixel_count.ok_or(EffectError::ConfigurationTooLarge)?;
    let pixel_count = i32::try_from(pixel_count).map_err(|_| EffectError::ConfigurationTooLarge)?;

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
    /// A new frame is ready and has been copied into the host-owned buffer.
    FrameReady,
    /// The guest reports that the frame has not changed.
    Unchanged,
}

/// Errors returned while validating, running, or exchanging data with an effect.
#[derive(Debug)]
pub enum EffectError {
    /// Wasmi rejected the module or a guest function trapped.
    Wasm(wasmi::Error),
    /// Defining an ABI host import failed.
    Linker(wasmi::errors::LinkerError),
    /// The module is not a valid WebAssembly v1 binary.
    InvalidWasmBinary,
    /// Application initialization via a Wasm start section is unsupported.
    StartFunctionNotAllowed,
    /// The guest reported an ABI version other than version 1.
    AbiVersionMismatch(i32),
    /// `led_init` returned a non-zero result.
    InitializationFailed(i32),
    /// A guest step returned a negative pattern error code.
    PatternError(i32),
    /// A guest returned an invalid pointer, length, or descriptor.
    InvalidBuffer,
    /// The guest framebuffer length was not exactly three bytes per pixel.
    InvalidFrameLength { expected: u32, actual: u32 },
    /// The framebuffer and input buffer overlap.
    OverlappingBuffers,
    /// The guest changed a buffer descriptor after initialization.
    UnstableBufferDescriptor,
    /// The CBOR configuration or logical pixel count cannot be represented by this ABI.
    ConfigurationTooLarge,
    /// A requested host-side allocation failed.
    AllocationFailed,
    /// The effect has trapped or has been deinitialized and cannot be called.
    Inactive,
}

#[derive(Debug)]
struct HostState {
    config: Vec<u8>,
    pixel_count: i32,
    limits: StoreLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BufferDescriptors {
    frame_ptr: u32,
    frame_len: u32,
    input_ptr: u32,
    input_capacity: u32,
}

/// An instantiated, ABI-checked effect.
///
/// Calls are synchronous and require `&mut self`, which serializes command
/// delivery and animation steps. Dropping this value always destroys the Wasm
/// instance; [`Self::deinit`] may be called first when the guest exports
/// `led_deinit`.
pub struct EffectInstance {
    store: Store<HostState>,
    memory: Memory,
    abi_version: TypedFunc<(), i32>,
    init: TypedFunc<(), i32>,
    step: TypedFunc<(i32, i32, i32), i32>,
    frame_ptr: TypedFunc<(), i32>,
    frame_len: TypedFunc<(), i32>,
    input_ptr: TypedFunc<(), i32>,
    input_capacity: TypedFunc<(), i32>,
    apply_input: TypedFunc<(i32, i32), i32>,
    deinit: Option<TypedFunc<(), ()>>,
    buffers: BufferDescriptors,
    frame: Vec<u8>,
    active: bool,
}

impl EffectInstance {
    /// Compiles and initializes an effect from a WebAssembly binary.
    ///
    /// This method follows the ABI lifecycle through the mandatory initial
    /// zero-time step and stores that first RGB8 frame in host-owned memory.
    pub fn load(wasm_bytes: &[u8], device_config: &DeviceConfig) -> Result<Self, EffectError> {
        let (config_bytes, pixel_count) = encode_device_config(device_config)?;
        if contains_start_section(wasm_bytes)? {
            return Err(EffectError::StartFunctionNotAllowed);
        }

        let mut config = Config::default();
        config
            .consume_fuel(true)
            .wasm_sign_extension(false)
            .wasm_saturating_float_to_int(false)
            .wasm_multi_value(false)
            .wasm_multi_memory(false)
            .wasm_bulk_memory(false)
            .wasm_reference_types(false)
            .wasm_tail_call(false)
            .wasm_extended_const(false)
            .wasm_custom_page_sizes(false)
            .wasm_wide_arithmetic(false)
            .set_max_stack_height(16 * 1024)
            .set_max_cached_stacks(1);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm_bytes).map_err(EffectError::Wasm)?;

        let mut store = Store::new(
            &engine,
            HostState {
                config: config_bytes,
                pixel_count,
                limits: StoreLimitsBuilder::new()
                    .memory_size(MAX_MEMORY_BYTES)
                    .table_elements(MAX_TABLE_ELEMENTS)
                    .instances(1)
                    .memories(1)
                    .tables(1)
                    .build(),
            },
        );
        store.limiter(|state| &mut state.limits);

        let mut linker = Linker::<HostState>::new(&engine);
        linker
            .func_wrap("led_v1", "pixel_count", |caller: Caller<'_, HostState>| {
                caller.data().pixel_count
            })
            .map_err(EffectError::Linker)?;
        linker
            .func_wrap("led_v1", "layout_size", |caller: Caller<'_, HostState>| {
                i32::try_from(caller.data().config.len()).unwrap_or(INVALID_IMPORT_RESULT)
            })
            .map_err(EffectError::Linker)?;
        linker
            .func_wrap(
                "led_v1",
                "layout_read",
                |mut caller: Caller<'_, HostState>, src_offset: i32, dst_ptr: i32, length: i32| {
                    let src_start = src_offset as u32 as usize;
                    let dst_start = dst_ptr as u32 as usize;
                    let byte_count = length as u32 as usize;
                    let Some(src_end) = src_start.checked_add(byte_count) else {
                        return INVALID_IMPORT_RESULT;
                    };
                    let Some(dst_end) = dst_start.checked_add(byte_count) else {
                        return INVALID_IMPORT_RESULT;
                    };
                    if src_end > caller.data().config.len() {
                        return INVALID_IMPORT_RESULT;
                    }
                    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
                        return INVALID_IMPORT_RESULT;
                    };
                    let (memory_bytes, host) = memory.data_and_store_mut(&mut caller);
                    if dst_end > memory_bytes.len() {
                        return INVALID_IMPORT_RESULT;
                    }
                    memory_bytes[dst_start..dst_end]
                        .copy_from_slice(&host.config[src_start..src_end]);
                    i32::try_from(byte_count).unwrap_or(INVALID_IMPORT_RESULT)
                },
            )
            .map_err(EffectError::Linker)?;

        // `contains_start_section` rejects application start functions before
        // instantiation. `instantiate_and_start` is used only because Wasmi
        // exposes no deferred-start linker API; without a start section it does
        // not execute guest application code here.
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(EffectError::Wasm)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(EffectError::InvalidBuffer)?;
        if memory.ty(&store).is_64() {
            return Err(EffectError::InvalidBuffer);
        }

        let abi_version = instance
            .get_typed_func::<(), i32>(&store, "led_abi_version")
            .map_err(EffectError::Wasm)?;
        let init = instance
            .get_typed_func::<(), i32>(&store, "led_init")
            .map_err(EffectError::Wasm)?;
        let step = instance
            .get_typed_func::<(i32, i32, i32), i32>(&store, "led_step")
            .map_err(EffectError::Wasm)?;
        let frame_ptr = instance
            .get_typed_func::<(), i32>(&store, "led_frame_ptr")
            .map_err(EffectError::Wasm)?;
        let frame_len = instance
            .get_typed_func::<(), i32>(&store, "led_frame_len")
            .map_err(EffectError::Wasm)?;
        let input_ptr = instance
            .get_typed_func::<(), i32>(&store, "led_input_ptr")
            .map_err(EffectError::Wasm)?;
        let input_capacity = instance
            .get_typed_func::<(), i32>(&store, "led_input_capacity")
            .map_err(EffectError::Wasm)?;
        let apply_input = instance
            .get_typed_func::<(i32, i32), i32>(&store, "led_apply_input")
            .map_err(EffectError::Wasm)?;
        let deinit = if instance.get_export(&store, "led_deinit").is_some() {
            Some(
                instance
                    .get_typed_func::<(), ()>(&store, "led_deinit")
                    .map_err(EffectError::Wasm)?,
            )
        } else {
            None
        };

        let mut effect = Self {
            store,
            memory,
            abi_version,
            init,
            step,
            frame_ptr,
            frame_len,
            input_ptr,
            input_capacity,
            apply_input,
            deinit,
            buffers: BufferDescriptors {
                frame_ptr: 0,
                frame_len: 0,
                input_ptr: 0,
                input_capacity: 0,
            },
            frame: Vec::new(),
            active: true,
        };

        let version = call_guest(
            &mut effect.store,
            &mut effect.active,
            &effect.abi_version,
            (),
        )?;
        if version != ABI_VERSION {
            effect.active = false;
            return Err(EffectError::AbiVersionMismatch(version));
        }

        let init_result = call_guest(&mut effect.store, &mut effect.active, &effect.init, ())?;
        if init_result != 0 {
            effect.active = false;
            return Err(EffectError::InitializationFailed(init_result));
        }

        effect.buffers = effect.read_buffer_descriptors()?;
        effect.validate_buffer_descriptors(pixel_count as u32)?;
        effect
            .frame
            .try_reserve_exact(effect.buffers.frame_len as usize)
            .map_err(|_| EffectError::AllocationFailed)?;
        effect.frame.resize(effect.buffers.frame_len as usize, 0);

        let initial_step = call_guest(
            &mut effect.store,
            &mut effect.active,
            &effect.step,
            (0, 0, 0),
        )?;
        if initial_step < 0 {
            effect.active = false;
            return Err(EffectError::PatternError(initial_step));
        }
        if initial_step > 1 {
            effect.active = false;
            return Err(EffectError::InvalidBuffer);
        }
        effect.ensure_descriptors_stable()?;
        effect.copy_frame()?;
        Ok(effect)
    }

    /// Advances the effect with elapsed time and delta in microseconds.
    ///
    /// Timestamp and delta inputs use their unsigned `u32` bit patterns. If a
    /// guest traps or violates the buffer contract, this instance is marked
    /// inactive and should be dropped.
    pub fn step(&mut self, elapsed_us: u64, dt_us: u32) -> Result<StepResult, EffectError> {
        self.ensure_active()?;
        let time_lo = elapsed_us as u32 as i32;
        let time_hi = (elapsed_us >> 32) as u32 as i32;
        let status = match call_guest(
            &mut self.store,
            &mut self.active,
            &self.step,
            (time_lo, time_hi, dt_us as i32),
        ) {
            Ok(status) => status,
            Err(error) => {
                self.active = false;
                return Err(error);
            }
        };
        if status < 0 {
            self.active = false;
            return Err(EffectError::PatternError(status));
        }
        if status > 1 {
            self.active = false;
            return Err(EffectError::InvalidBuffer);
        }
        if let Err(error) = self.ensure_descriptors_stable() {
            self.active = false;
            return Err(error);
        }
        if status == 0 {
            if let Err(error) = self.copy_frame() {
                self.active = false;
                return Err(error);
            }
            Ok(StepResult::FrameReady)
        } else {
            Ok(StepResult::Unchanged)
        }
    }

    /// Copies a command into the guest-owned input buffer and calls
    /// `led_apply_input`. Calls are serialized with [`Self::step`] by the
    /// mutable receiver.
    pub fn apply_input(&mut self, kind: i32, bytes: &[u8]) -> Result<i32, EffectError> {
        self.ensure_active()?;
        if bytes.len() > self.buffers.input_capacity as usize {
            return Err(EffectError::InvalidBuffer);
        }
        let start = self.buffers.input_ptr as usize;
        self.memory
            .write(&mut self.store, start, bytes)
            .map_err(|error| {
                self.active = false;
                EffectError::Wasm(error.into())
            })?;

        let length = i32::try_from(bytes.len()).map_err(|_| EffectError::InvalidBuffer)?;
        let result = match call_guest(
            &mut self.store,
            &mut self.active,
            &self.apply_input,
            (kind, length),
        ) {
            Ok(result) => result,
            Err(error) => {
                self.active = false;
                return Err(error);
            }
        };
        if let Err(error) = self.ensure_descriptors_stable() {
            self.active = false;
            return Err(error);
        }
        Ok(result)
    }

    /// Returns the most recently copied host-owned packed RGB8 framebuffer.
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    /// Returns the logical number of RGB pixels in this effect's framebuffer.
    pub fn pixel_count(&self) -> u32 {
        self.buffers.frame_len / 3
    }

    /// Calls the optional `led_deinit` export. Dropping the instance remains
    /// safe and does not require this method to be called.
    pub fn deinit(&mut self) -> Result<(), EffectError> {
        self.ensure_active()?;
        if let Some(deinit) = self.deinit {
            call_guest(&mut self.store, &mut self.active, &deinit, ())?;
        }
        self.active = false;
        Ok(())
    }

    fn read_buffer_descriptors(&mut self) -> Result<BufferDescriptors, EffectError> {
        let frame_ptr = call_guest(&mut self.store, &mut self.active, &self.frame_ptr, ())? as u32;
        let frame_len = call_guest(&mut self.store, &mut self.active, &self.frame_len, ())? as u32;
        let input_ptr = call_guest(&mut self.store, &mut self.active, &self.input_ptr, ())? as u32;
        let input_capacity =
            call_guest(&mut self.store, &mut self.active, &self.input_capacity, ())? as u32;
        Ok(BufferDescriptors {
            frame_ptr,
            frame_len,
            input_ptr,
            input_capacity,
        })
    }

    fn validate_buffer_descriptors(&mut self, pixel_count: u32) -> Result<(), EffectError> {
        let expected_len = pixel_count
            .checked_mul(3)
            .ok_or(EffectError::InvalidBuffer)?;
        if self.buffers.frame_len != expected_len {
            return Err(EffectError::InvalidFrameLength {
                expected: expected_len,
                actual: self.buffers.frame_len,
            });
        }

        let memory_size = self.memory.data_size(&self.store);
        let frame_range =
            checked_range(self.buffers.frame_ptr, self.buffers.frame_len, memory_size)
                .ok_or(EffectError::InvalidBuffer)?;
        let input_range = checked_range(
            self.buffers.input_ptr,
            self.buffers.input_capacity,
            memory_size,
        )
        .ok_or(EffectError::InvalidBuffer)?;
        if ranges_overlap(&frame_range, &input_range) {
            return Err(EffectError::OverlappingBuffers);
        }
        Ok(())
    }

    fn ensure_descriptors_stable(&mut self) -> Result<(), EffectError> {
        let current = self.read_buffer_descriptors()?;
        if current != self.buffers {
            return Err(EffectError::UnstableBufferDescriptor);
        }
        self.validate_buffer_descriptors(self.buffers.frame_len / 3)
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

    fn ensure_active(&self) -> Result<(), EffectError> {
        if self.active {
            Ok(())
        } else {
            Err(EffectError::Inactive)
        }
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

fn checked_range(start: u32, len: u32, memory_size: usize) -> Option<core::ops::Range<usize>> {
    let start = start as usize;
    let end = start.checked_add(len as usize)?;
    (end <= memory_size).then_some(start..end)
}

fn ranges_overlap(first: &core::ops::Range<usize>, second: &core::ops::Range<usize>) -> bool {
    first.start < second.end && second.start < first.end
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
