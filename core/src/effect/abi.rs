//! WASM ABI for a BLED effect.

/// ABI version.
pub const VERSION: i32 = 1;

/// Imports from the host to the guest.
pub mod import {
    pub const MODULE: &str = "bled_v1";

    /// Function returning the byte length of the CBOR encoded device layout.
    /// `bled_layout_size() -> i32`
    pub const LAYOUT_SIZE: &str = "bled_layout_size";

    /// Function copying a range of the CBOR encoded layout into guest memory.
    /// Returns the number of bytes copied.
    /// `bled_layout_read(offset: i32, dest_ptr: i32, length: i32) -> i32`
    pub const LAYOUT_READ: &str = "bled_layout_read";
}

/// Exports from the guest to the host.
pub mod export {
    use num_enum::IntoPrimitive;

    /// Function returning the ABI version.
    /// `bled_abi_version() -> i32`
    pub const ABI_VERSION: &str = "bled_abi_version";

    /// Function initializing the effect.
    /// Returns `InitResult`.
    /// `bled_init() -> i32`
    pub const INIT: &str = "bled_init";

    /// The result of a call to `bled_init`.
    #[derive(Debug, Clone, PartialEq, IntoPrimitive)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    #[repr(i32)]
    pub enum InitResult {
        Success = 0,
        OtherError = -1,
        LayoutParseError = -2,
        FrameBufferError = -3,
    }

    /// Function defining the desired time between frames.
    /// The host will attempt, but not guarentee, to call `bled_step` at this interval.
    /// This interval may change between steps.
    /// Returns the step time in milliseconds.
    /// `bled_step_time() -> i32`
    pub const STEP_TIME: &str = "bled_step_time";

    /// Function advancing the effect by one frame.
    /// Returns `StepResult`, indicating if the frame buffer has changed.
    /// `bled_step(time_ms: i32) -> i32`
    pub const STEP: &str = "bled_step";

    /// The result of a call to `bled_step`.
    #[derive(Debug, Clone, PartialEq, IntoPrimitive)]
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
    /// `bled_frame_len() -> i32`
    pub const FRAME_LEN: &str = "bled_frame_len";

    /// Function returning the guest's command buffer pointer.
    /// `bled_command_ptr() -> i32`
    pub const COMMAND_PTR: &str = "bled_command_ptr";

    /// Capacity of the command buffer.
    /// The host must be able to copy this much regardless of how much the guest actually uses.
    pub const COMMAND_CAPACITY: usize = 8;

    /// Function returning the guest's parameter buffer pointer.
    /// `bled_parameter_ptr() -> i32`
    pub const PARAMETER_PTR: &str = "bled_parameter_ptr";

    /// Length capacity of the parameter memory.
    /// The host must be able to copy this much regardless of how much the guest actually uses.
    /// (convineintly the maximum size of a GATT attribute value)
    pub const PARAMETER_CAPACITY: usize = 512;
}
