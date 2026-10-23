use tauri::State;

use crate::error::LauncherError;
use crate::skins::{self, TextureEntry};
use crate::state::AppState;

async fn microsoft_token(state: &AppState, account_id: &str) -> Result<String, LauncherError> {
    let account = state
        .cache
        .get_account(account_id)
        .await?
        .ok_or_else(|| LauncherError::Message("account not found".into()))?;
    if account.kind != "microsoft" {
        return Err(LauncherError::Message(
            "skin apply needs a Microsoft account".into(),
        ));
    }
    if account.access_token.trim().is_empty() || account.access_token == "0" {
        return Err(LauncherError::Message(
            "Microsoft session missing — sign in again".into(),
        ));
    }
    Ok(account.access_token)
}

#[tauri::command]
pub fn list_skins(state: State<'_, AppState>) -> Result<Vec<TextureEntry>, LauncherError> {
    skins::list_textures(&state.paths.root, "skins")
}

#[tauri::command]
pub fn save_skin(
    state: State<'_, AppState>,
    name: String,
    data: String,
    slim: bool,
) -> Result<Vec<TextureEntry>, LauncherError> {
    skins::save_texture(&state.paths.root, "skins", &name, &data, slim)
}

#[tauri::command]
pub fn delete_skin(
    state: State<'_, AppState>,
    file: String,
) -> Result<Vec<TextureEntry>, LauncherError> {
    skins::delete_texture(&state.paths.root, "skins", &file)
}

#[tauri::command]
pub fn set_skin_slim(
    state: State<'_, AppState>,
    file: String,
    slim: bool,
) -> Result<Vec<TextureEntry>, LauncherError> {
    skins::set_texture_slim(&state.paths.root, "skins", &file, slim)
}

#[tauri::command]
pub async fn fetch_skin_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<String, LauncherError> {
    let http = state.http.clone();
    drop(state);
    skins::fetch_texture_url(&http, &url).await
}

#[tauri::command]
pub async fn ms_skin_profile(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<serde_json::Value, LauncherError> {
    let http = state.http.clone();
    let token = microsoft_token(&state, &account_id).await?;
    drop(state);
    skins::ms_profile(&http, &token).await
}

#[tauri::command]
pub async fn ms_apply_skin(
    state: State<'_, AppState>,
    account_id: String,
    data: String,
    slim: bool,
) -> Result<serde_json::Value, LauncherError> {
    let http = state.http.clone();
    let token = microsoft_token(&state, &account_id).await?;
    drop(state);
    skins::ms_upload_skin(&http, &token, &data, slim).await
}

#[tauri::command]
pub async fn ms_set_cape(
    state: State<'_, AppState>,
    account_id: String,
    cape_id: String,
) -> Result<serde_json::Value, LauncherError> {
    let http = state.http.clone();
    let token = microsoft_token(&state, &account_id).await?;
    drop(state);
    skins::ms_set_cape(&http, &token, &cape_id).await
}

#[tauri::command]
pub async fn ms_reset_skin(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<serde_json::Value, LauncherError> {
    let http = state.http.clone();
    let token = microsoft_token(&state, &account_id).await?;
    drop(state);
    skins::ms_reset_skin(&http, &token).await
}
