use super::Frame;

/// Buffers and state made available to an effect while it runs.
pub struct Context<'a> {
    frame: Frame<'a>,
    command: &'a mut [u8],
    parameters: &'a mut [u8],
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        frame: &'a mut [u8],
        command: &'a mut [u8],
        parameters: &'a mut [u8],
    ) -> Self {
        Self {
            frame: Frame::new(frame),
            command,
            parameters,
        }
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
}
