use serde::{Deserialize, Serialize};

use crate::error::{io_at, LauncherError};
use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSettings {
    pub pre_launch: Option<String>,
    pub wrapper: Option<String>,
    pub post_exit: Option<String>,
}

impl Default for HookSettings {
    fn default() -> Self {
        Self {
            pre_launch: None,
            wrapper: None,
            post_exit: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherSettings {
    pub memory_max: u64,
    pub extra_launch_args: Vec<String>,
    pub custom_env_vars: Vec<(String, String)>,
    pub hooks: HookSettings,
    pub force_fullscreen: bool,
    pub game_resolution: [u32; 2],
    pub hide_on_process_start: bool,
    pub discord_rpc: bool,
    pub java_paths: serde_json::Map<String, serde_json::Value>,
    pub show_both_catalogs: bool,
    /// `modrinth`, `curseforge`, or `both`.
    #[serde(default)]
    pub catalog_source: String,
    /// When catalog_source is both: `modrinth`, `curseforge`, or `both`.
    #[serde(default)]
    pub catalog_duplicate_pref: String,
    #[serde(default)]
    pub catalog_page_size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_dir: Option<String>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            memory_max: 4096,
            extra_launch_args: Vec::new(),
            custom_env_vars: Vec::new(),
            hooks: HookSettings::default(),
            force_fullscreen: false,
            game_resolution: [1280, 720],
            hide_on_process_start: false,
            discord_rpc: true,
            java_paths: serde_json::Map::new(),
            show_both_catalogs: false,
            catalog_source: "modrinth".into(),
            catalog_duplicate_pref: "both".into(),
            catalog_page_size: 20,
            custom_dir: None,
        }
    }
}

fn settings_path(paths: &AppPaths) -> std::path::PathBuf {
    paths.root.join("winmoding.settings.json")
}

pub fn default_heap_mb() -> u64 {
    let total = crate::java::total_memory_mb().max(2048);
    (total / 4).clamp(1024, 16384)
}

pub fn load(paths: &AppPaths) -> LauncherSettings {
    let path = settings_path(paths);
    if !path.exists() {
        let mut value = LauncherSettings::default();
        value.memory_max = default_heap_mb();
        let _ = save(paths, &value);
        return value;
    }
    let mut settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_else(|| {
            let mut value = LauncherSettings::default();
            value.memory_max = default_heap_mb();
            value
        });
    if settings.memory_max < 512 {
        settings.memory_max = default_heap_mb();
    }
    let mut dirty = false;
    if settings.catalog_source.trim().is_empty() {
        settings.catalog_source = if settings.show_both_catalogs {
            "both".into()
        } else {
            "modrinth".into()
        };
        dirty = true;
    }
    if settings.catalog_duplicate_pref.trim().is_empty() {
        settings.catalog_duplicate_pref = if settings.show_both_catalogs {
            "both".into()
        } else {
            "modrinth".into()
        };
        dirty = true;
    }
    if ![5, 10, 15, 20, 50, 100].contains(&settings.catalog_page_size) {
        settings.catalog_page_size = 20;
        dirty = true;
    }
    settings.show_both_catalogs = settings.catalog_source.eq_ignore_ascii_case("both");
    if dirty {
        let _ = save(paths, &settings);
    }
    settings
}

pub fn save(paths: &AppPaths, settings: &LauncherSettings) -> Result<(), LauncherError> {
    let path = settings_path(paths);
    let body = serde_json::to_string_pretty(settings)
        .map_err(|err| LauncherError::Message(err.to_string()))?;
    std::fs::write(&path, body).map_err(|err| io_at(&path, err))
}

pub fn java_path_for(settings: &LauncherSettings, major: u32) -> Option<String> {
    settings
        .java_paths
        .get(&major.to_string())
        .and_then(|value| value.get("path").or(Some(value)))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
