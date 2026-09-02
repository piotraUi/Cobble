//! Modrinth resource pack integration: search, download with a SHA-1
//! content cache, `pack.mcmeta`/`pack_format` validation, coverage
//! checking against a known 1.8.9 texture list, and packing whatever's
//! present (plus original, non-Mojang fallback tiles for anything
//! missing) into one texture atlas.
//!
//! Texture packs are always fetched on demand at the user's explicit
//! request (see the `ui` crate's picker, roadmap step 5) — nothing
//! here bundles or redistributes Mojang assets; see `fallback` for the
//! placeholder tiles used when a pack doesn't cover something.

pub mod atlas;
pub mod cache;
pub mod coverage;
pub mod error;
pub mod fallback;
pub mod known_textures;
pub mod load;
pub mod modrinth;
pub mod pack;

pub use atlas::{AtlasRect, TextureAtlas};
pub use coverage::CoverageReport;
pub use error::{Result, TexturePackError};
pub use load::{
    block_atlas_key, build_fallback_atlas, download_and_load, item_atlas_key, load_pack_from_zip,
    LoadedPack, MISSING_TEXTURE_KEY,
};
pub use modrinth::MODRINTH_API_BASE;
pub use pack::PackMeta;

/// Minecraft version resource packs are matched against throughout
/// this crate.
pub const GAME_VERSION: &str = "1.8.9";
