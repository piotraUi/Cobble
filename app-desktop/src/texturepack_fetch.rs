//! One-shot background network calls for the texture pack picker
//! screen: search Modrinth, or download+load a chosen pack. Each call
//! spawns its own short-lived thread (same pattern as `network.rs`,
//! just without a long-running connection to keep alive) and reports
//! back over a channel so `main.rs` never blocks the render loop on it.

use texturepacks::modrinth::SearchHit;
use texturepacks::LoadedPack;
use tokio::sync::mpsc;

pub enum PickerEvent {
    SearchResults(Result<Vec<SearchHit>, String>),
    PackLoaded(Result<(String, LoadedPack), String>),
}

fn spawn<F>(task: F) -> mpsc::UnboundedReceiver<PickerEvent>
where
    F: std::future::Future<Output = PickerEvent> + Send + 'static,
{
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build texturepack-fetch runtime");
        let _ = tx.send(runtime.block_on(task));
    });
    rx
}

pub fn search() -> mpsc::UnboundedReceiver<PickerEvent> {
    spawn(async move {
        let result = async {
            let client = texturepacks::modrinth::build_client().map_err(|e| e.to_string())?;
            let response = texturepacks::modrinth::search_resourcepacks(&client, texturepacks::GAME_VERSION, 15)
                .await
                .map_err(|e| e.to_string())?;
            Ok(response.hits)
        }
        .await;
        PickerEvent::SearchResults(result)
    })
}

pub fn download_and_load(hit: SearchHit) -> mpsc::UnboundedReceiver<PickerEvent> {
    spawn(async move {
        let result = async {
            let client = texturepacks::modrinth::build_client().map_err(|e| e.to_string())?;
            let versions = texturepacks::modrinth::get_versions(&client, &hit.project_id, texturepacks::GAME_VERSION)
                .await
                .map_err(|e| e.to_string())?;
            let version = versions.first().ok_or_else(|| "pack has no 1.8.9 versions".to_string())?;
            let file = version.zip_file().ok_or_else(|| "pack version has no .zip file".to_string())?;
            let (_path, loaded) = texturepacks::download_and_load(&client, &hit.slug, file)
                .await
                .map_err(|e| e.to_string())?;
            Ok((hit.title.clone(), loaded))
        }
        .await;
        PickerEvent::PackLoaded(result)
    })
}
