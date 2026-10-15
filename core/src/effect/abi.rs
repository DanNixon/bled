/// Current WebAssembly effects ABI version.
pub const VERSION: i32 = 1;

pub mod import {
    /// Name of the Wasm module that provides host imports.
    pub const MODULE: &str = "bled_v1";

    /// Host import returning the byte length of the CBOR-encoded device layout.
    pub const LAYOUT_SIZE: &str = "layout_size";

    /// Host import copying a range of the CBOR-encoded layout into guest memory.
    pub const LAYOUT_READ: &str = "layout_read";
}

pub mod export {
    /// Guest export returning the ABI version.
    pub const ABI_VERSION: &str = "bled_abi_version";

    /// Guest export initializing the effect; zero indicates success.
    pub const INIT: &str = "bled_init";

    /// Guest export advancing the effect by one frame.
    pub const STEP: &str = "bled_step";
    /// `led_step` result indicating that the framebuffer has not changed.
    pub const STEP_UNCHANGED: i32 = -1;

    /// Guest export returning the framebuffer pointer.
    pub const FRAME_PTR: &str = "bled_frame_ptr";

    /// Guest export returning the framebuffer byte length.
    pub const FRAME_LEN: &str = "bled_frame_len";

    /// Guest export returning the input buffer pointer.
    pub const INPUT_PTR: &str = "bled_input_ptr";

    /// Guest export returning the input buffer capacity in bytes.
    pub const INPUT_CAPACITY: &str = "bled_input_capacity";

    /// Guest export applying an input message previously copied into its input buffer.
    pub const APPLY_INPUT: &str = "bled_apply_input";
}
