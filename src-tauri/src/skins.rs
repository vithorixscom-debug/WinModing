//! Local skin wardrobe + Mojang Microsoft profile APIs.
//! Logic adapted from Millida's skins engine (no Millida branding / hosts).

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::LauncherError;
use crate::net::HttpPool;

const MAX_ITEMS: usize = 48;
const MAX_BYTES: usize = 2_000_000;
const MS_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const TEXTURE_HOSTS: &[&str] = &[
    "textures.minecraft.net",
    "cdn.modrinth.com",
    "launcher-files.modrinth.com",
    "crafatar.com",
    "mc-heads.net",
    "minotar.net",
];

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct TextureMeta {
    file: String,
    name: String,
    #[serde(default)]
    slim: bool,
    #[serde(default)]
    slim_manual: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextureEntry {
    pub file: String,
    pub name: String,
    pub slim: bool,
    pub slim_manual: bool,
    pub data: String,
}

fn kind_dir(root: &Path, kind: &str) -> Result<PathBuf, LauncherError> {
    let sub = match kind {
        "skins" | "capes" => kind,
        other => {
            return Err(LauncherError::Message(format!(
                "unknown texture kind: {other}"
            )))
        }
    };
    Ok(root.join("textures").join(sub))
}

fn index_path(root: &Path, kind: &str) -> Result<PathBuf, LauncherError> {
    Ok(kind_dir(root, kind)?.join("index.json"))
}

fn as_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn b64_decode(input: &str) -> Option<Vec<u8>> {
    let raw = input.rsplit(',').next().unwrap_or(input);
    base64::engine::general_purpose::STANDARD.decode(raw.trim()).ok()
}

fn read_index(root: &Path, kind: &str) -> Vec<TextureMeta> {
    index_path(root, kind)
        .ok()
        .and_then(|p| fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Vec<TextureMeta>>(&b).ok())
        .unwrap_or_default()
}

fn entry(dir: &Path, meta: &TextureMeta) -> Option<TextureEntry> {
    let bytes = fs::read(dir.join(&meta.file)).ok()?;
    Some(TextureEntry {
        file: meta.file.clone(),
        name: meta.name.clone(),
        slim: meta.slim,
        slim_manual: meta.slim_manual,
        data: as_data_url(&bytes),
    })
}

fn sync_index(root: &Path, kind: &str) -> Result<Vec<TextureMeta>, LauncherError> {
    let dir = kind_dir(root, kind)?;
    let _ = fs::create_dir_all(&dir);
    let mut metas: Vec<TextureMeta> = read_index(root, kind)
        .into_iter()
        .filter(|m| dir.join(&m.file).is_file())
        .collect();
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().ends_with(".png") || metas.iter().any(|m| m.file == name) {
                continue;
            }
            metas.push(TextureMeta {
                name: name.trim_end_matches(".png").to_string(),
                file: name,
                ..TextureMeta::default()
            });
        }
    }
    Ok(metas)
}

fn store(root: &Path, kind: &str, metas: &[TextureMeta]) -> Result<Vec<TextureEntry>, LauncherError> {
    let dir = kind_dir(root, kind)?;
    let path = index_path(root, kind)?;
    let body = serde_json::to_vec_pretty(metas)
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    fs::write(&path, body).map_err(|err| LauncherError::Message(err.to_string()))?;
    Ok(metas.iter().filter_map(|m| entry(&dir, m)).collect())
}

fn file_stem(name: &str) -> String {
    let base = name.trim().trim_end_matches(".png").trim_end_matches(".PNG");
    let safe: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let safe = safe.trim_matches('-').to_string();
    if safe.is_empty() {
        "texture".into()
    } else {
        safe.chars().take(40).collect()
    }
}

fn png_bytes(data: &str) -> Result<Vec<u8>, LauncherError> {
    let bytes = b64_decode(data).ok_or_else(|| LauncherError::Message("invalid PNG".into()))?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(LauncherError::Message("not a PNG".into()));
    }
    if bytes.len() > MAX_BYTES {
        return Err(LauncherError::Message("PNG larger than 2 MB".into()));
    }
    Ok(bytes)
}

fn safe_file_name(file: &str) -> Result<String, LauncherError> {
    let name = Path::new(file)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() || name.contains("..") || !name.to_lowercase().ends_with(".png") {
        return Err(LauncherError::Message("bad texture file name".into()));
    }
    Ok(name)
}

fn to_https(url: Option<&str>) -> serde_json::Value {
    match url {
        Some(u) => serde_json::Value::String(u.replacen("http://", "https://", 1)),
        None => serde_json::Value::Null,
    }
}

fn ms_error(status: u16, text: &str) -> LauncherError {
    let msg = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| {
            v.get("errorMessage")
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str())
                .map(String::from)
        })
        .filter(|m| !m.is_empty());
    LauncherError::Message(match msg {
        Some(m) => m,
        None if status == 401 => "Microsoft session expired — sign in again".into(),
        None => format!("Mojang responded {status}"),
    })
}

async fn ms_json(
    http: &HttpPool,
    req: reqwest::RequestBuilder,
) -> Result<serde_json::Value, LauncherError> {
    let _ = http;
    let res = req
        .send()
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ms_error(status.as_u16(), &text));
    }
    if text.trim().is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(&text).map_err(|err| LauncherError::Message(err.to_string()))
}

pub fn list_textures(root: &Path, kind: &str) -> Result<Vec<TextureEntry>, LauncherError> {
    let metas = sync_index(root, kind)?;
    if metas.is_empty() {
        return Ok(vec![]);
    }
    store(root, kind, &metas)
}

pub fn save_texture(
    root: &Path,
    kind: &str,
    name: &str,
    data: &str,
    slim: bool,
) -> Result<Vec<TextureEntry>, LauncherError> {
    let bytes = png_bytes(data)?;
    let dir = kind_dir(root, kind)?;
    fs::create_dir_all(&dir).map_err(|err| LauncherError::Message(err.to_string()))?;
    let hash = hex::encode(Sha256::digest(&bytes));
    let file = format!("{}-{}.png", file_stem(name), &hash[..8]);
    fs::write(dir.join(&file), &bytes).map_err(|err| LauncherError::Message(err.to_string()))?;

    let title = name.trim().trim_end_matches(".png").trim();
    let mut metas = sync_index(root, kind)?;
    metas.retain(|m| m.file != file);
    metas.insert(
        0,
        TextureMeta {
            file,
            name: if title.is_empty() {
                "Skin".into()
            } else {
                title.to_string()
            },
            slim,
            slim_manual: false,
        },
    );
    while metas.len() > MAX_ITEMS {
        if let Some(extra) = metas.pop() {
            let _ = fs::remove_file(dir.join(&extra.file));
        }
    }
    store(root, kind, &metas)
}

pub fn delete_texture(
    root: &Path,
    kind: &str,
    file: &str,
) -> Result<Vec<TextureEntry>, LauncherError> {
    let dir = kind_dir(root, kind)?;
    let name = safe_file_name(file)?;
    let _ = fs::remove_file(dir.join(&name));
    let mut metas = sync_index(root, kind)?;
    metas.retain(|m| m.file != name);
    store(root, kind, &metas)
}

pub fn set_texture_slim(
    root: &Path,
    kind: &str,
    file: &str,
    slim: bool,
) -> Result<Vec<TextureEntry>, LauncherError> {
    let name = safe_file_name(file)?;
    let mut metas = sync_index(root, kind)?;
    for meta in &mut metas {
        if meta.file == name {
            meta.slim = slim;
            meta.slim_manual = true;
        }
    }
    store(root, kind, &metas)
}

fn https_host(url: &str) -> Result<String, LauncherError> {
    let raw = url.trim();
    let rest = raw
        .strip_prefix("https://")
        .ok_or_else(|| LauncherError::Message("need https PNG URL".into()))?;
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() || !TEXTURE_HOSTS.iter().any(|h| host == *h) {
        return Err(LauncherError::Message(format!(
            "skins only from: {}",
            TEXTURE_HOSTS.join(", ")
        )));
    }
    Ok(host)
}

pub async fn fetch_texture_url(http: &HttpPool, url: &str) -> Result<String, LauncherError> {
    let _ = https_host(url)?;
    let bytes = http.get_bytes(url.trim()).await?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(LauncherError::Message("URL is not a PNG".into()));
    }
    if bytes.len() > MAX_BYTES {
        return Err(LauncherError::Message("PNG larger than 2 MB".into()));
    }
    Ok(as_data_url(&bytes))
}

pub async fn ms_profile(http: &HttpPool, token: &str) -> Result<serde_json::Value, LauncherError> {
    // Use the multi-hop pool (system proxy / SOCKS) — primary-only fails on many RU IPs.
    let bearer = format!("Bearer {token}");
    let prof: serde_json::Value = http
        .get_json(MS_PROFILE, &[("Authorization", bearer.as_str())])
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    let capes: Vec<serde_json::Value> = prof["capes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c["id"].as_str().unwrap_or(""),
                "alias": c["alias"].as_str().unwrap_or(""),
                "url": to_https(c["url"].as_str()),
                "active": c["state"].as_str().unwrap_or("") == "ACTIVE",
            })
        })
        .collect();
    let skin = prof["skins"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["state"] == "ACTIVE").or_else(|| a.first()))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(serde_json::json!({
        "uuid": prof["id"].as_str().unwrap_or(""),
        "nick": prof["name"].as_str().unwrap_or(""),
        "skin": to_https(skin["url"].as_str()),
        "slim": skin["variant"].as_str().unwrap_or("").eq_ignore_ascii_case("slim"),
        "capes": capes,
    }))
}

pub async fn ms_set_cape(
    http: &HttpPool,
    token: &str,
    cape_id: &str,
) -> Result<serde_json::Value, LauncherError> {
    let client = http.primary();
    let url = format!("{MS_PROFILE}/capes/active");
    if cape_id.is_empty() {
        return ms_json(http, client.delete(&url).bearer_auth(token)).await;
    }
    ms_json(
        http,
        client
            .put(&url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "capeId": cape_id })),
    )
    .await
}

pub async fn ms_upload_skin(
    http: &HttpPool,
    token: &str,
    png_base64: &str,
    slim: bool,
) -> Result<serde_json::Value, LauncherError> {
    let bytes = png_bytes(png_base64)?;
    let boundary = format!("----WinModing{}", uuid::Uuid::new_v4().simple());
    let variant = if slim { "slim" } else { "classic" };
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let client = http.primary();
    let res = ms_json(
        http,
        client
            .post(format!("{MS_PROFILE}/skins"))
            .bearer_auth(token)
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body),
    )
    .await?;
    let active = res["skins"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["state"] == "ACTIVE").or_else(|| a.first()))
        .and_then(|s| s["url"].as_str())
        .unwrap_or_default();
    if active.is_empty() {
        return Err(LauncherError::Message(
            "Mojang accepted the request but the skin did not change".into(),
        ));
    }
    Ok(serde_json::json!({ "skin": to_https(Some(active)) }))
}

pub async fn ms_reset_skin(http: &HttpPool, token: &str) -> Result<serde_json::Value, LauncherError> {
    let client = http.primary();
    ms_json(
        http,
        client
            .delete(format!("{MS_PROFILE}/skins/active"))
            .bearer_auth(token),
    )
    .await
}
