use super::Frame;
use bled_core::config::Fixtures;

/// Buffers and state made available to an effect while it runs.
pub struct Context<'a> {
    fixtures: &'a Fixtures,
    frame: Frame<'a>,
    command: &'a mut [u8],
    parameters: &'a mut [u8],
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        fixtures: &'a Fixtures,
        frame: &'a mut [u8],
        command: &'a mut [u8],
        parameters: &'a mut [u8],
    ) -> Self {
        Self {
            fixtures,
            frame: Frame::new(frame),
            command,
            parameters,
        }
    }

    /// Read the device's fixture configuration.
    pub fn fixtures(&self) -> &'a Fixtures {
        self.fixtures
    }

    /// Read the effect's output frame.
    pub fn frame(&self) -> &Frame<'a> {
        &self.frame
    }

    /// Mutate the effect's output frame.
    pub fn frame_mut(&mut self) -> &mut Frame<'a> {
        &mut self.frame
    }

    /// Read the effect's command buffer.
    pub fn command(&self) -> &[u8] {
        self.command
    }

    /// Mutate the effect's command buffer.
    pub fn command_mut(&mut self) -> &mut [u8] {
        self.command
    }

    /// Read the effect's parameter buffer.
    pub fn parameters(&self) -> &[u8] {
        self.parameters
    }

    /// Mutate the effect's parameter buffer.
    pub fn parameters_mut(&mut self) -> &mut [u8] {
        self.parameters
    }

    /// Return 64 bits of entropy from the host.
    pub fn entropy_u64(&self) -> u64 {
        super::runtime::entropy_u64()
    }

    /// Fill a byte array with entropy by chaining 64-bit calls to the host.
    pub fn entropy<const N: usize>(&self) -> [u8; N] {
        let mut buf = [0u8; N];
        for chunk in buf.chunks_mut(8) {
            let bytes = self.entropy_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        buf
    }
}
