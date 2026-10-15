/// Maximum linear memory available to an effect instance.
const MAX_MEMORY_BYTES: usize = 64 * 1024;

/// Instruction budget for each guest function call.
const FUEL_PER_CALL: u64 = 1_000_000;

/// A loaded WASM effect.
pub struct EffectInstance {
    store: Store<HostState>,
    memory: Memory,

    abi_version: TypedFunc<(), i32>,

    init: TypedFunc<(), i32>,

    step_time: TypedFunc<(), i32>,
    step: TypedFunc<(i32,), i32>,

    frame_ptr: TypedFunc<(), i32>,
    frame_len: TypedFunc<(), i32>,

    command_ptr: TypedFunc<(), i32>,
    command_len: TypedFunc<(), i32>,

    parameter_ptr: TypedFunc<(), i32>,
    parameter_len: TypedFunc<(), i32>,

    frame: Vec<u8>,
    active: bool,
}

impl EffectInstance {
    /// Compiles and initializes an effect from a WebAssembly binary.
    pub fn load(wasm_bytes: &[u8], device_config: &DeviceConfig) -> Result<Self, EffectError> {
        let (config_bytes, pixel_count) = encode_device_config(device_config)?;

        let mut config = Config::default();
        config
            .consume_fuel(true)
            .wasm_sign_extension(true)
            .wasm_saturating_float_to_int(false)
            .wasm_multi_value(false)
            .wasm_multi_memory(false)
            .wasm_bulk_memory(true)
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
            .func_wrap(
                abi::import::MODULE,
                abi::import::LAYOUT_SIZE,
                |caller: Caller<'_, HostState>| {
                    i32::try_from(caller.data().config.len()).unwrap_or(INVALID_IMPORT_RESULT)
                },
            )
            .map_err(EffectError::Linker)?;
        linker
            .func_wrap(
                abi::import::MODULE,
                abi::import::LAYOUT_READ,
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
            .get_typed_func::<(), i32>(&store, abi::export::ABI_VERSION)
            .map_err(EffectError::Wasm)?;
        let init = instance
            .get_typed_func::<(), i32>(&store, abi::export::INIT)
            .map_err(EffectError::Wasm)?;
        let step_time = instance
            .get_typed_func::<(), i32>(&store, abi::export::STEP_TIME)
            .map_err(EffectError::Wasm)?;
        let step = instance
            .get_typed_func::<(i32,), i32>(&store, abi::export::STEP)
            .map_err(EffectError::Wasm)?;
        let frame_ptr = instance
            .get_typed_func::<(), i32>(&store, abi::export::FRAME_PTR)
            .map_err(EffectError::Wasm)?;
        let frame_len = instance
            .get_typed_func::<(), i32>(&store, abi::export::FRAME_LEN)
            .map_err(EffectError::Wasm)?;
        let command_ptr = instance
            .get_typed_func::<(), i32>(&store, abi::export::COMMAND_PTR)
            .map_err(EffectError::Wasm)?;
        let command_len = instance
            .get_typed_func::<(), i32>(&store, abi::export::COMMAND_LEN)
            .map_err(EffectError::Wasm)?;
        let parameter_ptr = instance
            .get_typed_func::<(), i32>(&store, abi::export::PARAMETER_PTR)
            .map_err(EffectError::Wasm)?;
        let parameter_len = instance
            .get_typed_func::<(), i32>(&store, abi::export::PARAMETER_LEN)
            .map_err(EffectError::Wasm)?;

        let mut effect = Self {
            store,
            memory,
            abi_version,
            init,
            step_time,
            step,
            frame_ptr,
            frame_len,
            command_ptr,
            command_len,
            parameter_ptr,
            parameter_len,
            frame: Vec::new(),
            active: true,
        };

        let version = call_guest(
            &mut effect.store,
            &mut effect.active,
            &effect.abi_version,
            (),
        )?;
        if version != abi::VERSION {
            effect.active = false;
            return Err(EffectError::AbiVersionMismatch(version));
        }

        let init_result = call_guest(&mut effect.store, &mut effect.active, &effect.init, ())?;
        if init_result != 0 {
            effect.active = false;
            return Err(EffectError::InitializationFailed(init_result));
        }

        // TODO
        // effect
        //     .frame
        //     .try_reserve_exact(effect.buffers.frame_len as usize)
        //     .map_err(|_| EffectError::AllocationFailed)?;
        // effect.frame.resize(effect.buffers.frame_len as usize, 0);

        Ok(effect)
    }
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
    /// The guest reported an ABI version other than the one supported.
    AbiVersionMismatch(i32),
    /// `bled_init` returned a non-success result.
    InitializationFailed(i32),
    /// `bled_step` returned its error result.
    StepFailed,
    /// `bled_step` returned a value outside the defined result set.
    InvalidStepResult(i32),
    /// `bled_step_time` returned a non-positive interval.
    InvalidStepTime(i32),
    /// A guest returned an invalid pointer, length, or descriptor.
    InvalidBuffer,
    /// The guest framebuffer length was not exactly three bytes per pixel.
    InvalidFrameLength { expected: u32, actual: u32 },
    /// The framebuffer and parameter buffer overlap.
    OverlappingBuffers,
    /// The guest changed a buffer descriptor after initialization.
    UnstableBufferDescriptor,
    /// The CBOR configuration or logical pixel count cannot be represented by this ABI.
    ConfigurationTooLarge,
    /// A requested host-side allocation failed.
    AllocationFailed,
}
