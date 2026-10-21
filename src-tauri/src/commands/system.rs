use std::path::{Path, PathBuf};

use serde::Serialize;
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryIcon {
    pub path: String,
    pub data_url: String,
}

fn push_gallery_dir(dir: &Path, out: &mut Vec<GalleryIcon>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
            continue;
        }
        let Some(data_url) = crate::content::file_data_url(&path) else {
            continue;
        };
        out.push(GalleryIcon {
            path: path.display().to_string(),
            data_url,
        });
    }
}

#[tauri::command]
pub fn list_icon_gallery(state: State<'_, AppState>) -> Vec<GalleryIcon> {
    let mut out = Vec::new();
    let mut dirs = vec![
        state.paths.root.join("icon-gallery"),
        state.paths.root.join("pack-icons"),
    ];
    if let Some(local) = dirs::data_local_dir() {
        let extra = local.join("WinModing").join("icon-gallery");
        if extra != dirs[0] {
            dirs.push(extra);
        }
    }
    for dir in dirs {
        push_gallery_dir(&dir, &mut out);
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out.truncate(80);
    out
}

#[tauri::command]
pub fn read_image_data_url(path: String) -> Result<String, LauncherError> {
    let path = PathBuf::from(path);
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err(LauncherError::Message("unsupported image type".into()));
    }
    crate::content::file_data_url(&path)
        .ok_or_else(|| LauncherError::Message("could not read image".into()))
}
