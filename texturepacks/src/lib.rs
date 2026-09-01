//! Modrinth resource pack integration: search, download, SHA-1 cache,
//! `pack.mcmeta`/`pack_format` validation, and coverage checking against
//! the known 1.8.9 block/item texture list, with original (non-Mojang)
//! fallback textures for anything missing.
//!
//! Not implemented yet — this is roadmap step 4.

/// Base URL for the Modrinth API v2, used once the search/download
/// client lands.
pub const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";
