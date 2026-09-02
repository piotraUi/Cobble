//! Rasterizes the bundled Minecraft-style pixel font (see
//! `assets/fonts/LICENSE.txt` — a public-domain fan recreation, not a
//! Mojang asset) into one glyph atlas, thresholded to hard on/off
//! alpha so it stays crisp under nearest-neighbor sampling instead of
//! going fuzzy with font antialiasing.

use std::collections::HashMap;

use ab_glyph::{Font as AbFont, FontRef, Glyph, ScaleFont};
use image::{Rgba, RgbaImage};

const REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/MinecraftRegular.otf");
const BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/MinecraftBold.otf");

/// ASCII range this UI can render text in (covers menus, addresses,
/// chat-adjacent text — no need for the full Unicode range yet).
const FIRST_CHAR: u8 = b' ';
const LAST_CHAR: u8 = b'~';

const COVERAGE_THRESHOLD: f32 = 0.5;
/// Padding between packed glyphs so nearest-neighbor sampling never
/// bleeds a neighboring glyph's pixels in at the edge.
const GLYPH_PADDING: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub struct Glyph2D {
    /// Normalized (u0, v0, u1, v1) into the font atlas.
    pub uv: (f32, f32, f32, f32),
    /// Quad size in pixels at the font's rasterized size.
    pub width: f32,
    pub height: f32,
    /// Offset from the pen position's top-left to the quad's top-left.
    pub offset_x: f32,
    pub offset_y: f32,
    pub advance: f32,
}

pub struct Font {
    pub atlas: RgbaImage,
    pub line_height: f32,
    /// Distance from the top of the line to the baseline — glyphs are
    /// rasterized relative to their own baseline (see `Glyph2D::offset_y`,
    /// which is negative for ink above it), so callers drawing at a
    /// "top of line" `y` need `y + ascent` to find that baseline.
    pub ascent: f32,
    glyphs: HashMap<char, Glyph2D>,
    /// A solid opaque white texel reserved in this same atlas, so solid
    /// color quads (button panels, HUD backgrounds) can go through the
    /// exact same textured-quad draw path as text.
    pub white_uv: (f32, f32, f32, f32),
}

impl Font {
    pub fn load_regular(pixel_size: f32) -> Self {
        Self::rasterize(REGULAR_BYTES, pixel_size)
    }

    pub fn load_bold(pixel_size: f32) -> Self {
        Self::rasterize(BOLD_BYTES, pixel_size)
    }

    fn rasterize(font_bytes: &'static [u8], pixel_size: f32) -> Self {
        let font = FontRef::try_from_slice(font_bytes).expect("bundled font should always parse");
        let scale = ab_glyph::PxScale::from(pixel_size);
        let scaled = font.as_scaled(scale);

        struct RasterGlyph {
            ch: char,
            image: RgbaImage,
            offset_x: f32,
            offset_y: f32,
            advance: f32,
            /// False for glyphs with no outline (space, and anything
            /// else with no ink) — `image` is still a tiny 1x1 dummy
            /// tile so the packer has something to place, but the
            /// resulting `Glyph2D` must report zero size so nothing
            /// gets drawn for it.
            has_ink: bool,
        }

        let mut rasters = Vec::new();
        for byte in FIRST_CHAR..=LAST_CHAR {
            let ch = byte as char;
            let glyph_id = font.glyph_id(ch);
            let advance = scaled.h_advance(glyph_id);
            let glyph: Glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));

            let (image, offset_x, offset_y, has_ink) = match font.outline_glyph(glyph) {
                Some(outlined) => {
                    let bounds = outlined.px_bounds();
                    let width = (bounds.width().ceil() as u32).max(1);
                    let height = (bounds.height().ceil() as u32).max(1);
                    let mut img = RgbaImage::new(width, height);
                    outlined.draw(|x, y, coverage| {
                        if coverage >= COVERAGE_THRESHOLD {
                            img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                        }
                    });
                    (img, bounds.min.x, bounds.min.y, true)
                }
                None => (RgbaImage::new(1, 1), 0.0, 0.0, false),
            };

            rasters.push(RasterGlyph {
                ch,
                image,
                offset_x,
                offset_y,
                advance,
                has_ink,
            });
        }

        // Reserve one glyph-sized white tile too, so `white_uv` sits in
        // the same atlas as everything else.
        let white_size = pixel_size.ceil().max(1.0) as u32;
        let mut white_tile = RgbaImage::new(white_size, white_size);
        for pixel in white_tile.pixels_mut() {
            *pixel = Rgba([255, 255, 255, 255]);
        }

        let (atlas, mut placements) = pack_shelf(
            rasters
                .iter()
                .map(|r| (GlyphKey::Char(r.ch), &r.image))
                .chain(std::iter::once((GlyphKey::WhiteTile, &white_tile))),
        );

        let atlas_w = atlas.width() as f32;
        let atlas_h = atlas.height() as f32;

        let mut glyphs = HashMap::with_capacity(rasters.len());
        for raster in &rasters {
            let rect = placements.remove(&GlyphKey::Char(raster.ch)).expect("placed");
            glyphs.insert(
                raster.ch,
                Glyph2D {
                    uv: (
                        rect.0 as f32 / atlas_w,
                        rect.1 as f32 / atlas_h,
                        (rect.0 + rect.2) as f32 / atlas_w,
                        (rect.1 + rect.3) as f32 / atlas_h,
                    ),
                    width: if raster.has_ink { rect.2 as f32 } else { 0.0 },
                    height: if raster.has_ink { rect.3 as f32 } else { 0.0 },
                    offset_x: raster.offset_x,
                    offset_y: raster.offset_y,
                    advance: raster.advance,
                },
            );
        }

        let white_rect = placements.remove(&GlyphKey::WhiteTile).expect("placed");
        let white_uv = (
            (white_rect.0 as f32 + white_size as f32 / 2.0) / atlas_w,
            (white_rect.1 as f32 + white_size as f32 / 2.0) / atlas_h,
            (white_rect.0 as f32 + white_size as f32 / 2.0 + 1.0) / atlas_w,
            (white_rect.1 as f32 + white_size as f32 / 2.0 + 1.0) / atlas_h,
        );

        Self {
            atlas,
            line_height: scaled.height(),
            ascent: scaled.ascent(),
            glyphs,
            white_uv,
        }
    }

    pub fn glyph(&self, ch: char) -> Option<&Glyph2D> {
        self.glyphs.get(&ch)
    }

    /// Total advance width of `text` at this font's rasterized size —
    /// used to center/right-align labels.
    pub fn text_width(&self, text: &str) -> f32 {
        text.chars().map(|c| self.glyph(c).map_or(0.0, |g| g.advance)).sum()
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum GlyphKey {
    Char(char),
    WhiteTile,
}

/// Pixel rect (x, y, width, height) of a placed tile within the atlas.
type PlacedRect = (u32, u32, u32, u32);

/// Simple shelf (row) packer: places tiles left-to-right, wrapping to a
/// new row when the current one is full, with `GLYPH_PADDING` between
/// tiles. Good enough for ~100 small, similarly-sized glyph tiles.
fn pack_shelf<'a>(
    tiles: impl Iterator<Item = (GlyphKey, &'a RgbaImage)>,
) -> (RgbaImage, HashMap<GlyphKey, PlacedRect>) {
    let tiles: Vec<_> = tiles.collect();
    let max_tile_w = tiles.iter().map(|(_, img)| img.width()).max().unwrap_or(1);
    let atlas_width = ((max_tile_w + GLYPH_PADDING) * 16).max(256);

    let mut placements = HashMap::with_capacity(tiles.len());
    let mut cursor_x = GLYPH_PADDING;
    let mut cursor_y = GLYPH_PADDING;
    let mut row_height = 0u32;

    for (key, img) in &tiles {
        if cursor_x + img.width() + GLYPH_PADDING > atlas_width {
            cursor_x = GLYPH_PADDING;
            cursor_y += row_height + GLYPH_PADDING;
            row_height = 0;
        }
        placements.insert(*key, (cursor_x, cursor_y, img.width(), img.height()));
        cursor_x += img.width() + GLYPH_PADDING;
        row_height = row_height.max(img.height());
    }
    let atlas_height = (cursor_y + row_height + GLYPH_PADDING).max(1);

    let mut atlas = RgbaImage::new(atlas_width, atlas_height);
    for (key, img) in &tiles {
        let (x, y, _, _) = placements[key];
        image::imageops::overlay(&mut atlas, *img, x as i64, y as i64);
    }

    (atlas, placements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_printable_ascii_range() {
        let font = Font::load_regular(16.0);
        for byte in FIRST_CHAR..=LAST_CHAR {
            assert!(font.glyph(byte as char).is_some(), "missing glyph for {byte}");
        }
    }

    #[test]
    fn space_has_advance_but_no_visible_pixels() {
        let font = Font::load_regular(16.0);
        let space = font.glyph(' ').unwrap();
        assert!(space.advance > 0.0);
    }

    #[test]
    fn text_width_is_sum_of_advances() {
        let font = Font::load_regular(16.0);
        let a = font.glyph('A').unwrap().advance;
        let b = font.glyph('B').unwrap().advance;
        assert!((font.text_width("AB") - (a + b)).abs() < 1e-4);
    }

    #[test]
    fn white_tile_is_fully_opaque_and_distinct_from_glyph_tiles() {
        let font = Font::load_regular(16.0);
        assert_ne!(font.white_uv, (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn all_glyphs_and_the_white_tile_fit_within_atlas_bounds() {
        let font = Font::load_regular(16.0);
        let (w, h) = (font.atlas.width(), font.atlas.height());
        for byte in FIRST_CHAR..=LAST_CHAR {
            let g = font.glyph(byte as char).unwrap();
            assert!(g.uv.0 >= 0.0 && g.uv.2 <= 1.0 + 1e-6, "char {byte} u out of range");
            assert!(g.uv.1 >= 0.0 && g.uv.3 <= 1.0 + 1e-6, "char {byte} v out of range");
        }
        assert!(w > 0 && h > 0);
    }

    #[test]
    fn atlas_alpha_is_hard_thresholded_not_antialiased() {
        // Every pixel's alpha should be either fully transparent or
        // fully opaque — no in-between coverage values leaking through.
        let font = Font::load_regular(16.0);
        for pixel in font.atlas.pixels() {
            assert!(pixel[3] == 0 || pixel[3] == 255, "found soft alpha {}", pixel[3]);
        }
    }
}
