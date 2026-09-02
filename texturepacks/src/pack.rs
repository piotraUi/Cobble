//! Reads and validates a resource pack's `pack.mcmeta`. 1.8.x packs
//! should declare `pack_format: 1`; anything else is surfaced as a
//! warning rather than a hard failure, since some community packs get
//! this wrong but still work fine in-game.

use std::io::{Read, Seek};

use serde::Deserialize;

use crate::error::{Result, TexturePackError};

/// The 1.8.x `pack_format` value ("Format 1", used by 1.6-1.8.9 packs;
/// see <https://minecraft.wiki/w/Pack_format>).
pub const EXPECTED_PACK_FORMAT_1_8: i64 = 1;

#[derive(Debug, Deserialize)]
struct PackMetaFile {
    pack: PackSection,
}

#[derive(Debug, Deserialize)]
struct PackSection {
    pack_format: i64,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Clone)]
pub struct PackMeta {
    pub pack_format: i64,
    pub description: String,
}

impl PackMeta {
    pub fn matches_1_8(&self) -> bool {
        self.pack_format == EXPECTED_PACK_FORMAT_1_8
    }
}

pub fn read_pack_meta<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<PackMeta> {
    let mut entry = archive
        .by_name("pack.mcmeta")
        .map_err(|_| TexturePackError::InvalidPackMeta("no pack.mcmeta in archive".into()))?;
    let mut contents = String::new();
    entry.read_to_string(&mut contents)?;
    drop(entry);

    let parsed: PackMetaFile = serde_json::from_str(&contents)
        .map_err(|e| TexturePackError::InvalidPackMeta(format!("failed to parse pack.mcmeta: {e}")))?;

    Ok(PackMeta {
        pack_format: parsed.pack.pack_format,
        description: parsed.pack.description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    fn zip_with_mcmeta(json: &str) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            writer
                .start_file("pack.mcmeta", FileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, json.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        zip::ZipArchive::new(Cursor::new(buf)).unwrap()
    }

    #[test]
    fn parses_valid_pack_format_1() {
        let mut archive = zip_with_mcmeta(r#"{"pack":{"pack_format":1,"description":"A pack"}}"#);
        let meta = read_pack_meta(&mut archive).unwrap();
        assert_eq!(meta.pack_format, 1);
        assert_eq!(meta.description, "A pack");
        assert!(meta.matches_1_8());
    }

    #[test]
    fn flags_mismatched_pack_format_without_erroring() {
        let mut archive = zip_with_mcmeta(r#"{"pack":{"pack_format":3,"description":"newer pack"}}"#);
        let meta = read_pack_meta(&mut archive).unwrap();
        assert_eq!(meta.pack_format, 3);
        assert!(!meta.matches_1_8());
    }

    #[test]
    fn missing_mcmeta_is_an_error() {
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            writer.finish().unwrap();
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(buf)).unwrap();
        assert!(read_pack_meta(&mut archive).is_err());
    }
}
