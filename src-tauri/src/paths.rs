use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{io_at, LauncherError};

pub const APP_FOLDER: &str = "WinModing";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub instances: PathBuf,
    pub runtime: PathBuf,
    pub meta: PathBuf,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub versions: PathBuf,
    pub database: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self, LauncherError> {
        let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let default_root = local.join(APP_FOLDER);
        adopt_legacy_sqlite(&default_root);
        adopt_named_file(
            &default_root,
            "winmoding.settings.json",
            &["winmods.settings.json", "winlauncher.settings.json"],
        );
        let custom = read_custom_dir(&default_root).and_then(sanitize_custom_dir);
        let root = custom.unwrap_or(default_root);
        if root != local.join(APP_FOLDER) {
            adopt_legacy_sqlite(&root);
            adopt_named_file(
                &root,
                "winmoding.settings.json",
                &["winmods.settings.json", "winlauncher.settings.json"],
            );
        }
        let database = root.join("winmoding.db");
        let paths = Self {
            instances: root.join("instances"),
            runtime: root.join("runtime"),
            meta: root.join("meta"),
            libraries: root.join("meta").join("libraries"),
            assets: root.join("meta").join("assets"),
            versions: root.join("meta").join("versions"),
            database,
            root,
        };
        paths.ensure()?;
        Ok(paths)
    }

    pub fn ensure(&self) -> Result<(), LauncherError> {
        for dir in [
            &self.root,
            &self.instances,
            &self.runtime,
            &self.meta,
            &self.libraries,
            &self.assets,
            &self.versions,
            &self.assets.join("indexes"),
            &self.assets.join("objects"),
        ] {
            fs::create_dir_all(dir).map_err(|err| io_at(dir, err))?;
        }
        Ok(())
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances.join(id)
    }

    pub fn version_json(&self, version_id: &str) -> PathBuf {
        self.versions
            .join(version_id)
            .join(format!("{version_id}.json"))
    }

    pub fn version_jar(&self, version_id: &str) -> PathBuf {
        self.versions
            .join(version_id)
            .join(format!("{version_id}.jar"))
    }

    pub fn loader_json(&self, loader: &str, game_version: &str, loader_version: &str) -> PathBuf {
        let id = format!("{loader}-{loader_version}-{game_version}");
        self.versions.join(&id).join(format!("{id}.json"))
    }

    pub fn java_home(&self, major: u32) -> PathBuf {
        self.runtime.join(format!("java-{major}"))
    }

    pub fn read_curseforge_key(&self) -> String {
        if let Some(key) = crate::protocol::env_nonempty("WINMODING_CF_KEY") {
            return key;
        }
        self.read_secret_file("curseforge.key")
    }

    pub fn write_curseforge_key(&self, key: &str) -> Result<(), LauncherError> {
        self.write_secret_file("curseforge.key", key)
    }

    pub fn read_microsoft_client_id(&self) -> String {
        self.require_microsoft_client_id().unwrap_or_default()
    }

    pub fn require_microsoft_client_id(&self) -> Result<String, LauncherError> {
        if let Some(value) = crate::protocol::env_nonempty("WINMODING_MS_CLIENT_ID") {
            return Ok(value);
        }
        Ok(self.read_secret_file("microsoft.client_id"))
    }

    pub fn write_microsoft_client_id(&self, key: &str) -> Result<(), LauncherError> {
        self.write_secret_file("microsoft.client_id", key)
    }

    fn read_secret_file(&self, name: &str) -> String {
        std::fs::read_to_string(self.meta.join(name))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
    }

    fn write_secret_file(&self, name: &str, key: &str) -> Result<(), LauncherError> {
        let path = self.meta.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| io_at(parent, err))?;
        }
        let trimmed = key.trim();
        if trimmed.is_empty() {
            if path.exists() {
                fs::remove_file(&path).map_err(|err| io_at(&path, err))?;
            }
            return Ok(());
        }
        fs::write(&path, trimmed).map_err(|err| io_at(&path, err))
    }
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn adopt_named_file(root: &Path, current: &str, leftovers: &[&str]) {
    let dest = root.join(current);
    for name in leftovers {
        let src = root.join(name);
        if !src.exists() {
            continue;
        }
        if !dest.exists() {
            let _ = fs::rename(&src, &dest);
            continue;
        }
        if file_len(&src) > file_len(&dest) {
            let backup = root.join(format!("{current}.bak"));
            let _ = fs::rename(&dest, &backup);
            let _ = fs::rename(&src, &dest);
        }
    }
}

fn adopt_legacy_sqlite(root: &Path) {
    if !root.exists() {
        return;
    }
    let dest = root.join("winmoding.db");
    for stem in ["winmods.db", "winlauncher.db"] {
        let src = root.join(stem);
        if !src.exists() {
            continue;
        }
        let dest_len = file_len(&dest);
        let src_len = file_len(&src);
        if dest.exists() && dest_len >= src_len && dest_len > 32_768 {
            continue;
        }
        if dest.exists() {
            let _ = fs::rename(&dest, root.join("winmoding.db.empty"));
            let _ = fs::rename(
                root.join("winmoding.db-wal"),
                root.join("winmoding.db.empty-wal"),
            );
            let _ = fs::rename(
                root.join("winmoding.db-shm"),
                root.join("winmoding.db.empty-shm"),
            );
        }
        let _ = fs::rename(&src, &dest);
        let _ = fs::rename(
            root.join(format!("{stem}-wal")),
            root.join("winmoding.db-wal"),
        );
        let _ = fs::rename(
            root.join(format!("{stem}-shm")),
            root.join("winmoding.db-shm"),
        );
    }
}

fn sanitize_custom_dir(dir: PathBuf) -> Option<PathBuf> {
    let name = dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name.eq_ignore_ascii_case("WinMods") || name.eq_ignore_ascii_case("WinLauncher") {
        return None;
    }
    Some(dir)
}

fn read_custom_dir(default_root: &Path) -> Option<PathBuf> {
    let body = fs::read_to_string(default_root.join("winmoding.settings.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value
        .get("customDir")
        .or_else(|| value.get("custom_dir"))
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(PathBuf::from)
}

pub fn ensure_instance_layout(root: &Path) -> Result<(), LauncherError> {
    for name in ["mods", "config", "saves", "resourcepacks", "shaderpacks"] {
        let dir = root.join(name);
        fs::create_dir_all(&dir).map_err(|err| io_at(&dir, err))?;
    }
    let options = root.join("options.txt");
    if !options.exists() {
        fs::write(&options, "lang:en_us\n").map_err(|err| io_at(&options, err))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_folder_is_winmoding() {
        assert_eq!(APP_FOLDER, "WinModing");
    }

    #[test]
    fn custom_dir_ignores_old_product_folders() {
        assert!(
            sanitize_custom_dir(PathBuf::from(r"C:\Users\x\AppData\Local\WinLauncher")).is_none()
        );
        assert!(sanitize_custom_dir(PathBuf::from(r"D:\Games\WinModing")).is_some());
    }
}
