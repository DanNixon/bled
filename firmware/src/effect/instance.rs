use alloc::vec::Vec;
use bled_core::{
    config::{DeviceConfig, MAX_CONFIG_CBOR_SIZE},
    effect::abi::{
        self, EFFECT_MANIFEST_SECTION_NAME,
        export::{InitResult, StepResult},
    },
};
use defmt::info;
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

#[derive(Debug)]
struct HostState {
    config: Vec<u8>,
    limits: StoreLimits,
}

/// A loaded WASM effect.
pub struct EffectInstance {
    store: Store<HostState>,
    module: Module,
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

    active: bool,
}

impl EffectInstance {
    /// Compiles and initializes an effect from a WebAssembly binary.
    pub fn load(wasm_bytes: &[u8], device_config: &DeviceConfig) -> Result<Self, EffectError> {
        info!("creating device config for effect");
        let mut fixture_config_bytes = Vec::new();
        fixture_config_bytes
            .try_reserve_exact(MAX_CONFIG_CBOR_SIZE)
            .map_err(|_| EffectError::AllocationFailed)?;
        fixture_config_bytes.resize(MAX_CONFIG_CBOR_SIZE, 0);
        let length =
            bled_core::io::encode_cbor(device_config.fixtures(), &mut fixture_config_bytes)
                .map_err(|_| EffectError::ConfigurationTooLarge)?;
        fixture_config_bytes.truncate(length);

        info!("loading effect");
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
        let mut sections = module
            .custom_sections()
            .filter(|section| section.name() == EFFECT_MANIFEST_SECTION_NAME);
        sections.next().ok_or(EffectError::MissingManifestSection)?;
        if sections.next().is_some() {
            return Err(EffectError::DuplicateManifestSection);
        }

        let mut store = Store::new(
            &engine,
            HostState {
                config: fixture_config_bytes,
                limits: StoreLimitsBuilder::new()
                    .memory_size(64 * 1024)
                    .table_elements(1024)
                    .instances(1)
                    .memories(1)
                    .tables(1)
                    .build(),
            },
        );
        store.limiter(|state| &mut state.limits);

        info!("linking effect");
        let mut linker = Linker::<HostState>::new(&engine);
        linker
            .func_wrap(
                abi::import::MODULE,
                abi::import::FIXTURE_CONFIG_SIZE,
                |caller: Caller<'_, HostState>| {
                    i32::try_from(caller.data().config.len()).unwrap_or(INVALID_IMPORT_RESULT)
                },
            )
            .map_err(EffectError::Linker)?;
        linker
            .func_wrap(
                abi::import::MODULE,
                abi::import::FIXTURE_CONFIG_READ,
                |mut caller: Caller<'_, HostState>, dst_ptr: i32| {
                    let dst_start = dst_ptr as u32 as usize;
                    let byte_count = caller.data().config.len();

                    let Some(dst_end) = dst_start.checked_add(byte_count) else {
                        return INVALID_IMPORT_RESULT;
                    };
                    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
                        return INVALID_IMPORT_RESULT;
                    };

                    let (memory_bytes, host) = memory.data_and_store_mut(&mut caller);
                    if dst_end > memory_bytes.len() {
                        return INVALID_IMPORT_RESULT;
                    }

                    memory_bytes[dst_start..dst_end].copy_from_slice(&host.config);

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
            module,
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
            active: true,
        };

        // Check ABI version
        info!("checking ABI version");
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

        // Initialise the effect
        info!("initializing effect");
        let init_result = call_guest(&mut effect.store, &mut effect.active, &effect.init, ())?;
        let init_result = InitResult::try_from(init_result)
            .map_err(|_| EffectError::InvalidInitResult(init_result))?;
        if init_result != InitResult::Success {
            effect.active = false;
            return Err(EffectError::InitializationFailed(init_result));
        }

        info!("effect ready");
        Ok(effect)
    }

    /// Returns the raw JSON bytes from the `bled.manifest` custom section.
    pub fn manifest_bytes(&self) -> &[u8] {
        self.module
            .custom_sections()
            .find(|section| section.name() == EFFECT_MANIFEST_SECTION_NAME)
            .expect("manifest section was validated during load")
            .data()
    }

    /// Queries the size of the guest framebuffer in bytes.
    pub fn frame_size(&mut self) -> Result<usize, EffectError> {
        call_guest::<_, i32>(&mut self.store, &mut self.active, &self.frame_len, ())?
            .try_into()
            .map_err(|_| EffectError::TypeConversionFailed)
    }

    pub fn read_frame_data(&mut self, buf: &mut [u8]) -> Result<usize, EffectError> {
        read_guest_buffer(
            &mut self.store,
            &mut self.active,
            self.memory,
            &self.frame_ptr,
            &self.frame_len,
            buf,
        )
    }

    /// Queries the size of the guest command buffer in bytes.
    pub fn command_size(&mut self) -> Result<usize, EffectError> {
        call_guest::<_, i32>(&mut self.store, &mut self.active, &self.command_len, ())?
            .try_into()
            .map_err(|_| EffectError::TypeConversionFailed)
    }

    pub fn read_command_data(&mut self, buf: &mut [u8]) -> Result<usize, EffectError> {
        read_guest_buffer(
            &mut self.store,
            &mut self.active,
            self.memory,
            &self.command_ptr,
            &self.command_len,
            buf,
        )
    }

    pub fn write_command_data(&mut self, buf: &[u8]) -> Result<(), EffectError> {
        write_guest_buffer(
            &mut self.store,
            &mut self.active,
            self.memory,
            &self.command_ptr,
            &self.command_len,
            buf,
        )
    }

    /// Queries the size of the guest parameter buffer in bytes.
    pub fn parameter_size(&mut self) -> Result<usize, EffectError> {
        call_guest::<_, i32>(&mut self.store, &mut self.active, &self.parameter_len, ())?
            .try_into()
            .map_err(|_| EffectError::TypeConversionFailed)
    }

    pub fn read_parameter_data(&mut self, buf: &mut [u8]) -> Result<usize, EffectError> {
        read_guest_buffer(
            &mut self.store,
            &mut self.active,
            self.memory,
            &self.parameter_ptr,
            &self.parameter_len,
            buf,
        )
    }

    pub fn write_parameter_data(&mut self, buf: &[u8]) -> Result<(), EffectError> {
        write_guest_buffer(
            &mut self.store,
            &mut self.active,
            self.memory,
            &self.parameter_ptr,
            &self.parameter_len,
            buf,
        )
    }

    /// Returns the guest's requested interval until the next step, in milliseconds.
    pub fn step_time(&mut self) -> Result<i32, EffectError> {
        let time_ms = call_guest(&mut self.store, &mut self.active, &self.step_time, ())?;
        if time_ms <= 0 {
            self.active = false;
            return Err(EffectError::InvalidStepTime(time_ms));
        }
        Ok(time_ms)
    }

    /// Advances the effect with elapsed time in milliseconds.
    ///
    /// If the guest traps or violates the buffer contract, this instance is
    /// marked inactive and should be dropped.
    pub fn step(&mut self, time_ms: i32) -> Result<StepResult, EffectError> {
        let status = match call_guest(&mut self.store, &mut self.active, &self.step, (time_ms,)) {
            Ok(status) => status,
            Err(error) => {
                self.active = false;
                return Err(error);
            }
        };
        let result =
            StepResult::try_from(status).map_err(|_| EffectError::InvalidStepResult(status))?;
        Ok(result)
    }
}

const INVALID_IMPORT_RESULT: i32 = -1;

fn read_guest_buffer(
    store: &mut Store<HostState>,
    active: &mut bool,
    memory: Memory,
    ptr_func: &TypedFunc<(), i32>,
    len_func: &TypedFunc<(), i32>,
    buf: &mut [u8],
) -> Result<usize, EffectError> {
    let ptr = call_guest(store, active, ptr_func, ())?;
    let len: usize = call_guest(store, active, len_func, ())?
        .try_into()
        .map_err(|_| EffectError::TypeConversionFailed)?;
    if buf.len() < len {
        return Err(EffectError::InvalidBuffer);
    }
    let start = ptr as u32 as usize;
    let Some(end) = start.checked_add(len) else {
        *active = false;
        return Err(EffectError::InvalidBuffer);
    };
    let Some(source) = memory.data(store).get(start..end) else {
        *active = false;
        return Err(EffectError::InvalidBuffer);
    };
    buf[..len].copy_from_slice(source);
    Ok(len)
}

fn write_guest_buffer(
    store: &mut Store<HostState>,
    active: &mut bool,
    memory: Memory,
    ptr_func: &TypedFunc<(), i32>,
    len_func: &TypedFunc<(), i32>,
    buf: &[u8],
) -> Result<(), EffectError> {
    let ptr = call_guest(store, active, ptr_func, ())?;
    let len: usize = call_guest(store, active, len_func, ())?
        .try_into()
        .map_err(|_| EffectError::TypeConversionFailed)?;
    if buf.len() > len {
        return Err(EffectError::InvalidBuffer);
    }
    let start = ptr as u32 as usize;
    let Some(end) = start.checked_add(len) else {
        *active = false;
        return Err(EffectError::InvalidBuffer);
    };
    let Some(destination) = memory.data_mut(store).get_mut(start..end) else {
        *active = false;
        return Err(EffectError::InvalidBuffer);
    };
    destination[..buf.len()].copy_from_slice(buf);
    Ok(())
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
    if let Err(error) = store.set_fuel(1_000_000) {
        *active = false;
        return Err(EffectError::Wasm(error));
    }
    function.call(store, params).map_err(|error| {
        *active = false;
        EffectError::Wasm(error)
    })
}

/// Errors returned while validating, running, or exchanging data with an effect.
#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    /// Wasmi rejected the module or a guest function trapped.
    #[error("WebAssembly error: {0}")]
    Wasm(#[source] wasmi::Error),
    /// Defining an ABI host import failed.
    #[error("failed to define an ABI host import: {0}")]
    Linker(#[source] wasmi::errors::LinkerError),
    /// The guest reported an ABI version other than the one supported.
    #[error("unsupported effect ABI version: {0}")]
    AbiVersionMismatch(i32),
    /// `bled_init` returned a value outside the defined result set.
    #[error("invalid bled_init result: {0}")]
    InvalidInitResult(i32),
    /// `bled_init` returned a non-success result.
    #[error("effect initialization failed: {0:?}")]
    InitializationFailed(InitResult),
    /// `bled_step` returned a value outside the defined result set.
    #[error("invalid bled_step result: {0}")]
    InvalidStepResult(i32),
    /// `bled_step_time` returned a non-positive interval.
    #[error("invalid bled_step_time interval: {0}")]
    InvalidStepTime(i32),
    /// A guest returned an invalid pointer, length, or descriptor.
    #[error("effect returned an invalid buffer")]
    InvalidBuffer,
    /// The CBOR configuration or logical pixel count cannot be represented by this ABI.
    #[error("effect configuration is too large for the ABI")]
    ConfigurationTooLarge,
    /// The module has no manifest custom section.
    #[error("effect has no manifest custom section")]
    MissingManifestSection,
    /// The module contains multiple manifest custom sections.
    #[error("effect has multiple manifest custom sections")]
    DuplicateManifestSection,
    /// A requested host-side allocation failed.
    #[error("host-side allocation failed")]
    AllocationFailed,
    /// A type conversion failed.
    #[error("effect value could not be represented by the ABI")]
    TypeConversionFailed,
    /// The effect is marked as inactive.
    #[error("effect is inactive")]
    Inactive,
}
