/// Front-end-agnostic input state. Desktop (keyboard/mouse) and Android
/// (virtual joystick/touch, see roadmap step 6) both fill this struct in
/// their own event loops so `client-core` never has to know which one is
/// driving it.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputState {
    pub move_forward: bool,
    pub move_backward: bool,
    pub move_left: bool,
    pub move_right: bool,
    pub jump: bool,
    pub sneak: bool,
    /// Mouse/touch look delta accumulated since the last frame, in pixels.
    pub look_delta: (f32, f32),
}

impl InputState {
    pub fn take_look_delta(&mut self) -> (f32, f32) {
        std::mem::take(&mut self.look_delta)
    }
}
