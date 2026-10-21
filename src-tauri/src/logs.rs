use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::domain::instance_id_is_safe;
use crate::error::{io_at, LauncherError};
use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize)]
pub struct LogFile {
    pub filename: String,
    pub stdout: String,
    pub stderr: String,
    pub log_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LatestLogCursor {
    pub cursor: u64,
    pub output: String,
    pub new_file: bool,
}

fn logs_dir(paths: &AppPaths, instance_id: &str) -> Result<PathBuf, LauncherError> {
    if !instance_id_is_safe(instance_id) {
        return Err(LauncherError::Instance {
            id: instance_id.to_string(),
            reason: "unsafe instance id".into(),
        });
    }
    Ok(paths.instance_dir(instance_id).join("logs"))
}

fn is_log_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".log") || lower.ends_with(".txt") || lower.ends_with(".log.gz")
}

fn is_gzip_log(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("gz"))
        .unwrap_or(false)
}

fn decode_log_bytes(path: &Path, bytes: Vec<u8>) -> Result<String, LauncherError> {
    let decoded = if is_gzip_log(path) {
        let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|err| io_at(path, err))?;
        out
    } else {
        bytes
    };
    Ok(String::from_utf8_lossy(&decoded).into_owned())
}

fn read_log_file(path: &Path) -> Result<String, LauncherError> {
    let bytes = std::fs::read(path).map_err(|err| io_at(path, err))?;
    decode_log_bytes(path, bytes)
}

pub fn analyze_crash(paths: &AppPaths, instance_id: &str) -> String {
    let excerpt = crash_excerpt(paths, instance_id);
    let lower = excerpt.to_ascii_lowercase();
    let mut reasons: Vec<&str> = Vec::new();
    if lower.contains("incompatible") || lower.contains("modresolutionexception") {
        reasons.push("A mod is incompatible with this Minecraft/loader version.");
    }
    if lower.contains("mixin") && (lower.contains("failed") || lower.contains("error")) {
        reasons.push("A mixin failed — two mods are likely patching the same class.");
    }
    if lower.contains("duplicate") && lower.contains("mod") {
        reasons.push("Duplicate mods are installed (same mod twice).");
    }
    if lower.contains("requires") && (lower.contains("mod") || lower.contains("fabric")) {
        reasons.push("A required dependency is missing or the wrong version.");
    }
    if lower.contains("not found")
        || lower.contains("noclassdeffound")
        || lower.contains("classnotfound")
    {
        reasons.push("A class/file was not found — download may be incomplete.");
    }
    if lower.contains("zipexception")
        || lower.contains("invalid cen")
        || lower.contains("truncated")
    {
        reasons.push("A jar is corrupted or was not fully downloaded.");
    }
    if lower.contains("outofmemory") || lower.contains("java heap") {
        reasons.push("The game ran out of memory.");
    }
    if lower.contains("failed to load") || lower.contains("exception in thread") {
        reasons.push("The game crashed while loading mods.");
    }
    if reasons.is_empty() {
        if excerpt.contains("latest.log") {
            return excerpt;
        }
        reasons.push("Fabric/game exited with an error. Check the log excerpt below.");
    }
    format!("{}\n\n--- latest.log ---\n{excerpt}", reasons.join("\n"))
}

pub fn crash_excerpt(paths: &AppPaths, instance_id: &str) -> String {
    let path = match logs_dir(paths, instance_id) {
        Ok(dir) => dir.join("latest.log"),
        Err(_) => return "latest.log is unavailable".into(),
    };
    if !path.exists() {
        return "latest.log was not written".into();
    }
    match read_log_file(&path) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(40);
            let tail = lines[start..].join("\n");
            if tail.trim().is_empty() {
                "latest.log is empty".into()
            } else {
                tail
            }
        }
        Err(err) => format!("could not read latest.log: {err}"),
    }
}

pub fn list_logs(
    paths: &AppPaths,
    instance_id: &str,
    clear_contents: bool,
) -> Result<Vec<LogFile>, LauncherError> {
    let dir = logs_dir(paths, instance_id)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|err| io_at(&dir, err))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.path()));
    let mut logs = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_log_name(&name) || name.eq_ignore_ascii_case("latest.log") {
            continue;
        }
        let stdout = if clear_contents {
            String::new()
        } else {
            read_log_file(&entry.path()).unwrap_or_default()
        };
        logs.push(LogFile {
            filename: name,
            stdout,
            stderr: String::new(),
            log_type: "output".into(),
        });
    }
    Ok(logs)
}

pub fn read_log(
    paths: &AppPaths,
    instance_id: &str,
    filename: &str,
) -> Result<String, LauncherError> {
    if filename.contains(['/', '\\']) {
        return Err(LauncherError::Message("unsafe log filename".into()));
    }
    let path = logs_dir(paths, instance_id)?.join(filename);
    if !path.exists() {
        return Ok(String::new());
    }
    read_log_file(&path)
}

pub fn delete_log(
    paths: &AppPaths,
    instance_id: &str,
    filename: &str,
) -> Result<(), LauncherError> {
    if filename.contains(['/', '\\']) {
        return Err(LauncherError::Message("unsafe log filename".into()));
    }
    let path = logs_dir(paths, instance_id)?.join(filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|err| io_at(&path, err))?;
    }
    Ok(())
}

pub fn delete_all_logs(paths: &AppPaths, instance_id: &str) -> Result<(), LauncherError> {
    let dir = logs_dir(paths, instance_id)?;
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir).map_err(|err| io_at(&dir, err))? {
        let entry = entry.map_err(|err| io_at(&dir, err))?;
        let path = entry.path();
        if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

pub fn latest_cursor(
    paths: &AppPaths,
    instance_id: &str,
    cursor: u64,
) -> Result<LatestLogCursor, LauncherError> {
    let path = logs_dir(paths, instance_id)?.join("latest.log");
    if !path.exists() {
        return Ok(LatestLogCursor {
            cursor: 0,
            output: String::new(),
            new_file: cursor > 0,
        });
    }
    let bytes = std::fs::read(&path).map_err(|err| io_at(&path, err))?;
    let len = bytes.len() as u64;
    if cursor > len {
        return Ok(LatestLogCursor {
            cursor: len,
            output: String::from_utf8_lossy(&bytes).into_owned(),
            new_file: true,
        });
    }
    Ok(LatestLogCursor {
        cursor: len,
        output: String::from_utf8_lossy(&bytes[cursor as usize..]).into_owned(),
        new_file: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!("winmoding-logs-{}", uuid::Uuid::new_v4()));
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
    fn list_logs_rejects_unsafe_id() {
        let paths = dummy_paths();
        assert!(list_logs(&paths, "../etc", false).is_err());
        assert!(list_logs(&paths, "ok-instance", false).is_ok());
    }
}
