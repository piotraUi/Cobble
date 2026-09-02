use crate::draw_list::Painter;
use crate::geometry::Rect;
use crate::input::UiInput;
use crate::style;

pub struct Button {
    pub rect: Rect,
    pub label: String,
    pub enabled: bool,
}

impl Button {
    pub fn new(rect: Rect, label: impl Into<String>) -> Self {
        Self {
            rect,
            label: label.into(),
            enabled: true,
        }
    }

    pub fn hovered(&self, mouse: (f32, f32)) -> bool {
        self.enabled && self.rect.contains(mouse.0, mouse.1)
    }

    /// Consumes the click if this button was actually hit — callers
    /// checking buttons in order (e.g. top to bottom) naturally get
    /// "only the topmost/first match" behavior for overlapping rects.
    pub fn clicked(&self, input: &UiInput) -> bool {
        input.clicked && self.hovered(input.mouse_pos)
    }

    pub fn draw(&self, painter: &mut Painter, mouse: (f32, f32)) {
        let (bg, text_color) = if !self.enabled {
            (style::BUTTON_DISABLED, style::TEXT_DISABLED)
        } else if self.hovered(mouse) {
            (style::BUTTON_HOVER, style::TEXT_PRIMARY)
        } else {
            (style::BUTTON_BASE, style::TEXT_PRIMARY)
        };

        painter.rect(self.rect, bg);
        painter.border(self.rect, 2.0, style::BUTTON_BORDER);

        let line_height = painter.font().line_height;
        let text_y = self.rect.y + (self.rect.h - line_height) / 2.0;
        let max_width = self.rect.w - LABEL_PADDING * 2.0;
        let label = truncate_to_width(painter.font(), &self.label, max_width);
        painter.text_centered(&label, self.rect.center_x(), text_y, text_color);
    }
}

const LABEL_PADDING: f32 = 6.0;

/// Shortens `text` with a trailing "..." if it wouldn't fit in
/// `max_width` at `font`'s size — long texture pack titles/download
/// counts shouldn't spill out of their button.
fn truncate_to_width(font: &crate::font::Font, text: &str, max_width: f32) -> String {
    if font.text_width(text) <= max_width {
        return text.to_string();
    }
    let ellipsis_width = font.text_width("...");
    let mut result = String::new();
    let mut width = ellipsis_width;
    for ch in text.chars() {
        let ch_width = font.glyph(ch).map_or(0.0, |g| g.advance);
        if width + ch_width > max_width {
            break;
        }
        width += ch_width;
        result.push(ch);
    }
    result.push_str("...");
    result
}

/// A minimal single-line text field: click to focus, type to append,
/// backspace to delete the last char, Enter to confirm (the screen
/// decides what "confirm" means). No cursor positioning/selection.
pub struct TextField {
    pub rect: Rect,
    pub value: String,
    pub placeholder: String,
    pub focused: bool,
}

impl TextField {
    pub fn new(rect: Rect, placeholder: impl Into<String>) -> Self {
        Self {
            rect,
            value: String::new(),
            placeholder: placeholder.into(),
            focused: false,
        }
    }

    /// Updates focus from clicks and, if focused, applies typed text /
    /// backspace. Returns true if Enter was pressed while focused.
    pub fn update(&mut self, input: &UiInput) -> bool {
        if input.clicked {
            self.focused = self.rect.contains(input.mouse_pos.0, input.mouse_pos.1);
        }
        if !self.focused {
            return false;
        }
        for ch in input.text_input.chars() {
            // Printable ASCII only — keeps this in the font's rendered range.
            if (' '..='~').contains(&ch) {
                self.value.push(ch);
            }
        }
        if input.backspace {
            self.value.pop();
        }
        input.enter
    }

    pub fn draw(&self, painter: &mut Painter) {
        painter.rect(self.rect, style::TEXT_FIELD_BG);
        let border_color = if self.focused {
            style::TEXT_FIELD_FOCUSED_BORDER
        } else {
            style::BUTTON_BORDER
        };
        painter.border(self.rect, 2.0, border_color);

        let line_height = painter.font().line_height;
        let text_y = self.rect.y + (self.rect.h - line_height) / 2.0;
        let text_x = self.rect.x + 6.0;
        if self.value.is_empty() && !self.focused {
            painter.text(&self.placeholder, text_x, text_y, style::TEXT_DISABLED);
        } else {
            painter.text(&self.value, text_x, text_y, style::TEXT_PRIMARY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;

    #[test]
    fn button_click_requires_hover() {
        let button = Button::new(Rect::new(10.0, 10.0, 20.0, 20.0), "Go");
        let mut input = UiInput {
            mouse_pos: (5.0, 5.0),
            clicked: true,
            ..Default::default()
        };
        assert!(!button.clicked(&input));
        input.mouse_pos = (15.0, 15.0);
        assert!(button.clicked(&input));
    }

    #[test]
    fn disabled_button_never_reports_hover_or_click() {
        let mut button = Button::new(Rect::new(0.0, 0.0, 20.0, 20.0), "Go");
        button.enabled = false;
        let input = UiInput {
            mouse_pos: (5.0, 5.0),
            clicked: true,
            ..Default::default()
        };
        assert!(!button.hovered(input.mouse_pos));
        assert!(!button.clicked(&input));
    }

    #[test]
    fn text_field_focuses_on_click_and_accepts_typed_text() {
        let mut field = TextField::new(Rect::new(0.0, 0.0, 100.0, 20.0), "address");
        let click = UiInput {
            mouse_pos: (10.0, 10.0),
            clicked: true,
            ..Default::default()
        };
        field.update(&click);
        assert!(field.focused);

        let typing = UiInput {
            text_input: "localhost".to_string(),
            ..Default::default()
        };
        field.update(&typing);
        assert_eq!(field.value, "localhost");
    }

    #[test]
    fn text_field_ignores_typing_while_unfocused() {
        let mut field = TextField::new(Rect::new(0.0, 0.0, 100.0, 20.0), "address");
        let typing = UiInput {
            text_input: "nope".to_string(),
            ..Default::default()
        };
        field.update(&typing);
        assert_eq!(field.value, "");
    }

    #[test]
    fn backspace_removes_last_char_only_while_focused() {
        let mut field = TextField::new(Rect::new(0.0, 0.0, 100.0, 20.0), "");
        field.focused = true;
        field.value = "abc".to_string();
        field.update(&UiInput {
            backspace: true,
            ..Default::default()
        });
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn enter_is_reported_only_while_focused() {
        let mut field = TextField::new(Rect::new(0.0, 0.0, 100.0, 20.0), "");
        assert!(!field.update(&UiInput {
            enter: true,
            ..Default::default()
        }));
        field.focused = true;
        assert!(field.update(&UiInput {
            enter: true,
            ..Default::default()
        }));
    }

    #[test]
    fn clicking_outside_defocuses() {
        let mut field = TextField::new(Rect::new(0.0, 0.0, 100.0, 20.0), "");
        field.focused = true;
        field.update(&UiInput {
            mouse_pos: (500.0, 500.0),
            clicked: true,
            ..Default::default()
        });
        assert!(!field.focused);
    }

    #[test]
    fn drawing_does_not_panic() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        let button = Button::new(Rect::new(0.0, 0.0, 50.0, 20.0), "Play");
        button.draw(&mut painter, (0.0, 0.0));
        let field = TextField::new(Rect::new(0.0, 30.0, 100.0, 20.0), "host");
        field.draw(&mut painter);
    }

    #[test]
    fn short_label_is_never_truncated() {
        let font = Font::load_regular(16.0);
        assert_eq!(truncate_to_width(&font, "Play", 1000.0), "Play");
    }

    #[test]
    fn long_label_is_truncated_with_ellipsis_and_fits() {
        let font = Font::load_regular(16.0);
        let long = "Faithful 32x (1234567 downloads)";
        let max_width = 150.0;
        let truncated = truncate_to_width(&font, long, max_width);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() < long.len());
        assert!(font.text_width(&truncated) <= max_width + 1.0);
    }

    #[test]
    fn a_real_button_draw_never_exceeds_its_own_width() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        let button = Button::new(Rect::new(0.0, 0.0, 100.0, 20.0), "A very very long label that will not fit");
        button.draw(&mut painter, (0.0, 0.0));
        for quad in &painter.list.quads {
            // Every glyph/box quad drawn for this button should stay
            // within the button's own horizontal bounds.
            assert!(quad.rect.x >= button.rect.x - 1.0, "quad started before the button");
            assert!(
                quad.rect.x + quad.rect.w <= button.rect.x + button.rect.w + 1.0,
                "quad at x={} w={} overflows button width {}",
                quad.rect.x,
                quad.rect.w,
                button.rect.w
            );
        }
    }
}
