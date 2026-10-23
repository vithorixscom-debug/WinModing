use std::path::{Path, PathBuf};

use base64::Engine;
use md5::{Digest, Md5};
use serde::Serialize;
use tauri::State;

use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::protocol::assert_trusted_https;
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

fn safe_player_name(username: &str) -> Option<String> {
    let name = username.trim();
    if name.is_empty() || name.len() > 16 {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(name.to_string())
}

fn normalize_uuid(raw: &str) -> Option<String> {
    let hex: String = raw.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    Some(hex.to_ascii_lowercase())
}

fn face_urls(name: &str, uuid: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();
    // Millida head renderer by nick — same source that used to work for offline too.
    urls.push(format!(
        "https://api.millida.net/v2/heads/avatar/{name}?size=64"
    ));
    urls.push(format!(
        "https://api.millida.net/v2/heads/avatar/{name}?size=128"
    ));
    // Full skin → we crop locally (better than a wrong default Steve stub).
    urls.push(format!("https://api.millida.net/v2/heads/skin/{name}"));
    if let Some(id) = uuid {
        urls.push(format!("https://mc-heads.net/avatar/{id}/64"));
        urls.push(format!("https://crafthead.net/avatar/{id}/64"));
        urls.push(format!(
            "https://crafatar.com/avatars/{id}?size=64&overlay=true"
        ));
        urls.push(format!("https://minotar.net/helm/{id}/64"));
    }
    urls.push(format!("https://mc-heads.net/avatar/{name}/64"));
    urls.push(format!("https://crafthead.net/avatar/{name}/64"));
    urls.push(format!("https://minotar.net/helm/{name}/64"));
    urls.push(format!(
        "https://crafatar.com/avatars/{name}?size=64&overlay=true"
    ));
    // RF-friendly relay for hosts that are often blocked.
    let mut with_relay = Vec::new();
    for url in &urls {
        if url.contains("api.millida.net") {
            with_relay.push(url.clone());
            continue;
        }
        with_relay.push(format!(
            "https://api.millida.net/v2/launcher/dl?url={}",
            urlencoding::encode(url)
        ));
        with_relay.push(url.clone());
    }
    with_relay
}

async fn fetch_one_face(http: &HttpPool, url: &str) -> Option<Vec<u8>> {
    let bytes = http.get_bytes(url).await.ok()?;
    if bytes.len() <= 64 || bytes.len() >= 2 * 1024 * 1024 {
        return None;
    }
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    // Millida /heads/skin returns a full skin texture — crop the head.
    if url.contains("/heads/skin/") {
        return crate::facegen::face_from_skin_png(&bytes);
    }
    Some(bytes)
}

async fn fetch_face_bytes(http: &HttpPool, urls: &[String]) -> Option<Vec<u8>> {
    // Sequential: first hit wins. Parallel races often returned a default Steve
    // stub from a slower mirror before the real Millida head arrived.
    for url in urls {
        if let Some(bytes) = fetch_one_face(http, url).await {
            return Some(bytes);
        }
    }
    None
}

fn face_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn write_avatar_cache(cache_dir: &Path, cache_key: &str, bytes: &[u8]) {
    if bytes.len() < 64 {
        return;
    }
    let _ = std::fs::create_dir_all(cache_dir);
    let _ = std::fs::write(cache_dir.join(format!("{cache_key}.png")), bytes);
    // Marker so we never treat a procedural stub as a permanent head.
    let _ = std::fs::write(cache_dir.join(format!("{cache_key}.real")), b"1");
}

fn cached_real_face(cache_dir: &Path, cache_key: &str) -> Option<String> {
    let cache = cache_dir.join(format!("{cache_key}.png"));
    let marker = cache_dir.join(format!("{cache_key}.real"));
    if !cache.is_file() || !marker.is_file() {
        return None;
    }
    if std::fs::metadata(&cache).map(|m| m.len() < 64).unwrap_or(true) {
        return None;
    }
    crate::content::file_data_url(&cache)
}

/// Only use a wardrobe skin that is clearly tied to this player (ms-<uuid> / name).
fn face_from_local_wardrobe(root: &Path, name: &str, uuid: Option<&str>) -> Option<Vec<u8>> {
    let skins = crate::skins::list_textures(root, "skins").ok()?;
    let name_l = name.to_ascii_lowercase();
    let uuid_l = uuid.map(|u| u.to_ascii_lowercase()).unwrap_or_default();
    for entry in skins {
        let file_l = entry.file.to_ascii_lowercase();
        let title_l = entry.name.to_ascii_lowercase();
        let owned = (!uuid_l.is_empty() && file_l.contains(&uuid_l[..uuid_l.len().min(8)]))
            || title_l == name_l
            || file_l.starts_with(&format!("{name_l}-"))
            || file_l.starts_with(&format!("ms-{name_l}"));
        if !owned {
            continue;
        }
        let raw = entry.data.rsplit(',').next().unwrap_or(&entry.data);
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(raw.trim()) else {
            continue;
        };
        if let Some(face) = crate::facegen::face_from_skin_png(&bytes) {
            return Some(face);
        }
    }
    None
}

fn persist_ms_skin(root: &Path, uuid: &str, skin_bytes: &[u8]) {
    let data = face_data_url(skin_bytes);
    let name = format!("ms-{}", &uuid[..uuid.len().min(8)]);
    let _ = crate::skins::save_texture(root, "skins", &name, &data, false);
}

async fn download_skin_png(http: &HttpPool, skin_url: &str) -> Option<Vec<u8>> {
    let mut urls = vec![skin_url.to_string()];
    urls.push(format!(
        "https://api.millida.net/v2/launcher/dl?url={}",
        urlencoding::encode(skin_url)
    ));
    for url in urls {
        if let Ok(bytes) = http.get_bytes(&url).await {
            if bytes.starts_with(&[0x89, b'P', b'N', b'G']) && bytes.len() > 64 {
                return Some(bytes);
            }
        }
    }
    None
}

async fn face_from_microsoft_session(
    http: &HttpPool,
    root: &Path,
    token: &str,
    uuid_hint: Option<&str>,
) -> Option<Vec<u8>> {
    let profile = crate::skins::ms_profile(http, token).await.ok()?;
    let skin_url = profile.get("skin")?.as_str()?.trim();
    if skin_url.is_empty() {
        return None;
    }
    let skin_bytes = download_skin_png(http, skin_url).await?;
    let uuid = uuid_hint
        .map(str::to_string)
        .or_else(|| {
            profile
                .get("uuid")
                .and_then(|v| v.as_str())
                .and_then(normalize_uuid)
        })
        .unwrap_or_else(|| "player".into());
    persist_ms_skin(root, &uuid, &skin_bytes);
    crate::facegen::face_from_skin_png(&skin_bytes)
}

fn decode_skin_data_url(data: &str) -> Option<Vec<u8>> {
    let raw = data.rsplit(',').next().unwrap_or(data);
    base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .ok()
        .filter(|b| b.starts_with(&[0x89, b'P', b'N', b'G']) && b.len() > 64)
}

/// Persist a head cropped from a skin the UI already loaded (Skins editor / MS profile).
#[tauri::command]
pub fn player_face_remember(
    state: State<'_, AppState>,
    username: String,
    uuid: Option<String>,
    skin_data: String,
) -> Result<(), LauncherError> {
    let Some(name) = safe_player_name(&username).or_else(|| {
        let fallback = username.trim();
        if fallback.is_empty() {
            None
        } else {
            Some(fallback.chars().take(16).collect())
        }
    }) else {
        return Ok(());
    };
    let uuid = uuid.as_deref().and_then(normalize_uuid);
    let cache_key = uuid
        .clone()
        .unwrap_or_else(|| name.to_ascii_lowercase());
    let Some(skin_bytes) = decode_skin_data_url(&skin_data) else {
        return Ok(());
    };
    let Some(face) = crate::facegen::face_from_skin_png(&skin_bytes) else {
        return Ok(());
    };
    persist_ms_skin(&state.paths.root, &cache_key, &skin_bytes);
    write_avatar_cache(&state.paths.root.join("avatars"), &cache_key, &face);
    Ok(())
}

#[tauri::command]
pub async fn player_face(
    state: State<'_, AppState>,
    username: String,
    uuid: Option<String>,
) -> Result<String, LauncherError> {
    let Some(name) = safe_player_name(&username) else {
        return Ok(String::new());
    };
    let uuid = uuid.as_deref().and_then(normalize_uuid);
    let cache_key = uuid
        .clone()
        .unwrap_or_else(|| name.to_ascii_lowercase());
    let root = state.paths.root.clone();
    let cache_dir = root.join("avatars");

    // 1) Real head already saved for THIS nick/uuid.
    if let Some(url) = cached_real_face(&cache_dir, &cache_key) {
        return Ok(url);
    }

    // 2) Wardrobe skin that belongs to this player only (never a random shared skin).
    if let Some(face) = face_from_local_wardrobe(&root, &name, uuid.as_deref()) {
        write_avatar_cache(&cache_dir, &cache_key, &face);
        return Ok(face_data_url(&face));
    }

    let http = state.http.clone();
    let accounts = state.cache.list_accounts().await.unwrap_or_default();
    drop(state);

    // 3) Matching Microsoft account only → official skin URL → crop + remember.
    let mut bytes = None;
    for account in &accounts {
        let same_name = account.username.eq_ignore_ascii_case(&name);
        let same_uuid = uuid
            .as_ref()
            .map(|id| normalize_uuid(&account.uuid).as_ref() == Some(id))
            .unwrap_or(false);
        if account.kind != "microsoft" || !(same_name || same_uuid) {
            continue;
        }
        if account.access_token.len() < 32 || account.access_token == "0" {
            continue;
        }
        let hint = normalize_uuid(&account.uuid);
        bytes =
            face_from_microsoft_session(&http, &root, &account.access_token, hint.as_deref()).await;
        if bytes.is_none() {
            if let Ok(fresh) = http.refresh_proxies() {
                bytes = face_from_microsoft_session(
                    &fresh,
                    &root,
                    &account.access_token,
                    hint.as_deref(),
                )
                .await;
            }
        }
        if bytes.is_some() {
            break;
        }
    }

    // 4) By nickname for everyone (microsoft miss, offline, ely) — Millida heads / CDN.
    if bytes.is_none() {
        let urls = face_urls(&name, uuid.as_deref());
        bytes = fetch_face_bytes(&http, &urls).await;
        if bytes.is_none() {
            if let Ok(fresh) = http.refresh_proxies() {
                bytes = fetch_face_bytes(&fresh, &urls).await;
            }
        }
    }

    if let Some(face) = bytes {
        write_avatar_cache(&cache_dir, &cache_key, &face);
        return Ok(face_data_url(&face));
    }

    // Last resort — unique local stub, never written as a "real" avatar.
    let seed = uuid.unwrap_or_else(|| name.to_ascii_lowercase());
    let stub = crate::facegen::procedural_face_png(&seed);
    if stub.len() < 64 {
        return Ok(String::new());
    }
    Ok(face_data_url(&stub))
}

fn media_cache_name(url: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(url.as_bytes());
    format!("{:x}.bin", hasher.finalize())
}

fn guess_media_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

/// Fetch catalog icons / media through the same RF-aware pool (forgecdn → Millida dl).
#[tauri::command]
pub async fn proxy_media(
    state: State<'_, AppState>,
    url: String,
) -> Result<String, LauncherError> {
    let raw = url.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    if raw.starts_with("data:image") {
        return Ok(raw.to_string());
    }
    assert_trusted_https(raw)?;
    let cache_dir = state.paths.root.join("media-cache");
    let cache = cache_dir.join(media_cache_name(raw));
    if cache.is_file() {
        if let Ok(bytes) = std::fs::read(&cache) {
            if bytes.len() > 32 && bytes.len() < 4 * 1024 * 1024 {
                let mime = guess_media_mime(&bytes);
                return Ok(format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ));
            }
        }
    }
    let http = state.http.clone();
    drop(state);
    let bytes = http.get_bytes(raw).await?;
    if bytes.len() <= 32 || bytes.len() >= 4 * 1024 * 1024 {
        return Ok(String::new());
    }
    let _ = std::fs::create_dir_all(&cache_dir);
    let _ = std::fs::write(&cache, &bytes);
    let mime = guess_media_mime(&bytes);
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub async fn list_recommended_servers(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    sort: Option<String>,
    category: Option<String>,
    version: Option<String>,
) -> Result<crate::servers_catalog::ServersPage, LauncherError> {
    let http = state.http.clone();
    drop(state);
    crate::servers_catalog::list_recommended_servers(
        &http,
        query.unwrap_or_default(),
        limit,
        offset,
        sort,
        category,
        version,
    )
    .await
}

#[tauri::command]
pub async fn server_status(
    _state: State<'_, AppState>,
    address: String,
) -> Result<serde_json::Value, LauncherError> {
    let addr = address.trim().to_string();
    if addr.is_empty() || addr.len() > 128 {
        return Ok(serde_json::json!({ "online": false }));
    }
    let safe: String = addr
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | ':' | '_'))
        .collect();
    if safe.is_empty() || safe != addr {
        return Ok(serde_json::json!({ "online": false }));
    }
    // Native SLP — mcsrvstat.us / mcstatus.io are often blocked in RF.
    Ok(crate::server_ping::ping_java(&safe).await)
}
