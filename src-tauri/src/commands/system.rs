use tauri::State;

use crate::error::LauncherError;
use crate::state::AppState;

#[tauri::command]
pub fn drag_window(window: tauri::WebviewWindow) {
    crate::window::start_os_drag(&window);
}

#[tauri::command]
pub fn open_external(url: String) -> Result<(), LauncherError> {
    crate::window::open_with_os(&url)
}

#[tauri::command]
pub fn list_icon_gallery(state: State<'_, AppState>) -> Vec<String> {
    let mut out = Vec::new();
    let dirs = [
        state.paths.root.join("icon-gallery"),
        state.paths.root.join("pack-icons"),
    ];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
                out.push(path.display().to_string());
            }
        }
    }
    out.sort();
    out.truncate(80);
    out
}
