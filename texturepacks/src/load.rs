//! Ties the pieces together: open a downloaded pack `.zip`, validate
//! `pack.mcmeta`, check texture coverage, and build a texture atlas
//! using the pack's own textures where present and the neutral
//! fallback everywhere else.

use std::io::Read;
use std::path::Path;

use image::RgbaImage;

use crate::atlas::{build_atlas, TextureAtlas};
use crate::coverage::{check_archive_coverage, CoverageReport};
use crate::error::Result;
use crate::fallback::{generate_fallback_texture, generate_missing_texture};
use crate::known_textures::{BLOCK_TEXTURES, ITEM_TEXTURES};
use crate::modrinth::VersionFile;
use crate::pack::{read_pack_meta, PackMeta};

pub struct LoadedPack {
    pub meta: PackMeta,
    pub coverage: CoverageReport,
    pub atlas: TextureAtlas,
}

/// Atlas key for a block texture, e.g. `"blocks/stone"`. Kept distinct
/// from the item namespace since 1.8.9 packs use separate folders and
/// a pack could in principle ship both `blocks/foo` and `items/foo`.
pub fn block_atlas_key(name: &str) -> String {
    format!("blocks/{name}")
}

pub fn item_atlas_key(name: &str) -> String {
    format!("items/{name}")
}

/// Reserved atlas key for the classic magenta/black "missing texture"
/// tile, always present in every atlas this crate builds — used by the
/// renderer for block ids it has no texture mapping for at all.
pub const MISSING_TEXTURE_KEY: &str = "missing";

/// Builds an atlas covering every known block/item name with nothing
/// but the neutral fallback checker — no pack, no zip, no network.
/// This is what the renderer uses before the player has picked (or
/// while they haven't picked) a texture pack, so the world is always
/// texturable.
pub fn build_fallback_atlas() -> TextureAtlas {
    let mut tiles = Vec::with_capacity(BLOCK_TEXTURES.len() + ITEM_TEXTURES.len() + 1);
    tiles.push((MISSING_TEXTURE_KEY.to_string(), generate_missing_texture()));
    for &name in BLOCK_TEXTURES {
        tiles.push((block_atlas_key(name), generate_fallback_texture()));
    }
    for &name in ITEM_TEXTURES {
        tiles.push((item_atlas_key(name), generate_fallback_texture()));
    }
    build_atlas(tiles)
}

pub fn load_pack_from_zip(path: &Path) -> Result<LoadedPack> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let meta = read_pack_meta(&mut archive)?;
    let coverage = check_archive_coverage(&mut archive)?;

    let file_names: Vec<String> = archive.file_names().map(str::to_string).collect();

    let mut tiles = Vec::with_capacity(BLOCK_TEXTURES.len() + ITEM_TEXTURES.len() + 1);
    tiles.push((MISSING_TEXTURE_KEY.to_string(), generate_missing_texture()));
    for &name in BLOCK_TEXTURES {
        let key = block_atlas_key(name);
        let relative = format!("assets/minecraft/textures/blocks/{name}.png");
        tiles.push((key, load_or_fallback(&mut archive, &file_names, &relative)?));
    }
    for &name in ITEM_TEXTURES {
        let key = item_atlas_key(name);
        let relative = format!("assets/minecraft/textures/items/{name}.png");
        tiles.push((key, load_or_fallback(&mut archive, &file_names, &relative)?));
    }

    Ok(LoadedPack {
        meta,
        coverage,
        atlas: build_atlas(tiles),
    })
}

fn load_or_fallback<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    file_names: &[String],
    relative_path: &str,
) -> Result<RgbaImage> {
    let Some(full_name) = file_names.iter().find(|f| f.ends_with(relative_path)) else {
        return Ok(generate_fallback_texture());
    };

    let mut entry = archive.by_name(full_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    drop(entry);

    match image::load_from_memory(&bytes) {
        Ok(img) => Ok(img.to_rgba8()),
        Err(e) => {
            log::warn!("failed to decode texture {full_name}: {e}, using fallback");
            Ok(generate_fallback_texture())
        }
    }
}

/// Downloads (or reuses the cached copy of) `file` and loads it as a
/// ready-to-render pack.
pub async fn download_and_load(
    client: &reqwest::Client,
    slug: &str,
    file: &VersionFile,
) -> Result<(std::path::PathBuf, LoadedPack)> {
    let path = crate::cache::download_and_cache(client, slug, file).await?;
    let loaded = load_pack_from_zip(&path)?;
    Ok((path, loaded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    fn build_test_pack(include_stone: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            writer.start_file("pack.mcmeta", FileOptions::default()).unwrap();
            std::io::Write::write_all(
                &mut writer,
                br#"{"pack":{"pack_format":1,"description":"test"}}"#,
            )
            .unwrap();

            if include_stone {
                writer
                    .start_file("assets/minecraft/textures/blocks/stone.png", FileOptions::default())
                    .unwrap();
                let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([200, 10, 10, 255]));
                let mut png_bytes = Vec::new();
                image::DynamicImage::ImageRgba8(img)
                    .write_to(&mut Cursor::new(&mut png_bytes), image::ImageOutputFormat::Png)
                    .unwrap();
                std::io::Write::write_all(&mut writer, &png_bytes).unwrap();
            }

            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn fallback_atlas_covers_every_known_texture_with_no_pack_at_all() {
        let atlas = build_fallback_atlas();
        for &name in BLOCK_TEXTURES {
            assert!(atlas.rect(&block_atlas_key(name)).is_some());
        }
        for &name in ITEM_TEXTURES {
            assert!(atlas.rect(&item_atlas_key(name)).is_some());
        }
        assert!(atlas.rect(MISSING_TEXTURE_KEY).is_some());
    }

    #[test]
    fn loaded_pack_atlas_also_has_the_missing_texture_tile() {
        let dir = std::env::temp_dir().join(format!("cobble-test-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.zip");
        std::fs::write(&path, build_test_pack(false)).unwrap();

        let loaded = load_pack_from_zip(&path).unwrap();
        assert!(loaded.atlas.rect(MISSING_TEXTURE_KEY).is_some());

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn loads_pack_texture_when_present_and_fallback_otherwise() {
        let dir = std::env::temp_dir().join(format!("cobble-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.zip");
        std::fs::write(&path, build_test_pack(true)).unwrap();

        let loaded = load_pack_from_zip(&path).unwrap();
        assert!(loaded.meta.matches_1_8());
        assert_eq!(loaded.coverage.found, 1); // just "stone"
        assert!(loaded.atlas.rect(&block_atlas_key("stone")).is_some());
        assert!(loaded.atlas.rect(&block_atlas_key("dirt")).is_some()); // fallback still gets a slot

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn every_known_texture_gets_a_slot_even_with_an_empty_pack() {
        let dir = std::env::temp_dir().join(format!("cobble-test-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty_pack.zip");
        std::fs::write(&path, build_test_pack(false)).unwrap();

        let loaded = load_pack_from_zip(&path).unwrap();
        assert_eq!(loaded.coverage.found, 0);
        for &name in BLOCK_TEXTURES {
            assert!(loaded.atlas.rect(&block_atlas_key(name)).is_some());
        }
        for &name in ITEM_TEXTURES {
            assert!(loaded.atlas.rect(&item_atlas_key(name)).is_some());
        }

        std::fs::remove_file(&path).unwrap();
    }
}
