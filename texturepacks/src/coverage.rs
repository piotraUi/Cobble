//! Checks how much of `known_textures`'s list a pack's zip actually
//! provides, so the texture pack picker can show something like
//! "187/240 textures — the rest will use built-in fallbacks".

use std::collections::HashSet;
use std::io::{Read, Seek};

use crate::error::Result;
use crate::known_textures::{BLOCK_TEXTURES, ITEM_TEXTURES};

#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub found: usize,
    pub total: usize,
    pub missing_blocks: Vec<String>,
    pub missing_items: Vec<String>,
}

impl CoverageReport {
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            return 100.0;
        }
        (self.found as f32 / self.total as f32) * 100.0
    }
}

fn category_path(category: &str, name: &str) -> String {
    format!("assets/minecraft/textures/{category}/{name}.png")
}

/// `entries` is every file path present in the pack's zip (e.g. from
/// `ZipArchive::file_names()`). Matches by suffix rather than an exact
/// prefix so a pack zipped with an extra top-level folder still counts.
fn check_coverage(entries: &HashSet<String>) -> CoverageReport {
    let mut missing_blocks = Vec::new();
    let mut missing_items = Vec::new();
    let mut found = 0usize;

    for &name in BLOCK_TEXTURES {
        let path = category_path("blocks", name);
        if entries.iter().any(|e| e.ends_with(&path)) {
            found += 1;
        } else {
            missing_blocks.push(name.to_string());
        }
    }
    for &name in ITEM_TEXTURES {
        let path = category_path("items", name);
        if entries.iter().any(|e| e.ends_with(&path)) {
            found += 1;
        } else {
            missing_items.push(name.to_string());
        }
    }

    CoverageReport {
        found,
        total: BLOCK_TEXTURES.len() + ITEM_TEXTURES.len(),
        missing_blocks,
        missing_items,
    }
}

pub fn check_archive_coverage<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<CoverageReport> {
    let entries: HashSet<String> = archive.file_names().map(|s| s.to_string()).collect();
    Ok(check_coverage(&entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_coverage_when_every_known_texture_present() {
        let mut entries = HashSet::new();
        for &name in BLOCK_TEXTURES {
            entries.insert(category_path("blocks", name));
        }
        for &name in ITEM_TEXTURES {
            entries.insert(category_path("items", name));
        }

        let report = check_coverage(&entries);
        assert_eq!(report.found, report.total);
        assert!(report.missing_blocks.is_empty());
        assert!(report.missing_items.is_empty());
        assert!((report.percentage() - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn partial_coverage_reports_exact_missing_names() {
        let mut entries = HashSet::new();
        entries.insert(category_path("blocks", "stone"));
        entries.insert(category_path("items", "apple"));

        let report = check_coverage(&entries);
        assert_eq!(report.found, 2);
        assert_eq!(report.total, BLOCK_TEXTURES.len() + ITEM_TEXTURES.len());
        assert!(!report.missing_blocks.contains(&"stone".to_string()));
        assert!(report.missing_blocks.contains(&"dirt".to_string()));
        assert!(!report.missing_items.contains(&"apple".to_string()));
    }

    #[test]
    fn matches_even_with_an_extra_top_level_folder_in_the_zip() {
        let mut entries = HashSet::new();
        entries.insert(format!("MyPack-master/{}", category_path("blocks", "stone")));

        let report = check_coverage(&entries);
        assert!(!report.missing_blocks.contains(&"stone".to_string()));
    }

    #[test]
    fn empty_pack_has_zero_coverage() {
        let report = check_coverage(&HashSet::new());
        assert_eq!(report.found, 0);
        assert_eq!(report.percentage(), 0.0);
    }
}
