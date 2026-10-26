//! WASM ABI for a BLED effect.

/// ABI version.
pub const VERSION: i32 = 2;

/// Imports from the host to the guest.
pub mod import {
    pub const MODULE: &str = "bled_v2";

    /// Function returning the byte length of the CBOR encoded fixture config.
    /// `bled_fixture_config_size() -> i32`
    pub const FIXTURE_CONFIG_SIZE: &str = "bled_fixture_config_size";

    /// Function copying the CBOR encoded fixture config into guest memory.
    /// The guest should ensure that the destination buffer is large enough by calling `bled_fixture_config_size` first.
    /// Returns the number of bytes copied.
    /// `bled_fixture_config_read(dest_ptr: i32) -> i32`
    pub const FIXTURE_CONFIG_READ: &str = "bled_fixture_config_read";
}

/// Exports from the guest to the host.
pub mod export {
    use num_enum::{IntoPrimitive, TryFromPrimitive};

    /// Function returning the ABI version.
    /// `bled_abi_version() -> i32`
    pub const ABI_VERSION: &str = "bled_abi_version";

    /// Function initializing the effect.
    /// Returns `InitResult`.
    /// `bled_init() -> i32`
    pub const INIT: &str = "bled_init";

    /// The result of a call to `bled_init`.
    #[derive(Debug, Clone, PartialEq, IntoPrimitive, TryFromPrimitive)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[repr(i32)]
    pub enum InitResult {
        Success = 0,
        OtherError = -1,
        ConfigParseError = -2,
        FrameBufferError = -3,
    }

    /// Function defining the desired time between frames.
    /// The host will attempt, but not guarantee, to call `bled_step` at this interval.
    /// This interval may change between steps.
    /// Returns the step time in milliseconds.
    /// `bled_step_time() -> i32`
    pub const STEP_TIME: &str = "bled_step_time";

    /// Function advancing the effect by one frame.
    /// Returns `StepResult`, indicating if the frame buffer has changed.
    /// `bled_step(time_ms: i32) -> i32`
    pub const STEP: &str = "bled_step";

    /// The result of a call to `bled_step`.
    #[derive(Debug, Clone, PartialEq, IntoPrimitive, TryFromPrimitive)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[repr(i32)]
    pub enum StepResult {
        Error = -1,
        Unchanged = 0,
        Changed = 1,
    }

    /// Function returning the pointer to the guest framebuffer.
    /// `bled_frame_ptr() -> i32`
    pub const FRAME_PTR: &str = "bled_frame_ptr";

    /// Function returning the length of the guest framebuffer in bytes.
    /// Once the effect is initialized, this value must remain constant.
    /// `bled_frame_len() -> i32`
    pub const FRAME_LEN: &str = "bled_frame_len";

    /// Function returning the guest's command buffer pointer.
    /// `bled_command_ptr() -> i32`
    pub const COMMAND_PTR: &str = "bled_command_ptr";

    /// Function returning the guest's command buffer size.
    /// Once the effect is initialized, this value must remain constant.
    /// Must be less than 512 bytes.
    /// `bled_command_len() -> i32`
    pub const COMMAND_LEN: &str = "bled_command_len";

    /// Function returning the guest's parameter buffer pointer.
    /// `bled_parameter_ptr() -> i32`
    pub const PARAMETER_PTR: &str = "bled_parameter_ptr";

    /// Function returning the guest's parameter buffer size.
    /// Once the effect is initialized, this value must remain constant.
    /// Must be less than 512 bytes.
    /// `bled_parameter_len() -> i32`
    pub const PARAMETER_LEN: &str = "bled_parameter_len";
}

pub const EFFECT_MANIFEST_SECTION_NAME: &str = "bled.manifest";

#[macro_export]
macro_rules! wasm_effect {
    ($manifest_json:literal, $effect_type:ty, $max_pixel_count:expr, $command_bytes:expr, $parameter_bytes:expr) => {
        #[used]
        #[unsafe(link_section = "bled.manifest")]
        static EFFECT_MANIFEST: [u8; $manifest_json.len()] = *$manifest_json;

        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo) -> ! {
            core::arch::wasm32::unreachable();
        }

        #[link(wasm_import_module = "bled_v2")]
        unsafe extern "C" {
            fn bled_fixture_config_size() -> i32;
            fn bled_fixture_config_read(dst_ptr: i32) -> i32;
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_abi_version() -> i32 {
            $crate::effect::abi::VERSION
        }

        static mut EFFECT: Option<$effect_type> = None;

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_init() -> i32 {
            // Get fixture config size
            let size = unsafe { bled_fixture_config_size() };
            if size <= 0 || size as usize > $crate::config::MAX_CONFIG_CBOR_SIZE {
                return $crate::effect::abi::export::InitResult::ConfigParseError.into();
            }

            // Read fixture config CBOR data
            let config_ptr = core::ptr::addr_of_mut!(FIXTURE_CONFIG_CBOR).cast::<u8>();
            let bytes_read = unsafe { bled_fixture_config_read(config_ptr as usize as i32) };
            if bytes_read != size {
                return $crate::effect::abi::export::InitResult::ConfigParseError.into();
            }

            // Parse fixture config
            let config_bytes = unsafe { &FIXTURE_CONFIG_CBOR[..size as usize] };
            let config = match $crate::io::decode_cbor::<$crate::config::Fixtures>(config_bytes) {
                Ok(config) => config,
                Err(_) => return $crate::effect::abi::export::InitResult::ConfigParseError.into(),
            };

            // Ensure frame buffer is large enough
            let pixel_count = config.total_length::<4>();
            if pixel_count > 4096 {
                return $crate::effect::abi::export::InitResult::FrameBufferError.into();
            }
            unsafe { FRAME_LEN = pixel_count * 3 }

            // Initialise the effect
            let effect = match <$effect_type>::new(&Context {}, config) {
                Ok(effect) => effect,
                Err(_) => return $crate::effect::abi::export::InitResult::OtherError.into(),
            };
            unsafe { EFFECT = Some(effect) }

            // Clear frame buffer
            unsafe { FRAME.fill(0) }

            $crate::effect::abi::export::InitResult::Success.into()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_step_time() -> i32 {
            let effect = unsafe { EFFECT.as_ref() };
            match effect {
                Some(effect) => effect.step_time(&Context {}).try_into().unwrap(),
                None => 250,
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_step(time_ms: i32) -> i32 {
            let effect = unsafe { EFFECT.as_mut() };
            match effect {
                Some(effect) => effect.step(&Context {}, time_ms.try_into().unwrap()),
                None => $crate::effect::abi::export::StepResult::Error,
            }
            .into()
        }

        const FRAME_BYTES: usize = $max_pixel_count * 3;
        static mut FRAME: [u8; FRAME_BYTES] = [0; FRAME_BYTES];
        static mut FRAME_LEN: usize = 0;

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_frame_ptr() -> i32 {
            core::ptr::addr_of_mut!(FRAME) as usize as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_frame_len() -> i32 {
            (unsafe { FRAME_LEN }).try_into().unwrap()
        }

        static mut FIXTURE_CONFIG_CBOR: [u8; $crate::config::MAX_CONFIG_CBOR_SIZE] =
            [0; $crate::config::MAX_CONFIG_CBOR_SIZE];

        const COMMAND_BYTES: usize = $command_bytes;
        static mut COMMAND: [u8; COMMAND_BYTES] = [0; COMMAND_BYTES];

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_command_ptr() -> i32 {
            core::ptr::addr_of_mut!(COMMAND) as usize as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_command_len() -> i32 {
            COMMAND_BYTES as i32
        }

        const PARAMETER_BYTES: usize = $parameter_bytes;
        static mut PARAMETERS: [u8; PARAMETER_BYTES] = [0; PARAMETER_BYTES];

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_parameter_ptr() -> i32 {
            core::ptr::addr_of_mut!(PARAMETERS) as usize as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_parameter_len() -> i32 {
            PARAMETER_BYTES as i32
        }

        pub struct Context;

        impl Context {
            pub fn frame(&self) -> &mut [u8] {
                unsafe { &mut FRAME[..FRAME_LEN] }
            }

            pub fn command(&self) -> &mut [u8] {
                unsafe { &mut COMMAND }
            }

            pub fn parameters(&self) -> &mut [u8] {
                unsafe { &mut PARAMETERS }
            }
        }

        pub trait WasmEffect: Sized {
            fn new(ctx: &Context, fixtures: $crate::config::Fixtures) -> Result<Self, ()>;

            fn step_time(&self, ctx: &Context) -> u32;

            fn step(
                &mut self,
                ctx: &Context,
                timestamp_ms: u32,
            ) -> $crate::effect::abi::export::StepResult;
        }
    };
}
