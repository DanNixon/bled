#[doc(hidden)]
#[macro_export]
macro_rules! __bled_effect_entrypoint {
    ($effect_type:ty, $max_pixel_count:expr, $command_bytes:expr, $parameter_bytes:expr) => {
        static __BLED_EFFECT_STATE: $crate::guest::GuestState<
            $crate::guest::Runtime<
                $effect_type,
                { $max_pixel_count * 3 },
                { $command_bytes },
                { $parameter_bytes },
            >,
        > = $crate::guest::GuestState::new($crate::guest::Runtime::new());

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_abi_version() -> i32 {
            $crate::bled_core::effect::abi::VERSION
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_init() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.init()).into()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_step_time() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.step_time())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_step(time_ms: i32) -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.step(time_ms))
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_frame_ptr() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.frame_ptr())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_frame_len() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.frame_len())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_command_ptr() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.command_ptr())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_command_len() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.command_len())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_parameter_ptr() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.parameter_ptr())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bled_parameter_len() -> i32 {
            __BLED_EFFECT_STATE.with(|state| state.parameter_len())
        }
    };
}

/// Declare a BLED WASM effect.
///
/// The manifest is embedded in the `bled.manifest` custom section. The effect
/// type must implement [`Effect`](crate::guest::Effect).
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

        $crate::__bled_effect_entrypoint!(
            $effect_type,
            $max_pixel_count,
            $command_bytes,
            $parameter_bytes
        );
    };
}
