use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::cache::MetadataCache;
use crate::domain::{instance_id_is_safe, ResolvedFile};
use crate::error::{io_at, LauncherError};
use crate::net::HttpPool;
use crate::paths::AppPaths;
use crate::protocol::{
    curseforge_cdn_url, curseforge_headers, curseforge_loader_type, curseforge_numeric_id,
    force_https, is_curseforge_id, parse_github_repo, CURSEFORGE_API, CURSEFORGE_ID_PREFIX,
    MODRINTH_API,
};
use crate::providers::{self, ModrinthApi, VersionSource};

mod icons;
mod optimize;
mod paths;
pub use icons::attach_icon_data;
pub use icons::{assign_random_icon, file_data_url};
pub use optimize::install_optimization_mods;
use paths::{folder_for, index_path, options_path, relative_is_safe};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledContent {
    pub project_id: String,
    pub source: String,
    pub file_name: String,
    pub folder: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub version_number: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Bundled / launcher-owned content. Hidden from the mods table.
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ContentIndex {
    #[serde(default)]
    files: Vec<InstalledContent>,
}

fn read_index(root: &Path) -> ContentIndex {
    std::fs::read_to_string(index_path(root))
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

fn write_index(root: &Path, index: &ContentIndex) -> Result<(), LauncherError> {
    let path = index_path(root);
    let body = serde_json::to_string_pretty(index)
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    std::fs::write(&path, body).map_err(|err| io_at(&path, err))
}

pub fn read_options(root: &Path) -> serde_json::Value {
    std::fs::read_to_string(options_path(root))
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or(serde_json::json!({}))
}

pub fn write_options(root: &Path, value: &serde_json::Value) -> Result<(), LauncherError> {
    let path = options_path(root);
    let mut map = value.as_object().cloned().unwrap_or_default();
    map.remove("iconDataUrl");
    map.remove("icon_data_url");
    if let Some(existing) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|value| value.as_object().cloned())
    {
        for key in ["recentTimePlayed", "lastPlayed", "submittedTimePlayed"] {
            if !map.contains_key(key) {
                if let Some(prev) = existing.get(key) {
                    map.insert(key.to_string(), prev.clone());
                }
            }
        }
    }
    let body = serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    std::fs::write(&path, body).map_err(|err| io_at(&path, err))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

pub fn add_play_seconds(root: &Path, seconds: u64) -> Result<(), LauncherError> {
    let mut map = read_options(root).as_object().cloned().unwrap_or_default();
    let recent = map
        .get("recentTimePlayed")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            map.get("recentTimePlayed")
                .and_then(|value| value.as_i64())
                .map(|value| value.max(0) as u64)
        })
        .unwrap_or(0);
    map.insert(
        "recentTimePlayed".into(),
        serde_json::json!(recent.saturating_add(seconds)),
    );
    map.insert("lastPlayed".into(), serde_json::json!(unix_now()));
    write_options(root, &serde_json::Value::Object(map))
}

pub fn set_last_crash(root: &Path, summary: &str) -> Result<(), LauncherError> {
    let mut extra = read_options(root);
    let mut map = extra.as_object_mut().cloned().unwrap_or_default();
    map.insert(
        "lastCrash".into(),
        serde_json::Value::String(summary.to_string()),
    );
    write_options(root, &serde_json::Value::Object(map))
}

pub fn clear_last_crash(root: &Path) -> Result<(), LauncherError> {
    let mut extra = read_options(root);
    if let Some(map) = extra.as_object_mut() {
        map.remove("lastCrash");
    }
    write_options(root, &extra)
}

pub fn ignore_mod_update(
    root: &Path,
    project_id: &str,
    version_id: &str,
) -> Result<(), LauncherError> {
    let mut extra = read_options(root);
    let mut map = extra.as_object_mut().cloned().unwrap_or_default();
    let mut ignored = map
        .get("ignoredUpdates")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    ignored.insert(
        project_id.to_string(),
        serde_json::Value::String(version_id.to_string()),
    );
    map.insert("ignoredUpdates".into(), serde_json::Value::Object(ignored));
    write_options(root, &serde_json::Value::Object(map))
}

pub fn list_installed(
    paths: &AppPaths,
    instance_id: &str,
) -> Result<Vec<InstalledContent>, LauncherError> {
    if !instance_id_is_safe(instance_id) {
        return Err(LauncherError::Instance {
            id: instance_id.to_string(),
            reason: "unsafe instance id".into(),
        });
    }
    let root = paths.instance_dir(instance_id);
    prune_missing(&root)?;
    Ok(read_index(&root).files)
}

fn covers_project(item: &InstalledContent, project_id: &str) -> bool {
    same_project(&item.project_id, project_id)
        || same_project(&base_mod_name(&item.file_name), project_id)
}

pub fn is_installed(
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
) -> Result<bool, LauncherError> {
    let wanted = project_id.trim().to_ascii_lowercase();
    if wanted.is_empty() {
        return Ok(false);
    }
    Ok(list_installed(paths, instance_id)?
        .iter()
        .any(|item| covers_project(item, &wanted)))
}

fn looks_like_version_token(token: &str) -> bool {
    let token = token.trim();
    if token.is_empty() {
        return false;
    }
    let body = token.strip_prefix('v').unwrap_or(token);
    let mut dots = 0u8;
    let mut digits = 0u8;
    for ch in body.chars() {
        if ch == '.' {
            dots += 1;
        } else if ch.is_ascii_digit() {
            digits += 1;
        } else if ch == '+' || ch == '-' {
            break;
        } else {
            return false;
        }
    }
    digits > 0 && (dots > 0 || digits >= 2)
}

fn base_mod_name(file_name: &str) -> String {
    let mut name = file_name.to_ascii_lowercase();
    for ext in [".jar.disabled", ".disabled", ".jar", ".zip"] {
        if name.ends_with(ext) {
            name.truncate(name.len() - ext.len());
            break;
        }
    }
    let mut keep = Vec::new();
    for part in name.split(['-', '_', '+']) {
        if part.is_empty() {
            continue;
        }
        if matches!(
            part,
            "fabric" | "forge" | "quilt" | "neoforge" | "mc" | "mod"
        ) {
            continue;
        }
        if looks_like_version_token(part) {
            break;
        }
        keep.push(part);
    }
    if keep.is_empty() {
        name
    } else {
        keep.join("-")
    }
}

fn pretty_version_label(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_end_matches(".jar.disabled")
        .trim_end_matches(".disabled")
        .trim_end_matches(".jar");
    let mut last = None;
    for part in trimmed.split(['-', '_', ' ', '+', '/']) {
        if looks_like_version_token(part) {
            last = Some(part.trim_start_matches('v').to_string());
        }
    }
    last.unwrap_or_else(|| trimmed.to_string())
}

fn same_project(stored: &str, wanted: &str) -> bool {
    fn norm(value: &str) -> String {
        let lower = value.trim().to_ascii_lowercase();
        lower
            .trim_start_matches("modrinth:")
            .trim_start_matches("curseforge:")
            .trim_start_matches("github:")
            .trim_start_matches("cf-")
            .to_string()
    }
    let stored = norm(stored);
    let wanted = norm(wanted);
    stored == wanted
        || stored.split('/').next_back() == Some(wanted.as_str())
        || wanted.split('/').next_back() == Some(stored.as_str())
        || (!stored.is_empty()
            && !wanted.is_empty()
            && (stored == base_mod_name(&wanted) || wanted == base_mod_name(&stored)))
}

fn content_file_present(root: &Path, folder: &str, file_name: &str) -> bool {
    let dest = root.join(folder).join(file_name);
    if dest.exists() {
        return true;
    }
    let mut disabled = dest.clone();
    disabled.set_file_name(format!("{file_name}.disabled"));
    disabled.exists()
        || dest.with_extension("jar.disabled").exists()
        || dest.with_extension("disabled").exists()
}

fn prune_missing(root: &Path) -> Result<(), LauncherError> {
    let mut index = read_index(root);
    let before = index.files.len();
    index
        .files
        .retain(|item| content_file_present(root, &item.folder, &item.file_name));
    let mut added = 0usize;
    for folder in ["mods", "resourcepacks", "shaderpacks", "datapacks"] {
        let dir = root.join(folder);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if !(lower.ends_with(".jar")
                || lower.ends_with(".zip")
                || lower.ends_with(".jar.disabled")
                || lower.ends_with(".disabled"))
            {
                continue;
            }
            let already = index
                .files
                .iter()
                .any(|item| item.folder == folder && names_match(&item.file_name, &name));
            if already {
                continue;
            }
            let stem = base_mod_name(&name);
            let catalog_match = index.files.iter().any(|item| {
                item.folder == folder
                    && item.source != "local"
                    && (same_project(&item.project_id, &stem)
                        || base_mod_name(&item.file_name) == stem)
            });
            if catalog_match {
                continue;
            }
            index.files.push(InstalledContent {
                project_id: stem.clone(),
                source: "local".into(),
                file_name: name.clone(),
                folder: folder.into(),
                version_id: None,
                version_number: Some(pretty_version_label(&name)),
                icon_url: None,
                hidden: false,
            });
            added += 1;
        }
    }
    let collapsed = collapse_duplicate_mods(root, &mut index);
    let orphans = delete_orphan_mod_jars(root, &index);
    if index.files.len() != before || added > 0 || collapsed || orphans {
        write_index(root, &index)?;
    }
    Ok(())
}

fn delete_orphan_mod_jars(root: &Path, index: &ContentIndex) -> bool {
    let mut changed = false;
    for folder in ["mods", "resourcepacks", "shaderpacks", "datapacks"] {
        let dir = root.join(folder);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if !(lower.ends_with(".jar")
                || lower.ends_with(".zip")
                || lower.ends_with(".jar.disabled")
                || lower.ends_with(".disabled"))
            {
                continue;
            }
            let keep = index
                .files
                .iter()
                .any(|item| item.folder == folder && names_match(&item.file_name, &name));
            if keep {
                continue;
            }
            let stem = base_mod_name(&name);
            let duplicate = index.files.iter().any(|item| {
                item.folder == folder
                    && (same_project(&item.project_id, &stem)
                        || base_mod_name(&item.file_name) == stem)
            });
            if duplicate {
                let path = dir.join(&name);
                if std::fs::remove_file(&path).is_ok() {
                    changed = true;
                }
            }
        }
    }
    changed
}

fn version_rank(value: &str) -> Vec<u32> {
    pretty_version_label(value)
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn better_mod(left: &InstalledContent, right: &InstalledContent) -> bool {
    let left_catalog = left.source != "local";
    let right_catalog = right.source != "local";
    if left_catalog != right_catalog {
        return left_catalog;
    }
    version_rank(&left.file_name) > version_rank(&right.file_name)
}

fn collapse_duplicate_mods(root: &Path, index: &mut ContentIndex) -> bool {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, item) in index.files.iter().enumerate() {
        let stem = base_mod_name(&item.file_name);
        let pid = item.project_id.trim().to_ascii_lowercase();
        let key = if !stem.is_empty() {
            format!("{}::{}", item.folder, stem)
        } else {
            format!("{}::{}", item.folder, pid)
        };
        groups.entry(key).or_default().push(idx);
    }
    let mut drop = HashSet::new();
    for indexes in groups.values() {
        if indexes.len() < 2 {
            continue;
        }
        let mut winner = indexes[0];
        for &idx in indexes.iter().skip(1) {
            if better_mod(&index.files[idx], &index.files[winner]) {
                winner = idx;
            }
        }
        for &idx in indexes {
            if idx != winner {
                drop.insert(idx);
            }
        }
    }
    if drop.is_empty() {
        return false;
    }
    for (idx, item) in index.files.iter().enumerate() {
        if drop.contains(&idx) {
            delete_indexed_file(root, &item.folder, &item.file_name);
        }
    }
    let mut keep = Vec::with_capacity(index.files.len() - drop.len());
    for (idx, item) in index.files.iter().cloned().enumerate() {
        if !drop.contains(&idx) {
            keep.push(item);
        }
    }
    index.files = keep;
    true
}

fn names_match(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
        || format!("{left}.disabled").eq_ignore_ascii_case(right)
        || format!("{right}.disabled").eq_ignore_ascii_case(left)
}

pub fn remove_content(
    paths: &AppPaths,
    instance_id: &str,
    project_path: &str,
) -> Result<(), LauncherError> {
    if !instance_id_is_safe(instance_id) {
        return Err(LauncherError::Instance {
            id: instance_id.to_string(),
            reason: "unsafe instance id".into(),
        });
    }
    let root = paths.instance_dir(instance_id);
    if !relative_is_safe(project_path) {
        return Err(LauncherError::Message("unsafe content path".into()));
    }
    let relative = project_path
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    let file_name = Path::new(&relative)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| relative.clone());
    let folder = Path::new(&relative)
        .parent()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != ".")
        .unwrap_or("mods");
    let dest = root.join(folder).join(&file_name);
    for path in [
        dest.clone(),
        dest.with_extension("jar.disabled"),
        dest.with_extension("disabled"),
        {
            let mut disabled = dest.clone();
            disabled.set_file_name(format!("{file_name}.disabled"));
            disabled
        },
    ] {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|err| io_at(&path, err))?;
        }
    }
    let mut index = read_index(&root);
    index.files.retain(|item| {
        let item_rel = format!("{}/{}", item.folder, item.file_name);
        !names_match(&item.file_name, &file_name)
            && !item_rel.eq_ignore_ascii_case(&relative)
            && !same_project(&item.project_id, &relative.to_ascii_lowercase())
            && !item.project_id.eq_ignore_ascii_case(&file_name)
    });
    write_index(&root, &index)?;
    Ok(())
}

fn safe_file_name(name: &str) -> String {
    Path::new(name)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty() && !value.contains(".."))
        .unwrap_or_else(|| "download.jar".into())
}

async fn download_files(
    http: &HttpPool,
    dest_dir: &Path,
    files: &[ResolvedFile],
) -> Result<Vec<String>, LauncherError> {
    tokio::fs::create_dir_all(dest_dir)
        .await
        .map_err(|err| io_at(dest_dir, err))?;
    let mut names = Vec::new();
    for file in files {
        let name = safe_file_name(&file.file_name);
        let dest = dest_dir.join(&name);
        if let Some(sha1) = file.sha1.as_deref().filter(|value| !value.is_empty()) {
            http.download_verified(&file.url, &dest, &crate::domain::ContentHash::sha1(sha1))
                .await?;
        } else {
            http.download_to(&file.url, &dest).await?;
        }
        names.push(name);
    }
    Ok(names)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedDownload {
    pub path: String,
    pub folder: String,
    pub file_name: String,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LibraryIndex {
    #[serde(default)]
    files: Vec<LibraryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryItem {
    project_id: String,
    file_name: String,
    folder: String,
}

fn library_index_path(paths: &AppPaths) -> std::path::PathBuf {
    paths.downloads_dir().join("library.json")
}

fn read_library(paths: &AppPaths) -> LibraryIndex {
    std::fs::read_to_string(library_index_path(paths))
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

fn write_library(paths: &AppPaths, index: &LibraryIndex) -> Result<(), LauncherError> {
    let path = library_index_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| io_at(parent, err))?;
    }
    let body = serde_json::to_string_pretty(index)
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    std::fs::write(&path, body).map_err(|err| io_at(&path, err))
}

pub fn list_saved_project_ids(paths: &AppPaths) -> Vec<String> {
    read_library(paths)
        .files
        .into_iter()
        .map(|item| item.project_id)
        .collect()
}

fn remember_library(paths: &AppPaths, project_id: &str, folder: &str, file_name: &str) {
    let mut index = read_library(paths);
    index
        .files
        .retain(|item| !item.project_id.eq_ignore_ascii_case(project_id));
    index.files.push(LibraryItem {
        project_id: project_id.to_string(),
        file_name: file_name.to_string(),
        folder: folder.to_string(),
    });
    let _ = write_library(paths, &index);
}

async fn resolve_modrinth_file(
    http: &HttpPool,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<ResolvedFile, LauncherError> {
    let version: serde_json::Value = if let Some(id) = version_id.filter(|value| !value.is_empty()) {
        http.get_json(
            &format!("{MODRINTH_API}/version/{}", urlencoding::encode(id)),
            &[],
        )
        .await?
    } else {
        let versions: Vec<serde_json::Value> = http
            .get_json(
                &format!(
                    "{MODRINTH_API}/project/{}/version",
                    urlencoding::encode(project_id)
                ),
                &[],
            )
            .await?;
        versions.into_iter().next().ok_or_else(|| {
            LauncherError::Message(format!("no files for {project_id}"))
        })?
    };
    let files = version
        .get("files")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let file = files
        .iter()
        .find(|item| item.get("primary").and_then(|value| value.as_bool()) == Some(true))
        .or_else(|| files.first())
        .ok_or_else(|| LauncherError::Message(format!("no files for {project_id}")))?;
    let file_name = file
        .get("filename")
        .and_then(|item| item.as_str())
        .unwrap_or("download.jar")
        .to_string();
    let url = file
        .get("url")
        .and_then(|item| item.as_str())
        .ok_or_else(|| LauncherError::Message(format!("no download url for {project_id}")))?
        .to_string();
    let sha1 = file
        .pointer("/hashes/sha1")
        .and_then(|item| item.as_str())
        .map(str::to_string);
    Ok(ResolvedFile {
        key: version
            .get("id")
            .and_then(|item| item.as_str())
            .unwrap_or(project_id)
            .to_string(),
        project_id: project_id.to_string(),
        file_name,
        url,
        sha1,
    })
}

async fn resolve_curseforge_file(
    http: &HttpPool,
    project_id: &str,
    version_id: Option<&str>,
    api_key: &str,
) -> Result<ResolvedFile, LauncherError> {
    let project_id = curseforge_numeric_id(project_id);
    let file = if let Some(file_id) =
        version_id.filter(|value| curseforge_numeric_id(value).chars().all(|ch| ch.is_ascii_digit()))
    {
        let file_id = curseforge_numeric_id(file_id);
        let parsed: CfFileWrap = http
            .get_json(
                &format!("{CURSEFORGE_API}/mods/{project_id}/files/{file_id}"),
                &curseforge_headers(api_key),
            )
            .await?;
        parsed.data
    } else {
        let parsed: CfFiles = http
            .get_json(
                &format!("{CURSEFORGE_API}/mods/{project_id}/files?pageSize=50"),
                &curseforge_headers(api_key),
            )
            .await?;
        pick_curseforge_file(parsed.data, "", "").ok_or_else(|| {
            LauncherError::Message(format!("no CurseForge file for {project_id}"))
        })?
    };
    let mut download = file
        .download_url
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_default();
    download = force_https(&download);
    if download.is_empty() {
        download = curseforge_cdn_url(file.id, &file.file_name);
    }
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone());
    Ok(ResolvedFile {
        key: file.id.to_string(),
        project_id: format!("{CURSEFORGE_ID_PREFIX}{project_id}"),
        file_name: file.file_name,
        url: download,
        sha1,
    })
}

pub async fn save_catalog_download(
    http: &HttpPool,
    paths: &AppPaths,
    project_id: &str,
    version_id: Option<&str>,
    source: &str,
    project_type: &str,
    api_key: &str,
) -> Result<SavedDownload, LauncherError> {
    let preferred = detect_source(source, project_id);
    let slug = slug_of(project_id);
    let file = if preferred == "curseforge" {
        resolve_curseforge_file(http, &slug, version_id, api_key).await?
    } else {
        resolve_modrinth_file(http, &slug, version_id).await?
    };
    let folder = folder_for(project_type);
    let dest_dir = paths.downloads_dir().join(folder);
    let names = download_files(http, &dest_dir, std::slice::from_ref(&file)).await?;
    let file_name = names
        .into_iter()
        .next()
        .ok_or_else(|| LauncherError::Message(format!("could not save {project_id}")))?;
    remember_library(paths, project_id, folder, &file_name);
    let path = dest_dir.join(&file_name);
    Ok(SavedDownload {
        path: path.display().to_string(),
        folder: dest_dir.display().to_string(),
        file_name,
        project_id: project_id.to_string(),
    })
}

fn delete_indexed_file(root: &Path, folder: &str, file_name: &str) {
    let dest = root.join(folder).join(file_name);
    for path in [
        dest.clone(),
        dest.with_extension("jar.disabled"),
        dest.with_extension("disabled"),
        {
            let mut disabled = dest.clone();
            disabled.set_file_name(format!("{file_name}.disabled"));
            disabled
        },
    ] {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn same_mod_file(folder: &str, file_name: &str, other: &InstalledContent, wanted: &str) -> bool {
    if other.folder != folder && !folder.is_empty() {
        return false;
    }
    same_project(&other.project_id, wanted) || names_match(&other.file_name, file_name) || {
        let left = base_mod_name(&other.file_name);
        let right = base_mod_name(file_name);
        !left.is_empty() && left == right
    }
}

fn adopt_existing(
    root: &Path,
    project_id: &str,
    source: &str,
    folder: &str,
    file_name: &str,
    version_id: Option<&str>,
    icon_url: Option<&str>,
) -> Result<Option<String>, LauncherError> {
    let stem = base_mod_name(file_name);
    if stem.is_empty() {
        return Ok(None);
    }
    let mut index = read_index(root);
    let mut adopted = None;
    for item in index.files.iter_mut() {
        if item.folder != folder && !folder.is_empty() {
            continue;
        }
        if base_mod_name(&item.file_name) != stem {
            continue;
        }
        let same_file = names_match(&item.file_name, file_name);
        let same_version = match (version_id, item.version_id.as_deref()) {
            (Some(wanted), Some(have)) => wanted == have,
            _ => same_file,
        };
        if !same_file || !same_version {
            continue;
        }
        adopted = Some(item.file_name.clone());
        if !same_project(&item.project_id, project_id) {
            item.project_id = project_id.to_string();
            item.source = source.to_string();
        }
        if let Some(value) = version_id {
            item.version_id = Some(value.to_string());
        }
        if let Some(value) = icon_url {
            item.icon_url = Some(value.to_string());
        }
    }
    if adopted.is_some() {
        write_index(root, &index)?;
    }
    Ok(adopted)
}

fn remember(
    root: &Path,
    project_id: &str,
    source: &str,
    folder: &str,
    names: &[String],
    version_id: Option<&str>,
    icon_url: Option<&str>,
) -> Result<(), LauncherError> {
    let mut index = read_index(root);
    let wanted = project_id.to_ascii_lowercase();
    for item in index.files.iter() {
        let is_new = names
            .iter()
            .any(|name| item.folder == folder && names_match(&item.file_name, name));
        if is_new {
            continue;
        }
        let matches_id = same_project(&item.project_id, &wanted);
        let matches_name = names
            .iter()
            .any(|name| same_mod_file(folder, name, item, &wanted));
        if matches_id || matches_name {
            delete_indexed_file(root, &item.folder, &item.file_name);
        }
    }
    index.files.retain(|item| {
        let matches_id = same_project(&item.project_id, &wanted);
        let matches_name = names
            .iter()
            .any(|name| same_mod_file(folder, name, item, &wanted));
        !matches_id && !matches_name
    });
    for name in names {
        if name.eq_ignore_ascii_case("download.jar") {
            continue;
        }
        index.files.push(InstalledContent {
            project_id: project_id.to_string(),
            source: source.to_string(),
            file_name: name.clone(),
            folder: folder.to_string(),
            version_id: version_id.map(str::to_string),
            version_number: Some(pretty_version_label(name)),
            icon_url: icon_url.map(str::to_string),
            hidden: false,
        });
    }
    collapse_duplicate_mods(root, &mut index);
    write_index(root, &index)
}

pub fn toggle_disable(
    paths: &AppPaths,
    instance_id: &str,
    project_path: &str,
) -> Result<String, LauncherError> {
    if !instance_id_is_safe(instance_id) {
        return Err(LauncherError::Instance {
            id: instance_id.to_string(),
            reason: "unsafe instance id".into(),
        });
    }
    let root = paths.instance_dir(instance_id);
    if !relative_is_safe(project_path) {
        return Err(LauncherError::Message("unsafe content path".into()));
    }
    let relative = project_path
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    let file_name = Path::new(&relative)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| relative.clone());
    let folder = Path::new(&relative)
        .parent()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != ".")
        .unwrap_or("mods");
    let current = root.join(folder).join(&file_name);
    let lower = file_name.to_ascii_lowercase();
    let disabled = lower.ends_with(".disabled");
    let new_name = if disabled {
        file_name
            .strip_suffix(".disabled")
            .or_else(|| file_name.strip_suffix(".DISABLED"))
            .unwrap_or(&file_name)
            .to_string()
    } else {
        format!("{file_name}.disabled")
    };
    let next = root.join(folder).join(&new_name);
    if current.exists() {
        if next.exists() && next != current {
            let _ = std::fs::remove_file(&next);
        }
        std::fs::rename(&current, &next).map_err(|err| io_at(&current, err))?;
    } else {
        let alt = if disabled {
            root.join(folder)
                .join(file_name.trim_end_matches(".disabled"))
        } else {
            let mut path = current.clone();
            path.set_file_name(format!("{file_name}.disabled"));
            path
        };
        if alt.exists() {
            std::fs::rename(&alt, &next).map_err(|err| io_at(&alt, err))?;
        } else {
            return Err(LauncherError::Message(format!(
                "mod file missing: {relative}"
            )));
        }
    }
    let mut index = read_index(&root);
    for item in index.files.iter_mut() {
        if item.folder == folder && names_match(&item.file_name, &file_name) {
            item.file_name = new_name.clone();
        }
    }
    write_index(&root, &index)?;
    Ok(format!("{folder}/{new_name}"))
}

async fn fetch_modrinth_icon(http: &HttpPool, id: &str) -> Option<String> {
    let url = format!("{MODRINTH_API}/project/{}", urlencoding::encode(id));
    let value: serde_json::Value = http.get_json(&url, &[]).await.ok()?;
    value
        .get("icon_url")
        .and_then(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

async fn fetch_curseforge_icon(http: &HttpPool, id: &str, api_key: &str) -> Option<String> {
    let url = format!("{CURSEFORGE_API}/mods/{id}");
    let value: serde_json::Value = http
        .get_json(&url, &curseforge_headers(api_key))
        .await
        .ok()?;
    value
        .pointer("/data/logo/thumbnailUrl")
        .or_else(|| value.pointer("/data/logo/url"))
        .and_then(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

pub fn detect_source(source: &str, project_id: &str) -> String {
    let source = source.trim().to_ascii_lowercase();
    let id = project_id.trim().to_ascii_lowercase();
    if source.contains("github")
        || id.contains("github.com")
        || id.starts_with("gh:")
        || parse_github_repo(project_id).is_some()
    {
        "github".into()
    } else if source.contains("curse") || is_curseforge_id(project_id) {
        "curseforge".into()
    } else {
        "modrinth".into()
    }
}

pub async fn install_any(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    game_version: &str,
    loader: &str,
    project_type: &str,
    preferred: &str,
    api_key: &str,
) -> Result<Vec<String>, LauncherError> {
    let preferred = detect_source(preferred, project_id);
    let slug = slug_of(project_id);
    if preferred == "github" {
        return install_github(
            http,
            cache,
            paths,
            instance_id,
            project_id,
            game_version,
            loader,
            project_type,
            api_key,
        )
        .await;
    }
    let mut errors = Vec::new();
    if preferred != "curseforge" {
        match install_modrinth(
            http,
            cache,
            paths,
            instance_id,
            &slug,
            version_id,
            game_version,
            loader,
            project_type,
        )
        .await
        {
            Ok(names) => return Ok(names),
            Err(err) => errors.push(format!("Modrinth: {err}")),
        }
    }
    match install_curseforge_flexible(
        http,
        paths,
        instance_id,
        &slug,
        version_id,
        game_version,
        loader,
        project_type,
        api_key,
    )
    .await
    {
        Ok(names) => return Ok(names),
        Err(err) => errors.push(format!("CurseForge: {err}")),
    }
    if preferred == "curseforge" {
        match install_modrinth(
            http,
            cache,
            paths,
            instance_id,
            &slug,
            version_id,
            game_version,
            loader,
            project_type,
        )
        .await
        {
            Ok(names) => return Ok(names),
            Err(err) => errors.push(format!("Modrinth: {err}")),
        }
    }
    Err(LauncherError::Message(format!(
        "could not install {project_id}: {}",
        errors.join(" | ")
    )))
}

fn slug_of(project_id: &str) -> String {
    parse_github_repo(project_id)
        .map(|(_, repo)| repo)
        .unwrap_or_else(|| curseforge_numeric_id(project_id).to_string())
}

pub async fn install_modrinth(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    game_version: &str,
    loader: &str,
    project_type: &str,
) -> Result<Vec<String>, LauncherError> {
    let api = ModrinthApi::new(http.clone(), cache.clone());
    let version = if let Some(id) = version_id.filter(|value| !value.is_empty()) {
        id.to_string()
    } else {
        api.latest_for_project(project_id, game_version, loader)
            .await?
    };
    let files = match providers::resolve_required(&api, &version, game_version, loader).await {
        Ok(rows) if !rows.is_empty() => rows,
        Ok(_) | Err(_) => {
            let remote = api.fetch_version(&version).await?;
            vec![crate::domain::ResolvedFile {
                key: remote.id,
                project_id: remote.project_id,
                file_name: remote.file_name,
                url: remote.url,
                sha1: remote.sha1,
            }]
        }
    };
    let folder = folder_for(project_type);
    let root = paths.instance_dir(instance_id);
    let dest = root.join(folder);
    let root_icon = fetch_modrinth_icon(http, project_id).await;
    let mut installed_names = Vec::new();
    for file in files {
        if file.project_id != project_id
            && is_installed(paths, instance_id, &file.project_id).unwrap_or(false)
        {
            continue;
        }
        let icon = if file.project_id == project_id {
            root_icon.clone()
        } else {
            fetch_modrinth_icon(http, &file.project_id).await
        };
        if let Some(existing) = adopt_existing(
            &root,
            &file.project_id,
            "modrinth",
            folder,
            &file.file_name,
            Some(&file.key),
            icon.as_deref(),
        )? {
            if file.project_id == project_id {
                installed_names.push(existing);
            }
            continue;
        }
        let names = download_files(http, &dest, std::slice::from_ref(&file)).await?;
        remember(
            &root,
            &file.project_id,
            "modrinth",
            folder,
            &names,
            Some(&file.key),
            icon.as_deref(),
        )?;
        if file.project_id == project_id {
            installed_names.extend(names);
        }
    }
    if installed_names.is_empty() {
        return Err(LauncherError::Message(format!(
            "could not install {project_id}"
        )));
    }
    Ok(installed_names)
}

pub async fn install_curseforge(
    http: &HttpPool,
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    game_version: &str,
    loader: &str,
    project_type: &str,
    api_key: &str,
) -> Result<Vec<String>, LauncherError> {
    let project_id = curseforge_numeric_id(project_id);
    let file = if let Some(file_id) =
        version_id.filter(|value| curseforge_numeric_id(value).chars().all(|ch| ch.is_ascii_digit()))
    {
        let file_id = curseforge_numeric_id(file_id);
        let parsed: CfFileWrap = http
            .get_json(
                &format!("{CURSEFORGE_API}/mods/{project_id}/files/{file_id}"),
                &curseforge_headers(api_key),
            )
            .await?;
        parsed.data
    } else {
        let mut url = format!("{CURSEFORGE_API}/mods/{project_id}/files?pageSize=50");
        if !game_version.is_empty() {
            url.push_str("&gameVersion=");
            url.push_str(&urlencoding::encode(game_version));
        }
        if let Some(kind) = curseforge_loader_type(loader) {
            url.push_str("&modLoaderType=");
            url.push_str(&kind.to_string());
        }
        let parsed: CfFiles = http.get_json(&url, &curseforge_headers(api_key)).await?;
        pick_curseforge_file(parsed.data, game_version, loader).ok_or_else(|| {
            LauncherError::Message(format!(
                "no CurseForge file for {project_id} / {game_version}"
            ))
        })?
    };
    let mut download = file
        .download_url
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_default();
    download = force_https(&download);
    if download.is_empty() {
        if let Ok(lookup) = http
            .get_json::<CfDownload>(
                &format!(
                    "{CURSEFORGE_API}/mods/{project_id}/files/{}/download-url",
                    file.id
                ),
                &curseforge_headers(api_key),
            )
            .await
        {
            download = force_https(lookup.data.trim());
        }
    }
    if download.is_empty() {
        download = curseforge_cdn_url(file.id, &file.file_name);
    }
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .map(|hash| hash.value.clone());
    let resolved = ResolvedFile {
        key: file.id.to_string(),
        project_id: project_id.to_string(),
        file_name: file.file_name,
        url: download,
        sha1,
    };
    let folder = folder_for(project_type);
    let root = paths.instance_dir(instance_id);
    let dest = root.join(folder);
    let names = download_files(http, &dest, &[resolved]).await?;
    let icon = fetch_curseforge_icon(http, project_id, api_key).await;
    remember(
        &root,
        project_id,
        "curseforge",
        folder,
        &names,
        Some(&file.id.to_string()),
        icon.as_deref(),
    )?;
    Ok(names)
}

pub async fn install_from_version(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<String>, LauncherError> {
    if is_curseforge_id(version_id) {
        let api_key = paths.read_curseforge_key();
        let instance = cache.get_instance(instance_id).await?;
        if let Some(record) = instance {
            let files = list_installed(paths, instance_id).unwrap_or_default();
            if let Some(item) = files.iter().find(|file| {
                file.version_id.as_deref() == Some(version_id)
                    || (file.source.contains("curse") && is_curseforge_id(&file.project_id))
            }) {
                return install_curseforge(
                    http,
                    paths,
                    instance_id,
                    &item.project_id,
                    Some(version_id),
                    &record.game_version,
                    record.loader.as_str(),
                    "mod",
                    &api_key,
                )
                .await;
            }
        }
    }
    let api = ModrinthApi::new(http.clone(), cache.clone());
    let remote = api.fetch_version(version_id).await?;
    install_modrinth(
        http,
        cache,
        paths,
        instance_id,
        &remote.project_id,
        Some(version_id),
        game_version,
        loader,
        "mod",
    )
    .await
}

pub(crate) async fn install_curseforge_flexible(
    http: &HttpPool,
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    game_version: &str,
    loader: &str,
    project_type: &str,
    api_key: &str,
) -> Result<Vec<String>, LauncherError> {
    let id = if is_curseforge_id(project_id) {
        curseforge_numeric_id(project_id).to_string()
    } else {
        lookup_curseforge_id(http, project_id, api_key).await?
    };
    install_curseforge(
        http,
        paths,
        instance_id,
        &id,
        version_id,
        game_version,
        loader,
        project_type,
        api_key,
    )
    .await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentUpdate {
    pub project_id: String,
    pub latest_id: String,
    pub latest_number: String,
    pub outdated: bool,
}

pub async fn catalog_project(
    http: &HttpPool,
    id: &str,
    api_key: &str,
) -> Result<serde_json::Value, LauncherError> {
    let wanted = id.trim();
    if is_curseforge_id(wanted) {
        let wanted = curseforge_numeric_id(wanted);
        let url = format!("{CURSEFORGE_API}/mods/{wanted}");
        let value: serde_json::Value = http.get_json(&url, &curseforge_headers(api_key)).await?;
        let data = value.get("data").cloned().unwrap_or(value);
        let title = data
            .pointer("/name")
            .and_then(|item| item.as_str())
            .unwrap_or(wanted);
        let icon = data
            .pointer("/logo/thumbnailUrl")
            .or_else(|| data.pointer("/logo/url"))
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let summary = data
            .pointer("/summary")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let downloads = data
            .pointer("/downloadCount")
            .and_then(|item| item.as_u64())
            .unwrap_or(0);
        let follows = data
            .pointer("/thumbsUpCount")
            .and_then(|item| item.as_u64())
            .unwrap_or(0);
        let slug = data
            .pointer("/slug")
            .and_then(|item| item.as_str())
            .unwrap_or(wanted);
        return Ok(serde_json::json!({
            "id": format!("{CURSEFORGE_ID_PREFIX}{wanted}"),
            "slug": slug,
            "title": title,
            "description": summary,
            "icon_url": icon,
            "project_type": "mod",
            "source": "curseforge",
            "versions": [],
            "categories": [],
            "downloads": downloads,
            "followers": follows,
            "follows": follows,
            "client_side": "unknown",
            "server_side": "unknown",
            "team": null,
            "published": data.get("dateCreated"),
            "updated": data.get("dateModified"),
        }));
    }
    let url = format!("{MODRINTH_API}/project/{}", urlencoding::encode(wanted));
    let mut project: serde_json::Value = http.get_json(&url, &[]).await?;
    if let Some(map) = project.as_object_mut() {
        map.insert(
            "source".into(),
            serde_json::Value::String("modrinth".into()),
        );
    }
    Ok(project)
}

pub async fn catalog_versions(
    http: &HttpPool,
    cache: &MetadataCache,
    id: &str,
    source: &str,
    game_version: &str,
    loader: &str,
    api_key: &str,
) -> Result<Vec<serde_json::Value>, LauncherError> {
    if source.contains("curse") || is_curseforge_id(id) {
        let id = curseforge_numeric_id(id);
        let mut url = format!("{CURSEFORGE_API}/mods/{id}/files?pageSize=50");
        if !game_version.is_empty() {
            url.push_str("&gameVersion=");
            url.push_str(&urlencoding::encode(game_version));
        }
        if let Some(kind) = curseforge_loader_type(loader) {
            url.push_str("&modLoaderType=");
            url.push_str(&kind.to_string());
        }
        let parsed: CfFiles = http.get_json(&url, &curseforge_headers(api_key)).await?;
        return Ok(parsed
            .data
            .into_iter()
            .map(|file| {
                let name = file
                    .display_name
                    .clone()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| file.file_name.clone());
                let loaders: Vec<String> = file
                    .game_versions
                    .iter()
                    .filter_map(|value| {
                        let lower = value.to_ascii_lowercase();
                        if lower.contains("fabric") {
                            Some("fabric".into())
                        } else if lower.contains("quilt") {
                            Some("quilt".into())
                        } else if lower.contains("neoforge") {
                            Some("neoforge".into())
                        } else if lower.contains("forge") {
                            Some("forge".into())
                        } else {
                            None
                        }
                    })
                    .collect();
                serde_json::json!({
                    "id": file.id.to_string(),
                    "name": name,
                    "version_number": name,
                    "game_versions": file.game_versions,
                    "loaders": loaders,
                    "date_published": file.file_date,
                    "version_type": "release",
                    "changelog": "",
                    "author_id": "",
                    "files": [{
                        "id": file.id.to_string(),
                        "filename": file.file_name,
                        "url": file.download_url,
                        "primary": true,
                        "size": 0
                    }],
                    "dependencies": []
                })
            })
            .collect());
    }
    let _ = cache;
    let url = format!(
        "{MODRINTH_API}/project/{}/version?game_versions={}&loaders={}",
        urlencoding::encode(id),
        urlencoding::encode(&serde_json::json!([game_version]).to_string()),
        urlencoding::encode(&serde_json::json!([loader]).to_string()),
    );
    http.get_json(&url, &[]).await
}

pub async fn content_updates(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    game_version: &str,
    loader: &str,
    api_key: &str,
) -> Result<Vec<ContentUpdate>, LauncherError> {
    let files = list_installed(paths, instance_id)?;
    let api = ModrinthApi::new(http.clone(), cache.clone());
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for file in files {
        if file.hidden || file.source == "local" {
            continue;
        }
        let pid = file.project_id.trim().to_ascii_lowercase();
        let stem = base_mod_name(&file.file_name);
        if !pid.is_empty() && !seen.insert(pid.clone()) {
            continue;
        }
        if !stem.is_empty() && stem != pid && !seen.insert(stem) {
            continue;
        }
        let current = file.version_id.clone().unwrap_or_default();
        let current_number = file
            .version_number
            .clone()
            .unwrap_or_else(|| pretty_version_label(&file.file_name));
        let latest = if file.source.contains("curse") || is_curseforge_id(&file.project_id) {
            let mut url = format!(
                "{CURSEFORGE_API}/mods/{}/files?pageSize=5",
                curseforge_numeric_id(&file.project_id)
            );
            if !game_version.is_empty() {
                url.push_str("&gameVersion=");
                url.push_str(&urlencoding::encode(game_version));
            }
            if let Some(kind) = curseforge_loader_type(loader) {
                url.push_str("&modLoaderType=");
                url.push_str(&kind.to_string());
            }
            match http
                .get_json::<CfFiles>(&url, &curseforge_headers(api_key))
                .await
            {
                Ok(parsed) => parsed
                    .data
                    .into_iter()
                    .max_by(|left, right| left.file_date.cmp(&right.file_date))
                    .map(|item| {
                        let raw = item
                            .display_name
                            .filter(|value| !value.is_empty())
                            .unwrap_or(item.file_name);
                        (item.id.to_string(), pretty_version_label(&raw))
                    }),
                Err(_) => None,
            }
        } else {
            match api
                .latest_for_project(&file.project_id, game_version, loader)
                .await
            {
                Ok(id) => {
                    let number = api
                        .fetch_version(&id)
                        .await
                        .map(|remote| {
                            if remote.version_number.trim().is_empty() {
                                pretty_version_label(&remote.file_name)
                            } else {
                                pretty_version_label(&remote.version_number)
                            }
                        })
                        .unwrap_or_else(|_| pretty_version_label(&id));
                    Some((id, number))
                }
                Err(_) => None,
            }
        };
        let Some((latest_id, latest_number)) = latest else {
            continue;
        };
        if latest_id.is_empty() {
            continue;
        }
        if !current.is_empty() {
            if latest_id == current {
                continue;
            }
        } else if latest_number == current_number {
            continue;
        }
        out.push(ContentUpdate {
            project_id: file.project_id,
            latest_id,
            latest_number,
            outdated: true,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingDependency {
    pub project_id: String,
    pub version_id: Option<String>,
}

fn skip_quilt_fabric_api(loader: &str, project_id: &str) -> bool {
    loader.eq_ignore_ascii_case("quilt")
        && (same_project(project_id, "P7dR8mSH") || same_project(project_id, "fabric-api"))
}

pub async fn list_missing_dependencies(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
) -> Result<Vec<MissingDependency>, LauncherError> {
    let loader = cache
        .get_instance(instance_id)
        .await?
        .map(|record| record.loader.as_str().to_string())
        .unwrap_or_default();
    let files = list_installed(paths, instance_id)?;
    let api = ModrinthApi::new(http.clone(), cache.clone());
    let mut missing = Vec::new();
    let mut seen = HashSet::new();
    for file in &files {
        if file.hidden || file.source == "local" {
            continue;
        }
        let Some(version_id) = file.version_id.as_deref().filter(|value| !value.is_empty()) else {
            continue;
        };
        if file.source.contains("curse") || is_curseforge_id(&file.project_id) {
            continue;
        }
        let Ok(remote) = api.fetch_version(version_id).await else {
            continue;
        };
        let mut required: Vec<(String, Option<String>, Option<String>)> = Vec::new();
        for dep_version in remote.required_version_ids {
            if let Ok(dep) = api.fetch_version(&dep_version).await {
                required.push((dep.project_id, Some(dep_version), Some(dep.file_name)));
            }
        }
        for pid in remote.required_project_ids {
            required.push((pid, None, None));
        }
        for (pid, dep_version, dep_file) in required {
            if skip_quilt_fabric_api(&loader, &pid) {
                continue;
            }
            let stem = dep_file.as_deref().map(base_mod_name).unwrap_or_default();
            if files.iter().any(|have| {
                covers_project(have, &pid)
                    || (!stem.is_empty() && base_mod_name(&have.file_name) == stem)
            }) {
                continue;
            }
            let key = pid.to_ascii_lowercase();
            if !seen.insert(key) {
                continue;
            }
            missing.push(MissingDependency {
                project_id: pid,
                version_id: dep_version,
            });
        }
    }
    Ok(missing)
}

pub async fn install_missing_dependencies(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    game_version: &str,
    loader: &str,
    api_key: &str,
) -> Result<usize, LauncherError> {
    let missing = list_missing_dependencies(http, cache, paths, instance_id).await?;
    let mut installed = 0usize;
    for dep in missing {
        if is_installed(paths, instance_id, &dep.project_id).unwrap_or(false) {
            continue;
        }
        let pinned = dep.version_id.as_deref().filter(|value| !value.is_empty());
        let mut result = install_any(
            http,
            cache,
            paths,
            instance_id,
            &dep.project_id,
            pinned,
            game_version,
            loader,
            "mod",
            "modrinth",
            api_key,
        )
        .await;
        if result.is_err() && pinned.is_some() {
            result = install_any(
                http,
                cache,
                paths,
                instance_id,
                &dep.project_id,
                None,
                game_version,
                loader,
                "mod",
                "modrinth",
                api_key,
            )
            .await;
        }
        if result.is_ok() {
            installed += 1;
        }
    }
    Ok(installed)
}

async fn lookup_curseforge_id(
    http: &HttpPool,
    slug: &str,
    api_key: &str,
) -> Result<String, LauncherError> {
    let url = format!(
        "{CURSEFORGE_API}/mods/search?gameId=432&slug={}&pageSize=5",
        urlencoding::encode(slug)
    );
    let parsed: CfSearchIds = http.get_json(&url, &curseforge_headers(api_key)).await?;
    parsed
        .data
        .into_iter()
        .next()
        .map(|item| item.id.to_string())
        .ok_or_else(|| LauncherError::Message(format!("CurseForge has no project `{slug}`")))
}

async fn install_github(
    http: &HttpPool,
    cache: &MetadataCache,
    paths: &AppPaths,
    instance_id: &str,
    project_id: &str,
    game_version: &str,
    loader: &str,
    project_type: &str,
    api_key: &str,
) -> Result<Vec<String>, LauncherError> {
    let (owner, repo) = parse_github_repo(project_id)
        .ok_or_else(|| LauncherError::Message(format!("not a GitHub repo: {project_id}")))?;
    let mut errors = Vec::new();
    match install_modrinth(
        http,
        cache,
        paths,
        instance_id,
        &repo,
        None,
        game_version,
        loader,
        project_type,
    )
    .await
    {
        Ok(names) => return Ok(names),
        Err(err) => errors.push(format!("Modrinth: {err}")),
    }
    match install_curseforge_flexible(
        http,
        paths,
        instance_id,
        &repo,
        None,
        game_version,
        loader,
        project_type,
        api_key,
    )
    .await
    {
        Ok(names) => return Ok(names),
        Err(err) => errors.push(format!("CurseForge: {err}")),
    }
    match install_github_release(http, paths, instance_id, &owner, &repo, project_type).await {
        Ok(names) => return Ok(names),
        Err(err) => errors.push(format!("GitHub releases: {err}")),
    }
    Err(LauncherError::Message(format!(
        "could not install github.com/{owner}/{repo}: {}",
        errors.join(" | ")
    )))
}

async fn install_github_release(
    http: &HttpPool,
    paths: &AppPaths,
    instance_id: &str,
    owner: &str,
    repo: &str,
    project_type: &str,
) -> Result<Vec<String>, LauncherError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=12");
    let releases: Vec<GhRelease> = http.get_json(&url, &[]).await?;
    let asset = releases
        .into_iter()
        .flat_map(|release| release.assets)
        .find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.ends_with(".jar")
                && !name.contains("sources")
                && !name.contains("javadoc")
                && !name.contains("dev-shadow")
        })
        .ok_or_else(|| {
            LauncherError::Message(format!("no jar in GitHub releases for {owner}/{repo}"))
        })?;
    let folder = folder_for(project_type);
    let root = paths.instance_dir(instance_id);
    let dest = root.join(folder);
    let resolved = ResolvedFile {
        key: asset.id.to_string(),
        project_id: format!("github:{owner}/{repo}"),
        file_name: asset.name.clone(),
        url: asset.browser_download_url,
        sha1: None,
    };
    let names = download_files(http, &dest, &[resolved]).await?;
    remember(
        &root,
        &format!("github:{owner}/{repo}"),
        "github",
        folder,
        &names,
        None,
        Some(&format!("https://github.com/{owner}.png")),
    )?;
    Ok(names)
}

#[derive(Debug, Deserialize)]
struct CfSearchIds {
    #[serde(default)]
    data: Vec<CfSearchId>,
}

#[derive(Debug, Deserialize)]
struct CfSearchId {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
}

fn pick_curseforge_file(files: Vec<CfFile>, game_version: &str, loader: &str) -> Option<CfFile> {
    let game = game_version.trim();
    let loader = loader.trim().to_ascii_lowercase();
    files
        .into_iter()
        .filter(|item| !item.file_name.trim().is_empty())
        .max_by(|a, b| {
            let score = |item: &CfFile| {
                let mut score = 0i64;
                if !game.is_empty()
                    && item
                        .game_versions
                        .iter()
                        .any(|value| value.eq_ignore_ascii_case(game))
                {
                    score += 100;
                }
                if !loader.is_empty()
                    && loader != "vanilla"
                    && item.game_versions.iter().any(|value| {
                        let lower = value.to_ascii_lowercase();
                        lower == loader || lower.contains(&loader)
                    })
                {
                    score += 50;
                }
                if item
                    .download_url
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                {
                    score += 10;
                }
                // Prefer newest file when scores tie (id grows over time; date as tie-break).
                score += item.id as i64;
                score
            };
            score(a)
                .cmp(&score(b))
                .then_with(|| a.file_date.cmp(&b.file_date))
        })
}

#[derive(Debug, Deserialize)]
struct CfFiles {
    #[serde(default)]
    data: Vec<CfFile>,
}

#[derive(Debug, Deserialize)]
struct CfFileWrap {
    data: CfFile,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    file_date: Option<String>,
    download_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
}

#[derive(Debug, Deserialize)]
struct CfHash {
    #[serde(default)]
    value: String,
    #[serde(default)]
    algo: u8,
}

#[derive(Debug, Deserialize)]
struct CfDownload {
    data: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    fn dummy_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!("winmoding-content-{}", uuid::Uuid::new_v4()));
        AppPaths {
            instances: root.clone(),
            runtime: root.clone(),
            meta: root.clone(),
            libraries: root.clone(),
            assets: root.clone(),
            versions: root.clone(),
            database: root.join("x.db"),
            root,
        }
    }

    #[test]
    fn list_installed_rejects_unsafe_id() {
        let paths = dummy_paths();
        assert!(list_installed(&paths, "../etc").is_err());
        assert!(list_installed(&paths, "ok-instance").is_ok());
    }

    #[test]
    fn remove_content_rejects_traversal() {
        let paths = dummy_paths();
        assert!(remove_content(&paths, "ok-instance", "../secret.jar").is_err());
    }
}
