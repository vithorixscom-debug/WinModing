use tauri::{AppHandle, State};

use crate::auth;
use crate::domain::AccountView;
use crate::error::LauncherError;
use crate::state::AppState;

#[tauri::command]
pub async fn login_offline(
    state: State<'_, AppState>,
    username: String,
) -> Result<AccountView, LauncherError> {
    let account = auth::offline_account(&username)?;
    let view = account.view();
    let cache = state.cache.clone();
    drop(state);
    cache.upsert_account(account).await?;
    Ok(view)
}

/// Microsoft login via device code in the system browser only.
/// Embedded WebView fallback is disabled — it breaks on RU IPs (no CSS / blocked assets).
#[tauri::command]
pub async fn login_msa(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AccountView, LauncherError> {
    let http = state.http.clone();
    let cache = state.cache.clone();
    drop(state);
    let account = auth::login_microsoft_device(&app, &http).await?;
    let view = account.view();
    cache.upsert_account(account).await?;
    Ok(view)
}

#[tauri::command]
pub async fn login_ely(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<AccountView, LauncherError> {
    let http = state.http.clone();
    let cache = state.cache.clone();
    drop(state);
    let account = auth::login_ely(&http, &username, &password).await?;
    let view = account.view();
    cache.upsert_account(account).await?;
    Ok(view)
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountView>, LauncherError> {
    let cache = state.cache.clone();
    drop(state);
    cache.list_account_views().await
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Result<(), LauncherError> {
    let cache = state.cache.clone();
    drop(state);
    cache.delete_account(&id).await
}
