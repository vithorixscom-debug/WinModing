use tauri::State;

use crate::domain::{CatalogPage, CatalogQuery};
use crate::error::LauncherError;
use crate::providers;
use crate::state::AppState;

#[tauri::command]
pub async fn search_catalog(
    state: State<'_, AppState>,
    query: CatalogQuery,
) -> Result<CatalogPage, LauncherError> {
    let http = state.http.clone();
    let cache = state.cache.clone();
    let curseforge_key = state.paths.read_curseforge_key();
    drop(state);
    providers::search_unified(&http, &cache, &query, &curseforge_key).await
}

#[tauri::command]
pub async fn get_catalog_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, LauncherError> {
    let key = state.paths.read_curseforge_key();
    crate::content::catalog_project(&state.http, &id, &key).await
}

#[tauri::command]
pub async fn get_catalog_version(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, LauncherError> {
    let id = id.trim();
    let url = format!(
        "{}/version/{}",
        crate::protocol::MODRINTH_API,
        urlencoding::encode(id)
    );
    state.http.get_json(&url, &[]).await
}

#[tauri::command]
pub async fn catalog_get_json(
    state: State<'_, AppState>,
    url: String,
) -> Result<serde_json::Value, LauncherError> {
    crate::protocol::assert_trusted_https(&url)?;
    state.http.get_json(&url, &[]).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCatalogRequest {
    pub project_id: String,
    pub version_id: Option<String>,
    pub source: String,
    pub project_type: Option<String>,
}

#[tauri::command]
pub async fn save_catalog_download(
    state: State<'_, AppState>,
    request: SaveCatalogRequest,
) -> Result<crate::content::SavedDownload, LauncherError> {
    let key = state.paths.read_curseforge_key();
    crate::content::save_catalog_download(
        &state.http,
        &state.paths,
        &request.project_id,
        request.version_id.as_deref(),
        &request.source,
        request.project_type.as_deref().unwrap_or("mod"),
        &key,
    )
    .await
}

#[tauri::command]
pub fn list_saved_catalog(state: State<'_, AppState>) -> Vec<String> {
    crate::content::list_saved_project_ids(&state.paths)
}

#[tauri::command]
pub fn open_downloads_folder(state: State<'_, AppState>) -> Result<String, LauncherError> {
    let folder = state.paths.downloads_dir();
    std::fs::create_dir_all(&folder).map_err(|err| crate::error::io_at(&folder, err))?;
    crate::window::open_with_os(&folder.display().to_string())?;
    Ok(folder.display().to_string())
}

#[tauri::command]
pub async fn list_project_versions(
    state: State<'_, AppState>,
    id: String,
    source: String,
    game_version: String,
    loader: String,
) -> Result<Vec<serde_json::Value>, LauncherError> {
    let key = state.paths.read_curseforge_key();
    crate::content::catalog_versions(
        &state.http,
        &state.cache,
        &id,
        &source,
        &game_version,
        &loader,
        &key,
    )
    .await
}
