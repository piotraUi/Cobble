//! End-to-end check against the real Modrinth API: search, pick a
//! version, download+cache, validate `pack.mcmeta`, and build an
//! atlas. Marked `#[ignore]` since it needs network access and hits a
//! third party service — run explicitly with:
//!
//!   cargo test -p texturepacks --test live_modrinth -- --ignored --nocapture

#[tokio::test]
#[ignore]
async fn search_download_and_load_a_real_1_8_9_pack() {
    let client = texturepacks::modrinth::build_client().expect("build client");

    let search = texturepacks::modrinth::search_resourcepacks(&client, texturepacks::GAME_VERSION, 5)
        .await
        .expect("search modrinth");
    assert!(!search.hits.is_empty(), "expected at least one 1.8.9 resourcepack hit");

    let hit = &search.hits[0];
    println!("top hit: {} ({})", hit.title, hit.slug);

    let versions = texturepacks::modrinth::get_versions(&client, &hit.project_id, texturepacks::GAME_VERSION)
        .await
        .expect("list versions");
    let version = versions.first().expect("project should have at least one 1.8.9 version");
    let file = version.zip_file().expect("version should have a zip file");

    let (path, loaded) = texturepacks::download_and_load(&client, &hit.slug, file)
        .await
        .expect("download and load pack");

    println!(
        "loaded {} from {}: pack_format={} ({}), coverage={}/{} ({:.1}%)",
        hit.title,
        path.display(),
        loaded.meta.pack_format,
        if loaded.meta.matches_1_8() { "matches 1.8" } else { "MISMATCHED" },
        loaded.coverage.found,
        loaded.coverage.total,
        loaded.coverage.percentage(),
    );

    assert!(loaded.atlas.size() > 0);

    // Downloading again should hit the cache instead of the network.
    let (cached_path, _) = texturepacks::download_and_load(&client, &hit.slug, file)
        .await
        .expect("second download should reuse cache");
    assert_eq!(path, cached_path);
}
