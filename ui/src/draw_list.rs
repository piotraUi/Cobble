//! A renderer-agnostic list of textured quads (the only primitive this
//! whole UI needs — see the module docs on `font::Font::white_uv` for
//! how solid-color panels/buttons share the same draw path as text).

use crate::font::Font;
use crate::geometry::{Color, Rect};

#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub rect: Rect,
    pub uv: (f32, f32, f32, f32),
    pub color: Color,
}

#[derive(Default)]
pub struct DrawList {
    pub quads: Vec<Quad>,
}

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, quad: Quad) {
        self.quads.push(quad);
    }
}

/// Builds a `DrawList` against a specific `Font`, offering the actual
/// drawing verbs (rect/text/border) screens are written against.
pub struct Painter<'a> {
    pub list: DrawList,
    font: &'a Font,
}

impl<'a> Painter<'a> {
    pub fn new(font: &'a Font) -> Self {
        Self {
            list: DrawList::new(),
            font,
        }
    }

    pub fn font(&self) -> &Font {
        self.font
    }

    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.list.push(Quad {
            rect,
            uv: self.font.white_uv,
            color,
        });
    }

    /// A rectangle outline `thickness` px wide, drawn as 4 solid quads.
    pub fn border(&mut self, rect: Rect, thickness: f32, color: Color) {
        self.rect(Rect::new(rect.x, rect.y, rect.w, thickness), color);
        self.rect(Rect::new(rect.x, rect.y + rect.h - thickness, rect.w, thickness), color);
        self.rect(Rect::new(rect.x, rect.y, thickness, rect.h), color);
        self.rect(Rect::new(rect.x + rect.w - thickness, rect.y, thickness, rect.h), color);
    }

    /// Draws `text` with the top-left of its line at `(x, y)`,
    /// returning the total advance width. Individual glyphs are
    /// rasterized relative to their own baseline (see
    /// `font::Glyph2D::offset_y`), so this converts `y` (top of line)
    /// to a baseline via the font's ascent before placing them.
    pub fn text(&mut self, text: &str, x: f32, y: f32, color: Color) -> f32 {
        let mut pen_x = x;
        let baseline_y = y + self.font.ascent;
        for ch in text.chars() {
            let Some(glyph) = self.font.glyph(ch) else {
                continue;
            };
            if glyph.width > 0.0 && glyph.height > 0.0 {
                self.list.push(Quad {
                    rect: Rect::new(pen_x + glyph.offset_x, baseline_y + glyph.offset_y, glyph.width, glyph.height),
                    uv: glyph.uv,
                    color,
                });
            }
            pen_x += glyph.advance;
        }
        pen_x - x
    }

    /// Draws `text` horizontally centered on `center_x`, top edge at `y`.
    pub fn text_centered(&mut self, text: &str, center_x: f32, y: f32, color: Color) -> f32 {
        let width = self.font.text_width(text);
        self.text(text, center_x - width / 2.0, y, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_emits_one_quad_per_visible_glyph() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        painter.text("AB", 0.0, 0.0, Color::WHITE);
        // Both 'A' and 'B' have visible ink, so 2 quads.
        assert_eq!(painter.list.quads.len(), 2);
    }

    #[test]
    fn space_emits_no_quad_but_still_advances() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        let advance = painter.text("A A", 0.0, 0.0, Color::WHITE);
        assert_eq!(painter.list.quads.len(), 2); // just the two 'A's
        assert!(advance > 0.0);
    }

    #[test]
    fn centered_text_is_actually_centered() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        painter.text_centered("A", 100.0, 0.0, Color::WHITE);
        let quad = painter.list.quads[0];
        let glyph_center = quad.rect.x + quad.rect.w / 2.0;
        assert!((glyph_center - 100.0).abs() < 1.0);
    }

    #[test]
    fn rect_and_border_use_the_fonts_white_texel() {
        let font = Font::load_regular(16.0);
        let mut painter = Painter::new(&font);
        painter.rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        assert_eq!(painter.list.quads[0].uv, font.white_uv);
    }
}
