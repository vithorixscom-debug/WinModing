use tauri::{AppHandle, Manager, State};

use crate::error::LauncherError;
use crate::state::AppState;

#[tauri::command]
pub async fn save_curseforge_key(
    state: State<'_, AppState>,
    key: String,
) -> Result<(), LauncherError> {
    state.paths.write_curseforge_key(&key)
}

#[tauri::command]
pub async fn curseforge_key_set(state: State<'_, AppState>) -> Result<bool, LauncherError> {
    Ok(!state.paths.read_curseforge_key().trim().is_empty())
}

#[tauri::command]
pub async fn save_microsoft_client_id(
    state: State<'_, AppState>,
    key: String,
) -> Result<(), LauncherError> {
    state.paths.write_microsoft_client_id(&key)
}

#[tauri::command]
pub async fn microsoft_client_id_set(state: State<'_, AppState>) -> Result<bool, LauncherError> {
    Ok(!state.paths.read_microsoft_client_id().trim().is_empty())
}

#[tauri::command]
pub fn get_launcher_settings(
    state: State<'_, AppState>,
) -> Result<crate::settings::LauncherSettings, LauncherError> {
    Ok(crate::settings::load(&state.paths))
}

#[tauri::command]
pub fn set_launcher_settings(
    state: State<'_, AppState>,
    settings: crate::settings::LauncherSettings,
) -> Result<(), LauncherError> {
    crate::settings::save(&state.paths, &settings)
}

#[tauri::command]
pub fn get_max_memory() -> u64 {
    crate::java::total_memory_kib()
}

#[tauri::command]
pub fn discord_set_idle() {
    crate::discord::set_idle();
}

#[tauri::command]
pub fn app_directory(state: State<'_, AppState>) -> String {
    state.paths.root.display().to_string()
}

#[tauri::command]
pub async fn show_window(app: AppHandle) -> Result<(), LauncherError> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub fn should_disable_mouseover() -> bool {
    false
}

#[tauri::command]
pub async fn pick_path(
    app: AppHandle,
    directory: bool,
    multiple: bool,
    filters: Vec<String>,
) -> Option<serde_json::Value> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = app.run_on_main_thread(move || {
        let mut dialog = rfd::FileDialog::new();
        if !filters.is_empty() {
            let names: Vec<&str> = filters.iter().map(String::as_str).collect();
            dialog = dialog.add_filter("Files", &names);
        }
        let picked = if directory {
            dialog
                .pick_folder()
                .map(|path| serde_json::Value::String(path.to_string_lossy().into_owned()))
        } else if multiple {
            dialog.pick_files().map(|paths| {
                serde_json::Value::Array(
                    paths
                        .into_iter()
                        .map(|path| serde_json::Value::String(path.to_string_lossy().into_owned()))
                        .collect(),
                )
            })
        } else {
            dialog
                .pick_file()
                .map(|path| serde_json::Value::String(path.to_string_lossy().into_owned()))
        };
        let _ = tx.send(picked);
    });
    rx.await.ok().flatten()
}
