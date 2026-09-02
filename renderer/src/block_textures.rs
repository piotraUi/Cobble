//! Maps the handful of `BlockId`s `client_core` currently knows about
//! to the block texture names `texturepacks` packs into the atlas.
//! Anything not listed here (which, against a real server, is most
//! block ids — this list is intentionally small right now) falls back
//! to the atlas's reserved "missing" tile.

use client_core::BlockId;
use texturepacks::TextureAtlas;

#[derive(Clone, Copy)]
pub enum FaceKind {
    Top,
    Bottom,
    Side,
}

/// Per-face texture names (unprefixed — see `texturepacks::block_atlas_key`)
/// for one block. Most blocks use the same texture on every face.
pub struct BlockFaces {
    pub top: &'static str,
    pub bottom: &'static str,
    pub side: &'static str,
}

impl BlockFaces {
    const fn uniform(name: &'static str) -> Self {
        Self {
            top: name,
            bottom: name,
            side: name,
        }
    }
}

pub fn block_faces(block: BlockId) -> Option<BlockFaces> {
    Some(match block {
        BlockId::STONE => BlockFaces::uniform("stone"),
        BlockId::DIRT => BlockFaces::uniform("dirt"),
        BlockId::GRASS => BlockFaces {
            top: "grass_top",
            bottom: "dirt",
            side: "grass_side",
        },
        BlockId::COBBLESTONE => BlockFaces::uniform("cobblestone"),
        BlockId::WOOD_PLANKS => BlockFaces::uniform("planks_oak"),
        BlockId::BEDROCK => BlockFaces::uniform("bedrock"),
        BlockId::SAND => BlockFaces::uniform("sand"),
        BlockId::LOG => BlockFaces {
            top: "log_oak_top",
            bottom: "log_oak_top",
            side: "log_oak",
        },
        BlockId::LEAVES => BlockFaces::uniform("leaves_oak"),
        _ => return None,
    })
}

/// Looks up the normalized UV rect the mesher should use for one face
/// of `block`, falling back to the atlas's reserved "missing" tile
/// when there's no texture mapping for this block id at all — every
/// atlas `texturepacks` builds always has that tile, so this never
/// leaves a block untextured.
pub fn face_uv(atlas: &TextureAtlas, block: BlockId, face: FaceKind) -> (f32, f32, f32, f32) {
    let key = block_faces(block).map(|faces| {
        let name = match face {
            FaceKind::Top => faces.top,
            FaceKind::Bottom => faces.bottom,
            FaceKind::Side => faces.side,
        };
        texturepacks::block_atlas_key(name)
    });

    key.and_then(|k| atlas.uv(&k))
        .or_else(|| atlas.uv(texturepacks::MISSING_TEXTURE_KEY))
        .unwrap_or((0.0, 0.0, 1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_block_resolves_to_its_own_atlas_tile() {
        let atlas = texturepacks::build_fallback_atlas();
        let uv = face_uv(&atlas, BlockId::STONE, FaceKind::Side);
        let expected = atlas.uv(&texturepacks::block_atlas_key("stone")).unwrap();
        assert_eq!(uv, expected);
    }

    #[test]
    fn grass_uses_different_textures_per_face() {
        let atlas = texturepacks::build_fallback_atlas();
        let top = face_uv(&atlas, BlockId::GRASS, FaceKind::Top);
        let side = face_uv(&atlas, BlockId::GRASS, FaceKind::Side);
        let bottom = face_uv(&atlas, BlockId::GRASS, FaceKind::Bottom);

        assert_eq!(top, atlas.uv(&texturepacks::block_atlas_key("grass_top")).unwrap());
        assert_eq!(side, atlas.uv(&texturepacks::block_atlas_key("grass_side")).unwrap());
        assert_eq!(bottom, atlas.uv(&texturepacks::block_atlas_key("dirt")).unwrap());
        // Different atlas tiles must not collide on the same rect.
        assert_ne!(top, side);
    }

    #[test]
    fn unmapped_block_id_falls_back_to_the_missing_tile() {
        let atlas = texturepacks::build_fallback_atlas();
        let unmapped = BlockId(9999); // no client_core constant uses this id
        let uv = face_uv(&atlas, unmapped, FaceKind::Side);
        let expected = atlas.uv(texturepacks::MISSING_TEXTURE_KEY).unwrap();
        assert_eq!(uv, expected);
    }

    #[test]
    fn air_has_no_face_mapping() {
        assert!(block_faces(BlockId::AIR).is_none());
    }
}
