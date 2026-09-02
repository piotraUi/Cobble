//! Local download cache for texture pack `.zip` files, keyed by SHA-1
//! so re-selecting an already-downloaded pack (or version) never
//! re-fetches it. Where that cache should actually live is platform-
//! specific — `dirs::home_dir()` works for desktop but returns `None`
//! on Android (no `$HOME`/passwd-entry concept there; apps get a
//! sandboxed data directory instead, which the host app resolves via
//! `android_activity::AndroidApp::internal_data_path()` — see
//! `app-android`) — so callers pass in the root explicitly rather than
//! this module guessing.

use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::error::{Result, TexturePackError};
use crate::modrinth::VersionFile;

/// The desktop-friendly default cache root (`~/.cobble/texturepacks/`).
/// Not meaningful on Android — see the module doc comment.
pub fn default_cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(TexturePackError::NoCacheDir)?;
    Ok(home.join(".cobble").join("texturepacks"))
}

fn cached_zip_path(cache_dir: &Path, slug: &str, sha1_hex: &str) -> PathBuf {
    // Short hash prefix keeps filenames readable while still being
    // unique per content — collisions are astronomically unlikely and
    // harmless anyway (worst case: an unnecessary re-download).
    let short_hash = &sha1_hex[..sha1_hex.len().min(12)];
    cache_dir.join(format!("{slug}-{short_hash}.zip"))
}

pub fn hex_sha1(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Downloads `file` into `cache_root` if it isn't already there
/// (identified by its Modrinth-reported SHA-1), verifying the download
/// against that hash, and returns the local path either way.
pub async fn download_and_cache(
    client: &reqwest::Client,
    slug: &str,
    file: &VersionFile,
    cache_root: &Path,
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(cache_root).await?;

    let expected_sha1 = file.hashes.sha1.as_deref();
    let cache_key = expected_sha1.unwrap_or(&file.filename);
    let path = cached_zip_path(cache_root, slug, cache_key);

    if path.exists() {
        log::info!("using cached texture pack: {}", path.display());
        return Ok(path);
    }

    log::info!("downloading texture pack {slug} from {}", file.url);
    let bytes = client.get(&file.url).send().await?.error_for_status()?.bytes().await?;

    if let Some(expected) = expected_sha1 {
        let actual = hex_sha1(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(TexturePackError::HashMismatch {
                expected: expected.to_string(),
                actual,
            });
        }
    }

    let tmp_path = path.with_extension("zip.part");
    {
        let mut tmp_file = tokio::fs::File::create(&tmp_path).await?;
        tmp_file.write_all(&bytes).await?;
        tmp_file.flush().await?;
    }
    tokio::fs::rename(&tmp_path, &path).await?;

    Ok(path)
}

/// Tiny local hex-encoding helper so we don't need a whole extra crate
/// just for `Sha1::finalize()` -> hex string.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(bytes.as_ref().len() * 2);
        for byte in bytes.as_ref() {
            write!(out, "{byte:02x}").unwrap();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_sha1_matches_known_vector() {
        // echo -n "abc" | sha1sum
        assert_eq!(hex_sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn cached_zip_path_is_stable_for_the_same_hash() {
        let dir = PathBuf::from("/tmp/cobble-test");
        let a = cached_zip_path(&dir, "faithful", "abcdef0123456789");
        let b = cached_zip_path(&dir, "faithful", "abcdef0123456789");
        assert_eq!(a, b);
        assert!(a.to_string_lossy().contains("faithful"));
    }
}
