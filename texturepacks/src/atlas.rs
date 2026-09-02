//! Packs a set of named 16x16 tiles (pack textures plus fallbacks for
//! anything missing) into one square atlas image, with a UV rect per
//! name — the CPU-side pixel data the `renderer` crate uploads as a
//! single GPU texture for both the world and the UI.

use std::collections::HashMap;

use image::{imageops, RgbaImage};

use crate::fallback::TEXTURE_SIZE;

#[derive(Debug, Clone, Copy)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
}

impl AtlasRect {
    /// Normalized (u0, v0, u1, v1) UV rect for this tile within `atlas_size`.
    pub fn uv(&self, atlas_size: u32) -> (f32, f32, f32, f32) {
        let size = atlas_size as f32;
        (
            self.x as f32 / size,
            self.y as f32 / size,
            (self.x + TEXTURE_SIZE) as f32 / size,
            (self.y + TEXTURE_SIZE) as f32 / size,
        )
    }
}

pub struct TextureAtlas {
    pub image: RgbaImage,
    pub tile_size: u32,
    rects: HashMap<String, AtlasRect>,
}

impl TextureAtlas {
    pub fn size(&self) -> u32 {
        self.image.width()
    }

    pub fn rect(&self, name: &str) -> Option<AtlasRect> {
        self.rects.get(name).copied()
    }

    pub fn uv(&self, name: &str) -> Option<(f32, f32, f32, f32)> {
        self.rect(name).map(|r| r.uv(self.size()))
    }
}

/// Packs `tiles` (name -> already-16x16 RGBA image; any tile that
/// isn't the expected size is cropped/padded to fit) into one square
/// atlas, arranged left-to-right, top-to-bottom.
pub fn build_atlas(tiles: Vec<(String, RgbaImage)>) -> TextureAtlas {
    let tile_count = tiles.len().max(1);
    let grid_size = (tile_count as f64).sqrt().ceil() as u32;
    let atlas_size = (grid_size * TEXTURE_SIZE).max(TEXTURE_SIZE);

    let mut atlas = RgbaImage::new(atlas_size, atlas_size);
    let mut rects = HashMap::with_capacity(tiles.len());

    for (index, (name, mut tile)) in tiles.into_iter().enumerate() {
        if tile.width() != TEXTURE_SIZE || tile.height() != TEXTURE_SIZE {
            tile = imageops::resize(&tile, TEXTURE_SIZE, TEXTURE_SIZE, imageops::FilterType::Nearest);
        }
        let col = (index as u32) % grid_size;
        let row = (index as u32) / grid_size;
        let x = col * TEXTURE_SIZE;
        let y = row * TEXTURE_SIZE;

        imageops::overlay(&mut atlas, &tile, x as i64, y as i64);
        rects.insert(name, AtlasRect { x, y });
    }

    TextureAtlas {
        image: atlas,
        tile_size: TEXTURE_SIZE,
        rects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fallback::generate_fallback_texture;

    #[test]
    fn every_tile_gets_a_distinct_non_overlapping_rect() {
        let tiles: Vec<(String, RgbaImage)> = (0..17)
            .map(|i| (format!("tile_{i}"), generate_fallback_texture()))
            .collect();
        let names: Vec<String> = tiles.iter().map(|(n, _)| n.clone()).collect();
        let atlas = build_atlas(tiles);

        let mut seen = std::collections::HashSet::new();
        for name in &names {
            let rect = atlas.rect(name).expect("every tile should have a rect");
            assert!(seen.insert((rect.x, rect.y)), "overlapping rect for {name}");
            assert!(rect.x + TEXTURE_SIZE <= atlas.size());
            assert!(rect.y + TEXTURE_SIZE <= atlas.size());
        }
    }

    #[test]
    fn uv_rect_is_normalized_into_0_to_1() {
        let tiles = vec![("stone".to_string(), generate_fallback_texture())];
        let atlas = build_atlas(tiles);
        let (u0, v0, u1, v1) = atlas.uv("stone").unwrap();
        assert!((0.0..=1.0).contains(&u0));
        assert!((0.0..=1.0).contains(&v0));
        assert!((0.0..=1.0).contains(&u1));
        assert!((0.0..=1.0).contains(&v1));
        assert!(u1 > u0);
        assert!(v1 > v0);
    }

    #[test]
    fn unknown_name_has_no_rect() {
        let atlas = build_atlas(vec![("stone".to_string(), generate_fallback_texture())]);
        assert!(atlas.rect("does_not_exist").is_none());
    }

    #[test]
    fn oversized_tile_is_resized_to_fit() {
        let big = RgbaImage::new(32, 32);
        let atlas = build_atlas(vec![("weird".to_string(), big)]);
        assert_eq!(atlas.image.width() % TEXTURE_SIZE, 0);
        let rect = atlas.rect("weird").unwrap();
        assert!(rect.x + TEXTURE_SIZE <= atlas.size());
    }
}
