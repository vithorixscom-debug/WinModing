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
mod system;
pub use accounts::{
    __cmd__list_accounts, __cmd__login_ely, __cmd__login_msa, __cmd__login_offline,
    __cmd__remove_account, __tauri_command_name_list_accounts, __tauri_command_name_login_ely,
    __tauri_command_name_login_msa, __tauri_command_name_login_offline,
    __tauri_command_name_remove_account, list_accounts, login_ely, login_msa, login_offline,
    remove_account,
};
pub use catalog::{
    __cmd__get_catalog_project, __cmd__list_project_versions, __cmd__search_catalog,
    __tauri_command_name_get_catalog_project, __tauri_command_name_list_project_versions,
    __tauri_command_name_search_catalog, get_catalog_project, list_project_versions,
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
pub use system::{
    __cmd__drag_window, __cmd__list_icon_gallery, __cmd__open_external,
    __tauri_command_name_drag_window, __tauri_command_name_list_icon_gallery,
    __tauri_command_name_open_external, drag_window, list_icon_gallery, open_external,
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
    })
}

#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    request: CreateInstanceRequest,
) -> Result<InstanceRecord, LauncherError> {
    let record = build_instance_record(&request)?;
    let dir = state.paths.instance_dir(&record.id);
    ensure_instance_layout(&dir)?;
    let cache = state.cache.clone();
    drop(state);
    cache.insert_instance(record.clone()).await?;
    Ok(record)
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

    drop(_download_done);

    if request.auto_optimize && matches!(record.loader, LoaderKind::Fabric | LoaderKind::Quilt) {
        let http = http.clone();
        let cache = cache.clone();
        let paths = paths.clone();
        let app = app.clone();
        let instance_id = record.id.clone();
        let instance_name = record.name.clone();
        let game_version = record.game_version.clone();
        let loader = record.loader.as_str().to_string();
        tokio::spawn(async move {
            if let Err(err) = crate::content::install_optimization_mods(
                &http,
                &cache,
                &paths,
                &instance_id,
                &game_version,
                &loader,
            )
            .await
            {
                log::warn!("auto-optimize failed for {instance_id}: {err}");
            }
            let _ = app.emit(
                "profile",
                serde_json::json!({
                    "uuid": instance_id,
                    "name": instance_name,
                    "profile_path": instance_id,
                    "profile_path_id": instance_id,
                    "path": instance_id,
                    "event": "Edited",
                }),
            );
        });
    }
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
    #[serde(default)]
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
