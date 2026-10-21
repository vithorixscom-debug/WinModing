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
