use base64::Engine;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::domain::{
    instance_id_is_safe, CreateInstanceRequest, InstanceRecord, LoaderKind, ResolvedFile,
    VersionListEntry,
};
use crate::error::LauncherError;
use crate::java::{self};
use crate::launch::{self, InstallProgress, InstallReport, LaunchReceipt};
use crate::logs;
use crate::paths::ensure_instance_layout;
use crate::providers::{self, ModrinthApi};
use crate::state::AppState;

mod accounts;
mod catalog;
mod settings;
mod skins;
mod system;
pub use accounts::{
    __cmd__list_accounts, __cmd__login_ely, __cmd__login_msa, __cmd__login_offline,
    __cmd__remove_account, __tauri_command_name_list_accounts, __tauri_command_name_login_ely,
    __tauri_command_name_login_msa, __tauri_command_name_login_offline,
    __tauri_command_name_remove_account, list_accounts, login_ely, login_msa, login_offline,
    remove_account,
};
pub use catalog::{
    __cmd__catalog_get_json, __cmd__get_catalog_project, __cmd__get_catalog_version,
    __cmd__list_project_versions, __cmd__list_saved_catalog, __cmd__open_downloads_folder,
    __cmd__save_catalog_download, __cmd__search_catalog, __tauri_command_name_catalog_get_json,
    __tauri_command_name_get_catalog_project, __tauri_command_name_get_catalog_version,
    __tauri_command_name_list_project_versions, __tauri_command_name_list_saved_catalog,
    __tauri_command_name_open_downloads_folder, __tauri_command_name_save_catalog_download,
    __tauri_command_name_search_catalog, catalog_get_json, get_catalog_project, get_catalog_version,
    list_project_versions, list_saved_catalog, open_downloads_folder, save_catalog_download,
    search_catalog,
};
pub use settings::{
    __cmd__app_directory, __cmd__curseforge_key_set, __cmd__discord_set_idle,
    __cmd__get_launcher_settings, __cmd__get_max_memory, __cmd__is_dev,
    __cmd__microsoft_client_id_set, __cmd__pick_path, __cmd__save_curseforge_key,
    __cmd__save_microsoft_client_id, __cmd__set_launcher_settings, __cmd__should_disable_mouseover,
    __cmd__show_window, __tauri_command_name_app_directory,
    __tauri_command_name_curseforge_key_set, __tauri_command_name_discord_set_idle,
    __tauri_command_name_get_launcher_settings, __tauri_command_name_get_max_memory,
    __tauri_command_name_is_dev, __tauri_command_name_microsoft_client_id_set,
    __tauri_command_name_pick_path, __tauri_command_name_save_curseforge_key,
    __tauri_command_name_save_microsoft_client_id, __tauri_command_name_set_launcher_settings,
    __tauri_command_name_should_disable_mouseover, __tauri_command_name_show_window, app_directory,
    curseforge_key_set, discord_set_idle, get_launcher_settings, get_max_memory, is_dev,
    microsoft_client_id_set, pick_path, save_curseforge_key, save_microsoft_client_id,
    set_launcher_settings, should_disable_mouseover, show_window,
};
pub use skins::{
    __cmd__delete_skin, __cmd__fetch_skin_url, __cmd__list_skins, __cmd__ms_apply_skin,
    __cmd__ms_reset_skin, __cmd__ms_set_cape, __cmd__ms_skin_profile, __cmd__save_skin,
    __cmd__set_skin_slim, __tauri_command_name_delete_skin, __tauri_command_name_fetch_skin_url,
    __tauri_command_name_list_skins, __tauri_command_name_ms_apply_skin,
    __tauri_command_name_ms_reset_skin, __tauri_command_name_ms_set_cape,
    __tauri_command_name_ms_skin_profile, __tauri_command_name_save_skin,
    __tauri_command_name_set_skin_slim, delete_skin, fetch_skin_url, list_skins, ms_apply_skin,
    ms_reset_skin, ms_set_cape, ms_skin_profile, save_skin, set_skin_slim,
};
pub use system::{
    __cmd__drag_window, __cmd__list_icon_gallery, __cmd__list_recommended_servers,
    __cmd__open_external, __cmd__player_face, __cmd__player_face_remember, __cmd__proxy_media,
    __cmd__read_image_data_url, __cmd__server_status, __tauri_command_name_drag_window,
    __tauri_command_name_list_icon_gallery, __tauri_command_name_list_recommended_servers,
    __tauri_command_name_open_external, __tauri_command_name_player_face,
    __tauri_command_name_player_face_remember, __tauri_command_name_proxy_media,
    __tauri_command_name_read_image_data_url, __tauri_command_name_server_status, drag_window,
    list_icon_gallery, list_recommended_servers, open_external, player_face, player_face_remember,
    proxy_media, read_image_data_url, server_status,
};

fn json_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_f64().map(|item| item as u64))
        .or_else(|| value.as_str()?.parse().ok())
}

fn slug_id(name: &str) -> String {
    let mut slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
    let base = if slug.is_empty() { "instance" } else { slug };
    format!("{base}-{suffix}")
}

fn emit_profile_download(
    app: &AppHandle,
    name: &str,
    id: &str,
    fraction: Option<f64>,
    message: &str,
) {
    let _ = app.emit(
        "loading",
        serde_json::json!({
            "event": {
                "type": "MinecraftDownload",
                "profile_name": name,
                "profile_uuid": id,
                "profile_path": name,
            },
            "loader_uuid": id,
            "fraction": fraction.map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),
            "message": message,
        }),
    );
}

struct ProfileDownloadGuard {
    app: AppHandle,
    name: String,
    id: String,
}

impl Drop for ProfileDownloadGuard {
    fn drop(&mut self) {
        emit_profile_download(&self.app, &self.name, &self.id, None, "Installed");
    }
}

#[tauri::command]
pub async fn list_instances(
    state: State<'_, AppState>,
) -> Result<Vec<InstanceRecord>, LauncherError> {
    let cache = state.cache.clone();
    drop(state);
    cache.list_instances().await
}

fn build_instance_record(request: &CreateInstanceRequest) -> Result<InstanceRecord, LauncherError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(LauncherError::Instance {
            id: "new".into(),
            reason: "name must be 1-64 characters".into(),
        });
    }
    if request.game_version.trim().is_empty() {
        return Err(LauncherError::Instance {
            id: "new".into(),
            reason: "game version is required".into(),
        });
    }
    let id = slug_id(name);
    if !instance_id_is_safe(&id) {
        return Err(LauncherError::Instance {
            id,
            reason: "generated id was rejected".into(),
        });
    }
    Ok(InstanceRecord {
        id,
        name: name.to_string(),
        game_version: request.game_version.trim().to_string(),
        loader: LoaderKind::parse(&request.loader),
        loader_version: request.loader_version.clone(),
        java_major: request.java_major,
        created_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0),
        sort_order: 0,
    })
}

#[tauri::command]
pub async fn move_instance(
    state: State<'_, AppState>,
    id: String,
    direction: String,
) -> Result<Vec<InstanceRecord>, LauncherError> {
    state.cache.move_instance(&id, &direction).await
}

#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    request: CreateInstanceRequest,
) -> Result<InstanceRecord, LauncherError> {
    let record = build_instance_record(&request)?;
    let dir = state.paths.instance_dir(&record.id);
    ensure_instance_layout(&dir)?;
    let http = state.http.clone();
    apply_create_icon(&http, &dir, request.icon_path.as_deref()).await;
    let cache = state.cache.clone();
    drop(state);
    cache.insert_instance(record.clone()).await?;
    Ok(record)
}

fn remember_icon_path(dir: &std::path::Path, dest: &std::path::Path) {
    let mut extra = crate::content::read_options(dir);
    if let Some(map) = extra.as_object_mut() {
        map.insert(
            "iconPath".into(),
            serde_json::Value::String(dest.display().to_string()),
        );
    }
    let _ = crate::content::write_options(dir, &extra);
}

/// Apply instance icon from a local path, data URL, or trusted https CDN URL
/// (modpack / catalog icons from cdn.modrinth.com etc.).
async fn apply_create_icon(
    http: &crate::net::HttpPool,
    dir: &std::path::Path,
    icon: Option<&str>,
) {
    if let Some(src) = icon.map(str::trim).filter(|s| !s.is_empty()) {
        let dest = dir.join("icon.png");
        if src.starts_with("data:image") {
            if let Some(encoded) = src.split(',').next_back() {
                if let Ok(bytes) =
                    base64::engine::general_purpose::STANDARD.decode(encoded.trim())
                {
                    let _ = std::fs::write(&dest, bytes);
                }
            }
        } else if src.starts_with("https://") || src.starts_with("http://") {
            let https = if let Some(rest) = src.strip_prefix("http://") {
                format!("https://{rest}")
            } else {
                src.to_string()
            };
            if crate::protocol::assert_trusted_https(&https).is_ok() {
                let _ = http.download_to(&https, &dest).await;
            }
        } else {
            let path = std::path::Path::new(src);
            if path.is_file() {
                let _ = std::fs::copy(path, &dest);
            }
        }
        if dest.is_file() {
            remember_icon_path(dir, &dest);
            return;
        }
    }
    if !dir.join("icon.png").is_file() {
        let _ = crate::content::assign_random_icon(dir);
    }
}

/// Create an isolated instance and download the Minecraft client/libraries/assets.
#[tauri::command]
pub async fn create_instance_and_install(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateInstanceRequest,
) -> Result<InstanceRecord, LauncherError> {
    let mut record = build_instance_record(&request)?;
    let dir = state.paths.instance_dir(&record.id);
    ensure_instance_layout(&dir)?;
    let http = state.http.clone();
    apply_create_icon(&http, &dir, request.icon_path.as_deref()).await;
    let paths = state.paths.clone();
    let cache = state.cache.clone();
    drop(state);
    cache.insert_instance(record.clone()).await?;
    let _ = app.emit(
        "profile",
        serde_json::json!({
            "uuid": record.id,
            "name": record.name,
            "profile_path": record.id,
            "path": record.id,
            "event": "Created",
        }),
    );

    if request.skip_install {
        return Ok(record);
    }

    emit_profile_download(&app, &record.name, &record.id, Some(0.01), "Starting");
    let _download_done = ProfileDownloadGuard {
        app: app.clone(),
        name: record.name.clone(),
        id: record.id.clone(),
    };

    let version_id = record.game_version.clone();
    let report =
        launch::install_vanilla(&http, &paths, &version_id, |progress: InstallProgress| {
            let _ = app.emit("install-progress", &progress);
            let fraction = if progress.total == 0 {
                0.01
            } else {
                (progress.completed as f64 / progress.total as f64).clamp(0.01, 0.89)
            };
            emit_profile_download(
                &app,
                &record.name,
                &record.id,
                Some(fraction),
                &format!(
                    "{} ({}/{})",
                    progress.stage, progress.completed, progress.total
                ),
            );
        })
        .await?;

    cache
        .mark_version_installed(
            &report.version_id,
            "release",
            &paths.version_json(&report.version_id).display().to_string(),
        )
        .await?;

    if matches!(record.loader, LoaderKind::Fabric | LoaderKind::Quilt) {
        let resolved = launch::resolve_loader_version(
            &http,
            &record.game_version,
            record.loader,
            record.loader_version.as_deref(),
        )
        .await?;
        record.loader_version = Some(resolved.clone());
        cache.update_instance(record.clone()).await?;
        let document = launch::resolve_document(
            &http,
            &paths,
            &record.game_version,
            record.loader,
            Some(&resolved),
        )
        .await?;
        let host = launch::HostEnv::current();
        launch::ensure_libraries(&http, &paths, &document, &host).await?;
        emit_profile_download(
            &app,
            &record.name,
            &record.id,
            Some(0.85),
            &format!("{} loader {}", record.loader.as_str(), resolved),
        );
    }

    let java_major = record.java_major.unwrap_or(report.java_major);
    if java::select_runtime(&java::detect_java_runtimes().await?, java_major).is_err() {
        emit_profile_download(
            &app,
            &record.name,
            &record.id,
            Some(0.93),
            &format!("Java {java_major}"),
        );
        let _ = java::install_adoptium(&http, &paths, java_major).await;
    }

    if request.auto_optimize && matches!(record.loader, LoaderKind::Fabric | LoaderKind::Quilt) {
        emit_profile_download(
            &app,
            &record.name,
            &record.id,
            Some(0.94),
            "Optimization mods",
        );
        match crate::content::install_optimization_mods(
            &http,
            &cache,
            &paths,
            &record.id,
            &record.game_version,
            record.loader.as_str(),
        )
        .await
        {
            Ok(names) => {
                emit_profile_download(
                    &app,
                    &record.name,
                    &record.id,
                    Some(0.99),
                    &format!("{} optimization mods", names.len()),
                );
                if names.is_empty() {
                    let _ = app.emit(
                        "warning",
                        serde_json::json!({
                            "message": "Auto-optimization could not install mods (network or no matching builds). Try again or add mods from Browse.",
                        }),
                    );
                }
            }
            Err(err) => {
                log::warn!("auto-optimize failed for {}: {err}", record.id);
                let _ = app.emit(
                    "warning",
                    serde_json::json!({
                        "message": format!("Auto-optimization failed: {err}"),
                    }),
                );
            }
        }
    }

    drop(_download_done);
    let _ = app.emit(
        "profile",
        serde_json::json!({
            "uuid": record.id,
            "name": record.name,
            "profile_path": record.id,
            "path": record.id,
            "event": "Edited",
        }),
    );
    Ok(record)
}

#[tauri::command]
pub async fn list_game_versions(
    state: State<'_, AppState>,
) -> Result<Vec<VersionListEntry>, LauncherError> {
    let http = state.http.clone();
    drop(state);
    #[derive(serde::Deserialize)]
    struct Manifest {
        versions: Vec<VersionListEntry>,
    }
    let text = http
        .get_text(crate::protocol::VERSION_MANIFEST_V2, &[])
        .await?;
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|err| LauncherError::CorruptVersion {
            reason: format!("version manifest: {err}"),
        })?;
    Ok(manifest.versions)
}

#[tauri::command]
pub async fn instance_full_path(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, LauncherError> {
    if !instance_id_is_safe(&id) {
        return Err(LauncherError::Instance {
            id,
            reason: "unsafe instance id".into(),
        });
    }
    Ok(state.paths.instance_dir(&id).display().to_string())
}

#[tauri::command]
pub async fn instance_is_running(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, LauncherError> {
    Ok(state.games.contains(&id))
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<(), LauncherError> {
    if !instance_id_is_safe(&id) {
        return Err(LauncherError::Instance {
            id,
            reason: "unsafe instance id".into(),
        });
    }
    let cache = state.cache.clone();
    let dir = state.paths.instance_dir(&id);
    drop(state);
    cache.delete_instance(&id).await?;
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(|err| crate::error::io_at(&dir, err))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn detect_java() -> Result<Vec<crate::domain::JavaRuntime>, LauncherError> {
    java::detect_java_runtimes().await
}

#[tauri::command]
pub async fn install_java(
    state: State<'_, AppState>,
    major: u32,
) -> Result<crate::domain::JavaRuntime, LauncherError> {
    if !matches!(major, 8 | 17 | 21) {
        return Err(LauncherError::JavaMissing { major });
    }
    let http = state.http.clone();
    let paths = state.paths.clone();
    drop(state);
    java::install_adoptium(&http, &paths, major).await
}

#[tauri::command]
pub async fn install_game(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
) -> Result<InstallReport, LauncherError> {
    let version_id = version_id.trim().to_string();
    if version_id.is_empty() || version_id.contains(['/', '\\']) {
        return Err(LauncherError::CorruptVersion {
            reason: "version id is empty or contains a path separator".into(),
        });
    }
    let http = state.http.clone();
    let paths = state.paths.clone();
    let cache = state.cache.clone();
    drop(state);
    emit_profile_download(&app, &version_id, &version_id, Some(0.01), "Starting");
    let report =
        launch::install_vanilla(&http, &paths, &version_id, |progress: InstallProgress| {
            let _ = app.emit("install-progress", &progress);
            let fraction = if progress.total == 0 {
                0.01
            } else {
                (progress.completed as f64 / progress.total as f64).clamp(0.01, 0.99)
            };
            emit_profile_download(
                &app,
                &version_id,
                &version_id,
                Some(fraction),
                &format!(
                    "{} ({}/{})",
                    progress.stage, progress.completed, progress.total
                ),
            );
        })
        .await?;
    emit_profile_download(&app, &version_id, &version_id, None, "Installed");
    cache
        .mark_version_installed(
            &report.version_id,
            "release",
            &paths.version_json(&report.version_id).display().to_string(),
        )
        .await?;
    Ok(report)
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    account_id: String,
    xmx_mb: Option<u64>,
    server: Option<String>,
) -> Result<LaunchReceipt, LauncherError> {
    let cache = state.cache.clone();
    let paths = state.paths.clone();
    let games = state.games.clone();
    let http = state.http.clone();
    drop(state);
    let settings = crate::settings::load(&paths);
    let extra = crate::content::read_options(&paths.instance_dir(&instance_id));
    let heap = xmx_mb
        .or_else(|| {
            extra
                .get("memory")
                .and_then(|value| value.get("maximum").or_else(|| value.get("max")))
                .and_then(json_u64)
        })
        .or(Some(settings.memory_max));
    let account = cache
        .get_account(&account_id)
        .await?
        .ok_or_else(|| LauncherError::Auth {
            reason: "account was not found".into(),
        })?;
    let account = match crate::auth::ensure_microsoft_fresh(&http, account).await {
        Ok(fresh) => {
            let _ = cache.upsert_account(fresh.clone()).await;
            fresh
        }
        Err(err) => {
            log::warn!("Microsoft token refresh failed: {err}");
            return Err(err);
        }
    };
    // Hard check: Microsoft launch must carry a real Minecraft services JWT (not a UI stub).
    if account.kind == "microsoft"
        && (account.access_token.len() < 32 || account.access_token == "0")
    {
        return Err(LauncherError::Auth {
            reason: "Microsoft session is missing — sign in again before launching".into(),
        });
    }
    log::info!(
        "launch auth kind={} user_type={} token_len={} uuid={}",
        account.kind,
        match account.kind.as_str() {
            "microsoft" => "msa",
            "ely" => "mojang",
            _ => "legacy",
        },
        account.access_token.len(),
        account.uuid
    );
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let app_logs = app.clone();
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            let _ = app_logs.emit("game-log", line);
        }
    });
    let receipt = launch::launch_instance(
        &http,
        &paths,
        &cache,
        &games,
        &instance_id,
        &account,
        heap,
        server,
        tx,
    )
    .await?;
    if settings.hide_on_process_start {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.minimize();
        }
    }
    let name = extra
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("Minecraft")
        .to_string();
    let name = cache
        .get_instance(&instance_id)
        .await
        .ok()
        .flatten()
        .map(|record| record.name)
        .unwrap_or(name);
    if settings.discord_rpc {
        crate::discord::set_playing(&name, receipt.pid);
    }
    let games_watch = games.clone();
    let app_watch = app.clone();
    let id_watch = instance_id.clone();
    let paths_watch = paths.clone();
    tokio::spawn(async move {
        let mut ticks: u64 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            ticks += 1;
            if ticks % 10 == 0 {
                let _ = crate::content::add_play_seconds(&paths_watch.instance_dir(&id_watch), 15);
                let _ = app_watch.emit(
                    "profile",
                    serde_json::json!({
                        "uuid": id_watch,
                        "path": id_watch,
                        "profile_path": id_watch,
                        "profile_path_id": id_watch,
                        "event": "played",
                    }),
                );
            }
            if let Some(code) = games_watch.poll_exit(&id_watch) {
                let _ = crate::content::add_play_seconds(&paths_watch.instance_dir(&id_watch), 2);
                let _ = app_watch.emit(
                    "profile",
                    serde_json::json!({
                        "uuid": id_watch,
                        "path": id_watch,
                        "profile_path": id_watch,
                        "profile_path_id": id_watch,
                        "event": "played",
                    }),
                );
                if code != 0 {
                    let summary = crate::logs::analyze_crash(&paths_watch, &id_watch);
                    let _ = crate::content::set_last_crash(
                        &paths_watch.instance_dir(&id_watch),
                        &summary,
                    );
                    let _ = app_watch.emit(
                        "process",
                        serde_json::json!({
                            "uuid": id_watch,
                            "pid": 0,
                            "event": "crashed",
                            "message": summary,
                            "profile_path_id": id_watch,
                        }),
                    );
                }
                let _ = app_watch.emit(
                    "process",
                    serde_json::json!({
                        "uuid": id_watch,
                        "pid": 0,
                        "event": "finished",
                        "message": "Minecraft exited",
                        "profile_path_id": id_watch,
                    }),
                );
                crate::discord::set_idle();
                break;
            }
            if !games_watch.contains(&id_watch) {
                crate::discord::set_idle();
                break;
            }
        }
    });
    Ok(receipt)
}

#[tauri::command]
pub async fn stop_game(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<(), LauncherError> {
    let games = state.games.clone();
    drop(state);
    games.stop(&instance_id).await
}

#[tauri::command]
pub fn logs_get_logs(
    state: State<'_, AppState>,
    profile_path: String,
    clear_contents: Option<bool>,
) -> Result<Vec<logs::LogFile>, LauncherError> {
    logs::list_logs(&state.paths, &profile_path, clear_contents.unwrap_or(false))
}

#[tauri::command]
pub fn logs_get_logs_by_filename(
    state: State<'_, AppState>,
    profile_path: String,
    filename: String,
) -> Result<logs::LogFile, LauncherError> {
    let stdout = logs::read_log(&state.paths, &profile_path, &filename)?;
    Ok(logs::LogFile {
        filename,
        stdout,
        stderr: String::new(),
        log_type: "output".into(),
    })
}

#[tauri::command]
pub fn logs_get_output_by_filename(
    state: State<'_, AppState>,
    profile_path: String,
    filename: String,
) -> Result<String, LauncherError> {
    logs::read_log(&state.paths, &profile_path, &filename)
}

#[tauri::command]
pub fn logs_delete_logs_by_filename(
    state: State<'_, AppState>,
    profile_path: String,
    filename: String,
) -> Result<(), LauncherError> {
    logs::delete_log(&state.paths, &profile_path, &filename)
}

#[tauri::command]
pub fn logs_delete_logs(
    state: State<'_, AppState>,
    profile_path: String,
) -> Result<(), LauncherError> {
    logs::delete_all_logs(&state.paths, &profile_path)
}

#[tauri::command]
pub fn logs_get_latest_log_cursor(
    state: State<'_, AppState>,
    profile_path: String,
    cursor: u64,
) -> Result<logs::LatestLogCursor, LauncherError> {
    logs::latest_cursor(&state.paths, &profile_path, cursor)
}

#[tauri::command]
pub async fn resolve_dependencies(
    state: State<'_, AppState>,
    version_id: String,
    game_version: String,
    loader: String,
) -> Result<Vec<ResolvedFile>, LauncherError> {
    let api = ModrinthApi::new(state.http.clone(), state.cache.clone());
    drop(state);
    providers::resolve_required(&api, version_id.trim(), game_version.trim(), loader.trim()).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallContentRequest {
    pub instance_id: String,
    pub project_id: String,
    pub version_id: Option<String>,
    pub source: String,
    pub project_type: Option<String>,
}

#[tauri::command]
pub async fn install_content(
    state: State<'_, AppState>,
    request: InstallContentRequest,
) -> Result<Vec<String>, LauncherError> {
    if !instance_id_is_safe(&request.instance_id) {
        return Err(LauncherError::Instance {
            id: request.instance_id,
            reason: "unsafe instance id".into(),
        });
    }
    let http = state.http.clone();
    let cache = state.cache.clone();
    let paths = state.paths.clone();
    let cf_key = state.paths.read_curseforge_key();
    drop(state);
    let instance = cache
        .get_instance(&request.instance_id)
        .await?
        .ok_or_else(|| LauncherError::Instance {
            id: request.instance_id.clone(),
            reason: "not found".into(),
        })?;
    let project_type = request.project_type.unwrap_or_else(|| "mod".into());
    crate::content::install_any(
        &http,
        &cache,
        &paths,
        &request.instance_id,
        &request.project_id,
        request.version_id.as_deref(),
        &instance.game_version,
        instance.loader.as_str(),
        &project_type,
        &request.source,
        &cf_key,
    )
    .await
}

#[tauri::command]
pub async fn add_project_from_version(
    state: State<'_, AppState>,
    instance_id: String,
    version_id: String,
) -> Result<Vec<String>, LauncherError> {
    let http = state.http.clone();
    let cache = state.cache.clone();
    let paths = state.paths.clone();
    drop(state);
    let instance =
        cache
            .get_instance(&instance_id)
            .await?
            .ok_or_else(|| LauncherError::Instance {
                id: instance_id.clone(),
                reason: "not found".into(),
            })?;
    crate::content::install_from_version(
        &http,
        &cache,
        &paths,
        &instance_id,
        &version_id,
        &instance.game_version,
        instance.loader.as_str(),
    )
    .await
}

#[tauri::command]
pub fn list_installed_content(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<crate::content::InstalledContent>, LauncherError> {
    crate::content::list_installed(&state.paths, &instance_id)
}

#[tauri::command]
pub fn content_is_installed(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
) -> Result<bool, LauncherError> {
    crate::content::is_installed(&state.paths, &instance_id, &project_id)
}

#[tauri::command]
pub fn remove_content(
    state: State<'_, AppState>,
    instance_id: String,
    project_path: String,
) -> Result<(), LauncherError> {
    crate::content::remove_content(&state.paths, &instance_id, &project_path)
}

#[tauri::command]
pub fn toggle_disable_content(
    state: State<'_, AppState>,
    instance_id: String,
    project_path: String,
) -> Result<String, LauncherError> {
    crate::content::toggle_disable(&state.paths, &instance_id, &project_path)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePatch {
    pub name: Option<String>,
    #[serde(alias = "game_version")]
    pub game_version: Option<String>,
    pub loader: Option<String>,
    #[serde(alias = "loader_version")]
    pub loader_version: Option<String>,
    pub groups: Option<Vec<String>>,
    #[serde(alias = "java_path")]
    pub java_path: Option<String>,
    #[serde(alias = "extra_launch_args")]
    pub extra_launch_args: Option<Vec<String>>,
    #[serde(alias = "custom_env_vars")]
    pub custom_env_vars: Option<Vec<Vec<String>>>,
    pub memory: Option<serde_json::Value>,
    /// Frontend sends camelCase `iconPath` (see profile.js edit_icon).
    #[serde(default, alias = "iconPath")]
    pub icon_path: Option<String>,
    #[serde(alias = "force_fullscreen")]
    pub force_fullscreen: Option<bool>,
    #[serde(alias = "game_resolution")]
    pub game_resolution: Option<serde_json::Value>,
    pub hooks: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn update_instance(
    state: State<'_, AppState>,
    id: String,
    patch: InstancePatch,
) -> Result<InstanceRecord, LauncherError> {
    if !instance_id_is_safe(&id) {
        return Err(LauncherError::Instance {
            id,
            reason: "unsafe instance id".into(),
        });
    }
    let cache = state.cache.clone();
    let root = state.paths.instance_dir(&id);
    drop(state);
    let mut record = cache
        .get_instance(&id)
        .await?
        .ok_or_else(|| LauncherError::Instance {
            id: id.clone(),
            reason: "not found".into(),
        })?;
    if let Some(name) = patch
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record.name = name.chars().take(64).collect();
    }
    if let Some(game) = patch
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record.game_version = game.to_string();
    }
    if let Some(loader) = patch.loader.as_deref() {
        record.loader = LoaderKind::parse(loader);
    }
    if patch.loader_version.is_some() {
        record.loader_version = patch.loader_version.clone();
    }
    cache.update_instance(record.clone()).await?;
    let mut extra = crate::content::read_options(&root);
    let obj = extra.as_object_mut().cloned().unwrap_or_default();
    let mut map = obj;
    if let Some(groups) = patch.groups {
        map.insert(
            "groups".into(),
            serde_json::to_value(groups).unwrap_or_default(),
        );
    }
    if let Some(java_path) = patch.java_path {
        if java_path.trim().is_empty() {
            map.remove("javaPath");
        } else {
            map.insert("javaPath".into(), serde_json::Value::String(java_path));
        }
    }
    if let Some(args) = patch.extra_launch_args {
        if args.is_empty() {
            map.remove("extraLaunchArgs");
        } else {
            map.insert(
                "extraLaunchArgs".into(),
                serde_json::to_value(args).unwrap_or_default(),
            );
        }
    }
    if let Some(memory) = patch.memory {
        if memory.is_null() {
            map.remove("memory");
        } else {
            map.insert("memory".into(), memory);
        }
    }
    if let Some(icon) = patch.icon_path {
        if icon.trim().is_empty() {
            map.remove("iconPath");
            map.remove("iconDataUrl");
            let _ = std::fs::remove_file(root.join("icon.png"));
        } else {
            let dest = root.join("icon.png");
            let src = std::path::Path::new(&icon);
            if src != dest.as_path() && src.is_file() {
                let _ = std::fs::copy(src, &dest);
            }
            let stored = if dest.is_file() {
                dest
            } else {
                src.to_path_buf()
            };
            map.insert(
                "iconPath".into(),
                serde_json::Value::String(stored.display().to_string()),
            );
        }
    }
    if let Some(vars) = patch.custom_env_vars {
        if vars.is_empty() {
            map.remove("customEnvVars");
        } else {
            map.insert(
                "customEnvVars".into(),
                serde_json::to_value(vars).unwrap_or_default(),
            );
        }
    }
    if let Some(fullscreen) = patch.force_fullscreen {
        map.insert(
            "forceFullscreen".into(),
            serde_json::Value::Bool(fullscreen),
        );
    }
    if let Some(resolution) = patch.game_resolution {
        if resolution.is_null() {
            map.remove("gameResolution");
            map.remove("forceFullscreen");
        } else {
            map.insert(
                "gameResolution".into(),
                serde_json::to_value(resolution).unwrap_or_default(),
            );
        }
    }
    if let Some(hooks) = patch.hooks {
        if hooks.is_null() {
            map.remove("hooks");
        } else {
            map.insert("hooks".into(), hooks);
        }
    }
    crate::content::write_options(&root, &serde_json::Value::Object(map))?;
    Ok(record)
}

#[tauri::command]
pub fn instance_options(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, LauncherError> {
    let root = state.paths.instance_dir(&id);
    if !root.join("icon.png").is_file() {
        let _ = crate::content::assign_random_icon(&root);
    }
    let mut extra = crate::content::read_options(&root);
    crate::content::attach_icon_data(&root, &mut extra);
    Ok(extra)
}

#[tauri::command]
pub fn save_instance_options(
    state: State<'_, AppState>,
    id: String,
    options: serde_json::Value,
) -> Result<(), LauncherError> {
    crate::content::write_options(&state.paths.instance_dir(&id), &options)
}

#[tauri::command]
pub fn save_instance_icon_png(
    state: State<'_, AppState>,
    id: String,
    data_url: String,
) -> Result<String, LauncherError> {
    let encoded = data_url.split(',').next_back().unwrap_or_default().trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    let dest = if id.is_empty() || id == "gallery" {
        let dir = state.paths.root.join("icon-gallery");
        std::fs::create_dir_all(&dir).map_err(|err| crate::error::io_at(&dir, err))?;
        dir.join(format!("{}.png", uuid::Uuid::new_v4()))
    } else {
        if !instance_id_is_safe(&id) {
            return Err(LauncherError::Instance {
                id,
                reason: "unsafe instance id".into(),
            });
        }
        let root = state.paths.instance_dir(&id);
        std::fs::create_dir_all(&root).map_err(|err| crate::error::io_at(&root, err))?;
        root.join("icon.png")
    };
    std::fs::write(&dest, bytes).map_err(|err| crate::error::io_at(&dest, err))?;
    Ok(dest.display().to_string())
}

#[tauri::command]
pub fn analyze_crash(state: State<'_, AppState>, id: String) -> String {
    crate::logs::analyze_crash(&state.paths, &id)
}

#[tauri::command]
pub fn add_play_seconds(
    state: State<'_, AppState>,
    id: String,
    seconds: u64,
) -> Result<(), LauncherError> {
    crate::content::add_play_seconds(&state.paths.instance_dir(&id), seconds)
}

#[tauri::command]
pub fn ignore_mod_update(
    state: State<'_, AppState>,
    id: String,
    project_id: String,
    version_id: String,
) -> Result<(), LauncherError> {
    crate::content::ignore_mod_update(&state.paths.instance_dir(&id), &project_id, &version_id)
}

#[tauri::command]
pub async fn list_missing_dependencies(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<crate::content::MissingDependency>, LauncherError> {
    crate::content::list_missing_dependencies(&state.http, &state.cache, &state.paths, &id).await
}

#[tauri::command]
pub async fn install_missing_dependencies(
    state: State<'_, AppState>,
    id: String,
) -> Result<usize, LauncherError> {
    let record = state
        .cache
        .get_instance(&id)
        .await?
        .ok_or_else(|| LauncherError::Instance {
            id: id.clone(),
            reason: "not found".into(),
        })?;
    let key = state.paths.read_curseforge_key();
    crate::content::install_missing_dependencies(
        &state.http,
        &state.cache,
        &state.paths,
        &id,
        &record.game_version,
        record.loader.as_str(),
        &key,
    )
    .await
}

#[tauri::command]
pub async fn check_content_updates(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<crate::content::ContentUpdate>, LauncherError> {
    let record = state
        .cache
        .get_instance(&id)
        .await?
        .ok_or_else(|| LauncherError::Instance {
            id: id.clone(),
            reason: "not found".into(),
        })?;
    let key = state.paths.read_curseforge_key();
    crate::content::content_updates(
        &state.http,
        &state.cache,
        &state.paths,
        &id,
        &record.game_version,
        record.loader.as_str(),
        &key,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_id_is_safe_instance_id() {
        let id = slug_id("My Pack!");
        assert!(instance_id_is_safe(&id), "{id}");
        assert!(!id.contains(".."));
        assert!(!id.contains('/'));
    }

    #[test]
    fn slug_id_falls_back_for_empty_name() {
        let id = slug_id("***");
        assert!(id.starts_with("instance-"));
        assert!(instance_id_is_safe(&id), "{id}");
    }

    #[test]
    fn unsafe_instance_ids_are_rejected() {
        for id in [
            "../etc",
            "..\\windows",
            "a/b",
            "C:\\Windows",
            "has space",
            "",
        ] {
            assert!(!instance_id_is_safe(id), "{id}");
        }
    }
}
