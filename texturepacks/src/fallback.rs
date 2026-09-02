//! Original (non-Mojang) placeholder texture used for anything a
//! selected pack doesn't provide. Deliberately not the classic
//! magenta/black "missing texture" checker — that reads as an error,
//! and this is an expected, permanent stand-in — just a plain neutral
//! gray checkerboard, generated in code so nothing gets bundled or
//! redistributed.

use image::{Rgba, RgbaImage};

pub const TEXTURE_SIZE: u32 = 16;

const LIGHT: Rgba<u8> = Rgba([138, 138, 138, 255]);
const DARK: Rgba<u8> = Rgba([96, 96, 96, 255]);
const CELL_SIZE: u32 = 4;

/// A flat, deterministic 16x16 checkerboard — the same for every
/// missing texture, since the goal is an unobtrusive neutral filler,
/// not per-block variety.
pub fn generate_fallback_texture() -> RgbaImage {
    RgbaImage::from_fn(TEXTURE_SIZE, TEXTURE_SIZE, |x, y| {
        let checker = (x / CELL_SIZE + y / CELL_SIZE).is_multiple_of(2);
        if checker {
            LIGHT
        } else {
            DARK
        }
    })
}

const MISSING_A: Rgba<u8> = Rgba([230, 20, 230, 255]);
const MISSING_B: Rgba<u8> = Rgba([20, 20, 20, 255]);

/// The classic magenta/black "missing texture" checker — a generic
/// computer-graphics convention (not a Mojang asset) used only when
/// the renderer has no texture *mapping* at all for a block id, which
/// is a different, rarer case than a pack simply not covering a known
/// name (that uses `generate_fallback_texture` instead).
pub fn generate_missing_texture() -> RgbaImage {
    RgbaImage::from_fn(TEXTURE_SIZE, TEXTURE_SIZE, |x, y| {
        let checker = (x / CELL_SIZE + y / CELL_SIZE).is_multiple_of(2);
        if checker {
            MISSING_A
        } else {
            MISSING_B
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_the_requested_size() {
        let img = generate_fallback_texture();
        assert_eq!(img.width(), TEXTURE_SIZE);
        assert_eq!(img.height(), TEXTURE_SIZE);
    }

    #[test]
    fn is_deterministic() {
        assert_eq!(generate_fallback_texture(), generate_fallback_texture());
    }

    #[test]
    fn uses_only_the_two_neutral_gray_tones() {
        let img = generate_fallback_texture();
        for pixel in img.pixels() {
            assert!(*pixel == LIGHT || *pixel == DARK, "unexpected pixel {pixel:?}");
        }
    }

    #[test]
    fn is_actually_checkered_not_flat() {
        let img = generate_fallback_texture();
        let has_light = img.pixels().any(|p| *p == LIGHT);
        let has_dark = img.pixels().any(|p| *p == DARK);
        assert!(has_light && has_dark);
    }
}
