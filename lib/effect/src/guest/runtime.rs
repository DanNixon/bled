use super::{Context, Effect};
use bled_core::config::Fixtures;
#[cfg(target_arch = "wasm32")]
use bled_core::config::MAX_CONFIG_CBOR_SIZE;
use bled_core::effect::abi::export::{InitResult, StepResult};

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "bled_v2")]
unsafe extern "C" {
    fn bled_fixture_config_size() -> i32;
    fn bled_fixture_config_read(destination: i32) -> i32;
}

/// Owns an effect and all of the buffers used by the WASM ABI.
pub struct Runtime<
    E,
    const FRAME_BYTES: usize,
    const COMMAND_BYTES: usize,
    const PARAMETER_BYTES: usize,
> {
    effect: Option<E>,
    frame: [u8; FRAME_BYTES],
    frame_len: usize,
    command: [u8; COMMAND_BYTES],
    parameters: [u8; PARAMETER_BYTES],
    #[cfg(target_arch = "wasm32")]
    config: [u8; MAX_CONFIG_CBOR_SIZE],
}

impl<E, const FRAME_BYTES: usize, const COMMAND_BYTES: usize, const PARAMETER_BYTES: usize>
    Runtime<E, FRAME_BYTES, COMMAND_BYTES, PARAMETER_BYTES>
{
    /// Create an uninitialized runtime.
    pub const fn new() -> Self {
        Self {
            effect: None,
            frame: [0; FRAME_BYTES],
            frame_len: 0,
            command: [0; COMMAND_BYTES],
            parameters: [0; PARAMETER_BYTES],
            #[cfg(target_arch = "wasm32")]
            config: [0; MAX_CONFIG_CBOR_SIZE],
        }
    }
}

impl<E: Effect, const FRAME_BYTES: usize, const COMMAND_BYTES: usize, const PARAMETER_BYTES: usize>
    Default for Runtime<E, FRAME_BYTES, COMMAND_BYTES, PARAMETER_BYTES>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Effect, const FRAME_BYTES: usize, const COMMAND_BYTES: usize, const PARAMETER_BYTES: usize>
    Runtime<E, FRAME_BYTES, COMMAND_BYTES, PARAMETER_BYTES>
{
    /// Read the fixture config from the host and initialize the effect.
    #[cfg(target_arch = "wasm32")]
    pub fn init(&mut self) -> InitResult {
        let size = unsafe { bled_fixture_config_size() };
        if size <= 0 || size as usize > MAX_CONFIG_CBOR_SIZE {
            return InitResult::ConfigParseError;
        }
        let size = size as usize;

        let bytes_read =
            unsafe { bled_fixture_config_read(self.config.as_mut_ptr() as usize as i32) };
        if bytes_read != size as i32 {
            return InitResult::ConfigParseError;
        }

        let fixtures = match bled_core::io::decode_cbor::<Fixtures>(&self.config[..size]) {
            Ok(fixtures) => fixtures,
            Err(_) => return InitResult::ConfigParseError,
        };

        self.init_from_fixtures(fixtures)
    }

    /// Initialize the effect using an already decoded fixture configuration.
    pub fn init_from_fixtures(&mut self, fixtures: Fixtures) -> InitResult {
        let pixel_count = fixtures.total_length::<4>();
        let Some(frame_len) = pixel_count.checked_mul(3) else {
            return InitResult::FrameBufferError;
        };
        if frame_len > FRAME_BYTES || frame_len > i32::MAX as usize {
            return InitResult::FrameBufferError;
        }

        self.frame_len = frame_len;
        self.frame.fill(0);

        let result = {
            let mut ctx = Context::new(
                &mut self.frame[..self.frame_len],
                &mut self.command,
                &mut self.parameters,
            );
            E::new(&mut ctx, fixtures)
        };

        match result {
            Ok(effect) => {
                self.effect = Some(effect);
                InitResult::Success
            }
            Err(_) => InitResult::OtherError,
        }
    }

    /// Query the effect's desired step interval in milliseconds.
    pub fn step_time(&mut self) -> i32 {
        let Some(effect) = self.effect.as_ref() else {
            return 250;
        };

        let ctx = Context::new(
            &mut self.frame[..self.frame_len],
            &mut self.command,
            &mut self.parameters,
        );
        i32::try_from(effect.step_time(&ctx)).unwrap_or(250)
    }

    /// Advance the effect by one frame.
    pub fn step(&mut self, time_ms: i32) -> i32 {
        let Some(effect) = self.effect.as_mut() else {
            return StepResult::Error as i32;
        };
        let Ok(time_ms) = u32::try_from(time_ms) else {
            return StepResult::Error as i32;
        };

        let mut ctx = Context::new(
            &mut self.frame[..self.frame_len],
            &mut self.command,
            &mut self.parameters,
        );
        effect.step(&mut ctx, time_ms) as i32
    }

    /// Pointer to the frame buffer.
    pub fn frame_ptr(&self) -> i32 {
        self.frame.as_ptr() as usize as i32
    }

    /// Length of the populated portion of the frame buffer in bytes.
    pub fn frame_len(&self) -> i32 {
        i32::try_from(self.frame_len).unwrap_or(i32::MAX)
    }

    /// Pointer to the command buffer.
    pub fn command_ptr(&self) -> i32 {
        self.command.as_ptr() as usize as i32
    }

    /// Length of the command buffer in bytes.
    pub fn command_len(&self) -> i32 {
        i32::try_from(COMMAND_BYTES).unwrap_or(i32::MAX)
    }

    /// Pointer to the parameter buffer.
    pub fn parameter_ptr(&self) -> i32 {
        self.parameters.as_ptr() as usize as i32
    }

    /// Length of the parameter buffer in bytes.
    pub fn parameter_len(&self) -> i32 {
        i32::try_from(PARAMETER_BYTES).unwrap_or(i32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bled_core::RGB8;
    use bled_core::effect::abi::export::StepResult;

    struct TestEffect {
        _fixtures: Fixtures,
    }

    impl Effect for TestEffect {
        fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()> {
            ctx.parameters_mut()[0] = 7;
            Ok(Self {
                _fixtures: fixtures,
            })
        }

        fn step_time(&self, _ctx: &Context<'_>) -> u32 {
            42
        }

        fn step(&mut self, ctx: &mut Context<'_>, _timestamp_ms: u32) -> StepResult {
            ctx.frame_mut().pixels_mut()[0] = RGB8::new(1, 2, 3);
            StepResult::Changed
        }
    }

    struct FailingEffect;

    impl Effect for FailingEffect {
        fn new(_ctx: &mut Context<'_>, _fixtures: Fixtures) -> Result<Self, ()> {
            Err(())
        }

        fn step_time(&self, _ctx: &Context<'_>) -> u32 {
            0
        }

        fn step(&mut self, _ctx: &mut Context<'_>, _timestamp_ms: u32) -> StepResult {
            StepResult::Error
        }
    }

    fn fixtures() -> Fixtures {
        bled_core::io::decode_json(
            br#"[
                {
                    "name": "fixture",
                    "layout": {"linear_array": {}},
                    "spans": [
                        {"name": "span", "channel": 0, "start": 0, "length": 2}
                    ]
                }
            ]"#,
        )
        .unwrap()
    }

    #[test]
    fn initializes_and_runs_an_effect() {
        let mut runtime = Runtime::<TestEffect, 6, 2, 2>::new();

        assert_eq!(runtime.init_from_fixtures(fixtures()), InitResult::Success);
        assert_eq!(runtime.frame_len(), 6);
        assert_eq!(runtime.step_time(), 42);
        assert_eq!(runtime.step(10), StepResult::Changed as i32);
    }

    #[test]
    fn rejects_configuration_that_exceeds_the_frame_buffer() {
        let mut runtime = Runtime::<TestEffect, 3, 0, 0>::new();

        assert_eq!(
            runtime.init_from_fixtures(fixtures()),
            InitResult::FrameBufferError
        );
    }

    #[test]
    fn maps_effect_initialization_errors() {
        let mut runtime = Runtime::<FailingEffect, 6, 0, 0>::new();

        assert_eq!(
            runtime.init_from_fixtures(fixtures()),
            InitResult::OtherError
        );
    }

    #[test]
    fn reports_defaults_before_initialization() {
        let mut runtime = Runtime::<TestEffect, 6, 0, 0>::new();

        assert_eq!(runtime.step_time(), 250);
        assert_eq!(runtime.step(10), StepResult::Error as i32);
    }
}
