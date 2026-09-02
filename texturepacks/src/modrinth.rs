//! Minimal Modrinth API v2 client: searching for resource packs
//! compatible with a given Minecraft version, and listing a project's
//! versions to find its downloadable `.zip`.
//!
//! See <https://docs.modrinth.com/api/>. Modrinth asks that API
//! clients identify themselves with a descriptive `User-Agent`
//! (project name + contact info), which `build_client` sets.

use serde::Deserialize;

use crate::error::Result;

pub const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";

pub fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("Cobble/0.1.0 (https://github.com/piotraUi/Cobble; Minecraft 1.8.9 client)")
        .build()?)
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub hashes: FileHashes,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    pub files: Vec<VersionFile>,
}

impl Version {
    /// The version's primary `.zip` file, or the first `.zip` file if
    /// none is marked primary — resource pack versions should always
    /// have exactly one download.
    pub fn zip_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary && f.filename.ends_with(".zip"))
            .or_else(|| self.files.iter().find(|f| f.filename.ends_with(".zip")))
    }
}

/// Searches Modrinth for resource packs compatible with `game_version`,
/// sorted by downloads (most popular first).
pub async fn search_resourcepacks(
    client: &reqwest::Client,
    game_version: &str,
    limit: u32,
) -> Result<SearchResponse> {
    let facets = format!(r#"[["project_type:resourcepack"],["versions:{game_version}"]]"#);
    let response = client
        .get(format!("{MODRINTH_API_BASE}/search"))
        .query(&[
            ("facets", facets.as_str()),
            ("index", "downloads"),
            ("limit", &limit.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

/// Lists a project's versions compatible with `game_version`, newest first
/// (Modrinth's default order).
pub async fn get_versions(
    client: &reqwest::Client,
    project_id: &str,
    game_version: &str,
) -> Result<Vec<Version>> {
    let game_versions = format!(r#"["{game_version}"]"#);
    let response = client
        .get(format!("{MODRINTH_API_BASE}/project/{project_id}/version"))
        .query(&[("game_versions", game_versions.as_str())])
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}
