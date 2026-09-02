//! Per-frame UI input, accumulated by the host from window events and
//! handed to the active `Screen` once a frame — mirrors how
//! `client_core::InputState` decouples game input from winit.

#[derive(Default)]
pub struct UiInput {
    pub mouse_pos: (f32, f32),
    /// Left mouse button was pressed down since the last `take`.
    pub clicked: bool,
    /// Characters typed since the last `take` (already layout-resolved
    /// by the host, e.g. from winit's `KeyEvent::text`).
    pub text_input: String,
    pub backspace: bool,
    pub enter: bool,
}

impl UiInput {
    /// Returns a snapshot of the accumulated input and clears the
    /// per-frame flags (mouse position is sticky, everything else isn't).
    pub fn take(&mut self) -> UiInput {
        UiInput {
            mouse_pos: self.mouse_pos,
            clicked: std::mem::take(&mut self.clicked),
            text_input: std::mem::take(&mut self.text_input),
            backspace: std::mem::take(&mut self.backspace),
            enter: std::mem::take(&mut self.enter),
        }
    }
}
