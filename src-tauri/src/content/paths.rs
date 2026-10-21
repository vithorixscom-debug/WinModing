use std::fs;
use std::path::{Path, PathBuf};

fn adopt(root: &Path, current: &str, leftovers: &[&str]) -> PathBuf {
    let dest = root.join(current);
    if dest.exists() {
        return dest;
    }
    for name in leftovers {
        let src = root.join(name);
        if src.exists() {
            if fs::rename(&src, &dest).is_ok() {
                return dest;
            }
            return src;
        }
    }
    dest
}

pub fn index_path(root: &Path) -> PathBuf {
    adopt(
        root,
        "winmoding.content.json",
        &["winmods.content.json", "winlauncher.content.json"],
    )
}

pub fn options_path(root: &Path) -> PathBuf {
    adopt(
        root,
        "winmoding.options.json",
        &["winmods.options.json", "winlauncher.options.json"],
    )
}

pub fn folder_for(project_type: &str) -> &'static str {
    match project_type {
        "resourcepack" | "resourcepacks" => "resourcepacks",
        "shader" | "shaderpack" | "shaderpacks" => "shaderpacks",
        "datapack" | "datapacks" => "datapacks",
        _ => "mods",
    }
}

pub fn relative_is_safe(path: &str) -> bool {
    let relative = path.replace('\\', "/");
    let relative = relative.trim_start_matches('/');
    !relative.is_empty() && !relative.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_reject_traversal() {
        assert!(relative_is_safe("mods/sodium.jar"));
        assert!(!relative_is_safe("../secret.jar"));
        assert!(!relative_is_safe("mods/../../x.jar"));
        assert!(!relative_is_safe(""));
    }

    #[test]
    fn index_path_renames_leftover_filename() {
        let root = std::env::temp_dir().join(format!("winmoding-adopt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("winlauncher.content.json"), b"{}").unwrap();
        let path = index_path(&root);
        assert_eq!(path.file_name().unwrap(), "winmoding.content.json");
        assert!(path.exists());
        assert!(!root.join("winlauncher.content.json").exists());
        let _ = fs::remove_dir_all(root);
    }
}
