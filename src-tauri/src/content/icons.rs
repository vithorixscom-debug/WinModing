use base64::Engine;
use std::path::Path;

use crate::error::{io_at, LauncherError};

use super::{read_options, write_options};

const DEFAULT_INSTANCE_ICONS: &[&[u8]] = &[
    include_bytes!("../../instance-icons/01.png"),
    include_bytes!("../../instance-icons/02.png"),
    include_bytes!("../../instance-icons/03.png"),
    include_bytes!("../../instance-icons/04.png"),
    include_bytes!("../../instance-icons/05.png"),
    include_bytes!("../../instance-icons/06.png"),
    include_bytes!("../../instance-icons/07.png"),
    include_bytes!("../../instance-icons/08.png"),
];

pub fn file_data_url(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let mime = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn attach_icon_data(root: &Path, extra: &mut serde_json::Value) {
    let Some(map) = extra.as_object_mut() else {
        return;
    };
    let stored = map
        .get("iconPath")
        .or_else(|| map.get("icon_path"))
        .and_then(|value| value.as_str())
        .map(std::path::PathBuf::from);
    let local = root.join("icon.png");
    let path = if local.is_file() {
        local
    } else if let Some(item) = stored.filter(|item| item.is_file()) {
        item
    } else {
        return;
    };
    if let Some(url) = file_data_url(&path) {
        map.insert("iconDataUrl".into(), serde_json::Value::String(url));
        map.insert(
            "iconPath".into(),
            serde_json::Value::String(path.display().to_string()),
        );
    }
}

#[allow(dead_code)]
pub fn assign_random_icon(root: &Path) -> Result<String, LauncherError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos() as usize)
        .unwrap_or(0);
    let bytes = DEFAULT_INSTANCE_ICONS[stamp % DEFAULT_INSTANCE_ICONS.len()];
    let dest = root.join("icon.png");
    std::fs::write(&dest, bytes).map_err(|err| io_at(&dest, err))?;
    let mut extra = read_options(root);
    if let Some(map) = extra.as_object_mut() {
        map.insert(
            "iconPath".into(),
            serde_json::Value::String(dest.display().to_string()),
        );
    }
    write_options(root, &extra)?;
    Ok(dest.display().to_string())
}
