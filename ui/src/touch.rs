//! Mobile controls: a virtual joystick (movement), touch-drag look,
//! and a handful of on-screen buttons (jump, mine, place, hotbar
//! slots), all tracked by touch id so the joystick, a look-drag, and a
//! button press can all be active from different fingers at once.
//! Reuses `Painter`/`DrawList` — nothing here is Android-specific, so
//! it's exercised by ordinary unit tests without a device.

use crate::draw_list::Painter;
use crate::geometry::{Color, Rect};
use crate::widgets::Button;

/// A touch identifier, as delivered by the platform (e.g. winit's
/// `Touch::id`) — stable for the duration of one finger's contact.
pub type TouchId = u64;

pub struct VirtualJoystick {
    pub center: (f32, f32),
    pub radius: f32,
    touch: Option<TouchId>,
    knob_offset: (f32, f32),
}

impl VirtualJoystick {
    pub fn new(center: (f32, f32), radius: f32) -> Self {
        Self {
            center,
            radius,
            touch: None,
            knob_offset: (0.0, 0.0),
        }
    }

    fn contains(&self, pos: (f32, f32)) -> bool {
        let dx = pos.0 - self.center.0;
        let dy = pos.1 - self.center.1;
        // A generous catch area (2x the visible base) is easier to hit
        // with a thumb than the exact circle.
        dx * dx + dy * dy <= (self.radius * 2.0) * (self.radius * 2.0)
    }

    /// Claims `id` if `pos` is within the joystick's base and no touch
    /// currently owns it. Returns whether it was claimed.
    fn try_claim(&mut self, id: TouchId, pos: (f32, f32)) -> bool {
        if self.touch.is_none() && self.contains(pos) {
            self.touch = Some(id);
            self.set_knob_from(pos);
            true
        } else {
            false
        }
    }

    fn set_knob_from(&mut self, pos: (f32, f32)) {
        let dx = pos.0 - self.center.0;
        let dy = pos.1 - self.center.1;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= self.radius || len == 0.0 {
            self.knob_offset = (dx, dy);
        } else {
            self.knob_offset = (dx / len * self.radius, dy / len * self.radius);
        }
    }

    fn moved(&mut self, id: TouchId, pos: (f32, f32)) {
        if self.touch == Some(id) {
            self.set_knob_from(pos);
        }
    }

    fn released(&mut self, id: TouchId) {
        if self.touch == Some(id) {
            self.touch = None;
            self.knob_offset = (0.0, 0.0);
        }
    }

    /// Normalized (x, y) in roughly [-1, 1], y positive = pulled down.
    pub fn value(&self) -> (f32, f32) {
        if self.radius == 0.0 {
            return (0.0, 0.0);
        }
        (self.knob_offset.0 / self.radius, self.knob_offset.1 / self.radius)
    }

    pub fn draw(&self, painter: &mut Painter) {
        let base = Rect::new(self.center.0 - self.radius, self.center.1 - self.radius, self.radius * 2.0, self.radius * 2.0);
        painter.rect(base, Color::rgba(0.1, 0.1, 0.1, 0.4));
        painter.border(base, 2.0, Color::rgba(0.0, 0.0, 0.0, 0.6));

        let knob_size = self.radius * 0.7;
        let knob = Rect::new(
            self.center.0 + self.knob_offset.0 - knob_size / 2.0,
            self.center.1 + self.knob_offset.1 - knob_size / 2.0,
            knob_size,
            knob_size,
        );
        let knob_color = if self.touch.is_some() {
            Color::rgba(0.8, 0.8, 0.8, 0.8)
        } else {
            Color::rgba(0.6, 0.6, 0.6, 0.6)
        };
        painter.rect(knob, knob_color);
    }
}

/// A simple on/off touch button (jump, mine, place, ...): tracks the
/// one touch holding it down, if any.
pub struct TouchButton {
    pub rect: Rect,
    pub label: String,
    touch: Option<TouchId>,
}

impl TouchButton {
    pub fn new(rect: Rect, label: impl Into<String>) -> Self {
        Self {
            rect,
            label: label.into(),
            touch: None,
        }
    }

    fn try_claim(&mut self, id: TouchId, pos: (f32, f32)) -> bool {
        if self.touch.is_none() && self.rect.contains(pos.0, pos.1) {
            self.touch = Some(id);
            true
        } else {
            false
        }
    }

    fn released(&mut self, id: TouchId) {
        if self.touch == Some(id) {
            self.touch = None;
        }
    }

    pub fn is_held(&self) -> bool {
        self.touch.is_some()
    }

    pub fn draw(&self, painter: &mut Painter) {
        let mut button = Button::new(self.rect, self.label.clone());
        button.enabled = true;
        let hovered_pos = if self.is_held() { self.rect.center_x() } else { f32::NEG_INFINITY };
        button.draw(painter, (hovered_pos, self.rect.y + self.rect.h / 2.0));
    }
}

/// Everything needed to drive the game from touch input: a movement
/// joystick (bottom-left), a look-drag anywhere else on screen, and a
/// row of action buttons (bottom-right: jump/mine/place).
pub struct TouchController {
    pub joystick: VirtualJoystick,
    pub jump: TouchButton,
    pub mine: TouchButton,
    pub place: TouchButton,
    look_touch: Option<TouchId>,
    look_last_pos: (f32, f32),
    look_delta: (f32, f32),
}

const JOYSTICK_RADIUS: f32 = 60.0;
const ACTION_BUTTON_SIZE: f32 = 64.0;
const EDGE_MARGIN: f32 = 24.0;

impl TouchController {
    pub fn new(viewport: (f32, f32)) -> Self {
        let mut controller = Self {
            joystick: VirtualJoystick::new((0.0, 0.0), JOYSTICK_RADIUS),
            jump: TouchButton::new(Rect::new(0.0, 0.0, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE), "Jump"),
            mine: TouchButton::new(Rect::new(0.0, 0.0, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE), "Mine"),
            place: TouchButton::new(Rect::new(0.0, 0.0, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE), "Place"),
            look_touch: None,
            look_last_pos: (0.0, 0.0),
            look_delta: (0.0, 0.0),
        };
        controller.relayout(viewport);
        controller
    }

    pub fn relayout(&mut self, viewport: (f32, f32)) {
        self.joystick.center = (EDGE_MARGIN + JOYSTICK_RADIUS, viewport.1 - EDGE_MARGIN - JOYSTICK_RADIUS);

        let bx = viewport.0 - EDGE_MARGIN - ACTION_BUTTON_SIZE;
        let by = viewport.1 - EDGE_MARGIN - ACTION_BUTTON_SIZE;
        self.jump.rect = Rect::new(bx, by, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE);
        self.mine.rect = Rect::new(bx - ACTION_BUTTON_SIZE - 12.0, by, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE);
        self.place.rect = Rect::new(bx - (ACTION_BUTTON_SIZE + 12.0) * 2.0, by, ACTION_BUTTON_SIZE, ACTION_BUTTON_SIZE);
    }

    pub fn touch_down(&mut self, id: TouchId, pos: (f32, f32)) {
        if self.joystick.try_claim(id, pos) {
            return;
        }
        if self.jump.try_claim(id, pos) || self.mine.try_claim(id, pos) || self.place.try_claim(id, pos) {
            return;
        }
        if self.look_touch.is_none() {
            self.look_touch = Some(id);
            self.look_last_pos = pos;
        }
    }

    pub fn touch_moved(&mut self, id: TouchId, pos: (f32, f32)) {
        self.joystick.moved(id, pos);
        if self.look_touch == Some(id) {
            self.look_delta.0 += pos.0 - self.look_last_pos.0;
            self.look_delta.1 += pos.1 - self.look_last_pos.1;
            self.look_last_pos = pos;
        }
    }

    pub fn touch_up(&mut self, id: TouchId) {
        self.joystick.released(id);
        self.jump.released(id);
        self.mine.released(id);
        self.place.released(id);
        if self.look_touch == Some(id) {
            self.look_touch = None;
        }
    }

    /// Accumulated look-drag delta since the last call, then reset —
    /// mirrors `client_core::InputState::take_look_delta`.
    pub fn take_look_delta(&mut self) -> (f32, f32) {
        std::mem::take(&mut self.look_delta)
    }

    /// Normalized movement (x = strafe, y = forward/back, forward negative).
    pub fn movement(&self) -> (f32, f32) {
        self.joystick.value()
    }

    pub fn draw(&self, painter: &mut Painter) {
        self.joystick.draw(painter);
        self.jump.draw(painter);
        self.mine.draw(painter);
        self.place.draw(painter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;

    #[test]
    fn joystick_claims_a_touch_within_its_base_and_reports_value() {
        let mut stick = VirtualJoystick::new((100.0, 100.0), 60.0);
        assert!(stick.try_claim(1, (110.0, 100.0)));
        let (x, y) = stick.value();
        assert!(x > 0.0 && y.abs() < 1e-3);
    }

    #[test]
    fn joystick_ignores_a_second_touch_while_one_is_active() {
        let mut stick = VirtualJoystick::new((100.0, 100.0), 60.0);
        assert!(stick.try_claim(1, (110.0, 100.0)));
        assert!(!stick.try_claim(2, (90.0, 100.0)));
    }

    #[test]
    fn joystick_knob_is_clamped_to_its_radius() {
        let mut stick = VirtualJoystick::new((0.0, 0.0), 50.0);
        // Claim within the base, then drag the finger far outside it —
        // the knob should clamp to the base's edge, not follow forever.
        assert!(stick.try_claim(1, (10.0, 0.0)));
        stick.moved(1, (1000.0, 0.0));
        let (x, y) = stick.value();
        assert!((x - 1.0).abs() < 1e-3);
        assert!(y.abs() < 1e-3);
    }

    #[test]
    fn joystick_recenters_on_release() {
        let mut stick = VirtualJoystick::new((0.0, 0.0), 50.0);
        stick.try_claim(1, (40.0, 0.0));
        assert!(stick.value().0 > 0.0);
        stick.released(1);
        assert_eq!(stick.value(), (0.0, 0.0));
    }

    #[test]
    fn controller_routes_joystick_button_and_look_touches_independently() {
        let mut controller = TouchController::new((800.0, 600.0));

        // Finger 1: joystick.
        let joystick_pos = controller.joystick.center;
        controller.touch_down(1, joystick_pos);
        // Finger 2: jump button.
        let jump_center = (controller.jump.rect.center_x(), controller.jump.rect.y + 1.0);
        controller.touch_down(2, jump_center);
        // Finger 3: look drag, somewhere in the middle of the screen.
        controller.touch_down(3, (400.0, 300.0));
        controller.touch_moved(3, (420.0, 310.0));

        assert!(controller.jump.is_held());
        let (dx, dy) = controller.take_look_delta();
        assert!((dx - 20.0).abs() < 1e-3);
        assert!((dy - 10.0).abs() < 1e-3);

        controller.touch_up(2);
        assert!(!controller.jump.is_held());
    }

    #[test]
    fn look_delta_resets_after_being_taken() {
        let mut controller = TouchController::new((800.0, 600.0));
        controller.touch_down(1, (400.0, 300.0));
        controller.touch_moved(1, (450.0, 300.0));
        assert_ne!(controller.take_look_delta(), (0.0, 0.0));
        assert_eq!(controller.take_look_delta(), (0.0, 0.0));
    }

    #[test]
    fn drawing_does_not_panic() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        let controller = TouchController::new((800.0, 600.0));
        controller.draw(&mut painter);
        assert!(!painter.list.quads.is_empty());
    }
}
