use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = command;
}

fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    hide_window(&mut command);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::null());
    command
}

use serde::Deserialize;

use crate::domain::{ContentHash, JavaRuntime};
use crate::error::LauncherError;
use crate::net::HttpPool;
use crate::paths::AppPaths;
use crate::protocol::adoptium_jre_url;

pub async fn detect_java_runtimes() -> Result<Vec<JavaRuntime>, LauncherError> {
    tokio::task::spawn_blocking(scan_java_candidates)
        .await
        .map_err(|err| LauncherError::Message(err.to_string()))?
}

fn scan_java_candidates() -> Result<Vec<JavaRuntime>, LauncherError> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        push_candidate(
            &mut found,
            &mut seen,
            PathBuf::from(home).join("bin").join(java_binary()),
            "JAVA_HOME",
        );
    }
    for root in well_known_roots() {
        if !root.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            push_candidate(
                &mut found,
                &mut seen,
                path.join("bin").join(java_binary()),
                "install-dir",
            );
        }
        push_candidate(
            &mut found,
            &mut seen,
            root.join("bin").join(java_binary()),
            "install-dir",
        );
    }
    if let Some(path_hit) = command_output("where", &["javaw"]) {
        for line in path_hit.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                push_candidate(&mut found, &mut seen, PathBuf::from(trimmed), "PATH");
            }
        }
    }
    Ok(found)
}

pub fn select_runtime(runtimes: &[JavaRuntime], major: u32) -> Result<JavaRuntime, LauncherError> {
    runtimes
        .iter()
        .find(|runtime| runtime.major == major)
        .cloned()
        .ok_or(LauncherError::JavaMissing { major })
}

pub async fn install_adoptium(
    http: &HttpPool,
    paths: &AppPaths,
    major: u32,
) -> Result<JavaRuntime, LauncherError> {
    let url = adoptium_jre_url(major);
    let text = http.get_text(&url, &[]).await?;
    let assets: Vec<AdoptiumAsset> =
        serde_json::from_str(&text).map_err(|err| LauncherError::CorruptVersion {
            reason: format!("Adoptium response: {err}"),
        })?;
    let asset = assets
        .into_iter()
        .next()
        .ok_or_else(|| LauncherError::JavaMissing { major })?;
    let link = asset.binary.package.link;
    let checksum = asset.binary.package.checksum;
    if checksum.trim().is_empty() {
        return Err(LauncherError::MissingHash { url: link });
    }
    let archive = paths.runtime.join(format!("java-{major}.zip"));
    http.download_verified(&link, &archive, &ContentHash::sha256(checksum))
        .await?;
    let home = paths.java_home(major);
    let archive_for_extract = archive.clone();
    let home_for_extract = home.clone();
    tokio::task::spawn_blocking(move || {
        extract_zip_blocking(&archive_for_extract, &home_for_extract)
    })
    .await
    .map_err(|err| LauncherError::Message(err.to_string()))??;
    let binary = find_java(&home).ok_or(LauncherError::JavaMissing { major })?;
    probe_java(&binary, "adoptium").ok_or(LauncherError::JavaMissing { major })
}

fn extract_zip_blocking(archive: &Path, dest: &Path) -> Result<(), LauncherError> {
    let archive = archive.to_path_buf();
    let dest = dest.to_path_buf();
    std::fs::create_dir_all(&dest).map_err(|err| crate::error::io_at(&dest, err))?;
    let file = std::fs::File::open(&archive).map_err(|err| crate::error::io_at(&archive, err))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|err| LauncherError::CorruptVersion {
        reason: format!("jre archive: {err}"),
    })?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|err| LauncherError::CorruptVersion {
                reason: format!("jre entry: {err}"),
            })?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let target = dest.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|err| crate::error::io_at(&target, err))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| crate::error::io_at(parent, err))?;
        }
        let mut output =
            std::fs::File::create(&target).map_err(|err| crate::error::io_at(&target, err))?;
        std::io::copy(&mut entry, &mut output).map_err(|err| crate::error::io_at(&target, err))?;
    }
    Ok(())
}

fn find_java(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 6 {
            continue;
        }
        let candidate = dir.join("bin").join(java_binary());
        if candidate.exists() {
            return Some(candidate);
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                stack.push((entry.path(), depth + 1));
            }
        }
    }
    None
}

fn java_binary() -> &'static str {
    if cfg!(windows) {
        "javaw.exe"
    } else {
        "java"
    }
}

fn well_known_roots() -> Vec<PathBuf> {
    let program_files =
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    ["Eclipse Adoptium", "Java", "Microsoft", "Zulu"]
        .into_iter()
        .map(|name| PathBuf::from(&program_files).join(name))
        .collect()
}

fn push_candidate(
    found: &mut Vec<JavaRuntime>,
    seen: &mut std::collections::HashSet<String>,
    path: PathBuf,
    source: &str,
) {
    if !path.exists() {
        return;
    }
    let key = path.display().to_string().to_ascii_lowercase();
    if !seen.insert(key) {
        return;
    }
    if let Some(runtime) = probe_java(&path, source) {
        found.push(runtime);
    }
}

fn probe_java(path: &Path, source: &str) -> Option<JavaRuntime> {
    let mut command = Command::new(path);
    hide_window(&mut command);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let output = command.arg("-version").output().ok()?;
    let text = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    let line = text.lines().next()?.trim().to_string();
    let major = parse_major(&line)?;
    Some(JavaRuntime {
        path: path.display().to_string(),
        major,
        version_line: line,
        source: source.to_string(),
    })
}

pub fn parse_major(version_line: &str) -> Option<u32> {
    let start = version_line.find('"')?;
    let rest = &version_line[start + 1..];
    let end = rest.find('"')?;
    let version = &rest[..end];
    let mut parts = version.split('.');
    let first = parts.next()?;
    if first == "1" {
        parts.next()?.parse().ok()
    } else {
        first.parse().ok()
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = hidden_command(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn available_memory_mb() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.available_memory() / (1024 * 1024)
}

pub fn total_memory_mb() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory() / (1024 * 1024)
}

pub fn total_memory_kib() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory() / 1024
}

pub fn pid_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.process(sysinfo::Pid::from_u32(pid)).is_some()
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    #[serde(default)]
    checksum: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_and_legacy_java_versions() {
        assert_eq!(
            parse_major("openjdk version \"21.0.2\" 2024-01-16"),
            Some(21)
        );
        assert_eq!(parse_major("java version \"1.8.0_392\""), Some(8));
    }
}
