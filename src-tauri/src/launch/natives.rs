use std::fs::{self, File};
use std::io::{self, copy};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zip::ZipArchive;

use crate::error::{io_at, LauncherError};

/// Fresh directory per launch so a DLL still mapped by a previous process
/// cannot block the next extraction.
pub fn fresh_natives_dir(instance_root: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    instance_root.join("natives").join(stamp.to_string())
}

pub fn extract_natives(jar: &Path, dest: &Path) -> Result<usize, LauncherError> {
    fs::create_dir_all(dest).map_err(|err| io_at(dest, err))?;
    let file = File::open(jar).map_err(|err| io_at(jar, err))?;
    let mut archive = ZipArchive::new(file).map_err(|err| LauncherError::CorruptVersion {
        reason: format!("natives jar {}: {err}", jar.display()),
    })?;
    let mut written = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| LauncherError::CorruptVersion {
                reason: format!("natives entry {index}: {err}"),
            })?;
        if entry.is_dir() {
            continue;
        }
        let Some(safe_name) = entry.enclosed_name() else {
            continue;
        };
        let Some(file_name) = safe_name.file_name().map(|name| name.to_os_string()) else {
            continue;
        };
        let name = file_name.to_string_lossy();
        if !is_native_library(&name) {
            continue;
        }
        let target = dest.join(file_name);
        let mut output = File::create(&target).map_err(|err| io_at(&target, err))?;
        copy(&mut entry, &mut output).map_err(|err| io_at(&target, err))?;
        written += 1;
    }
    Ok(written)
}

fn is_native_library(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib")
}

pub fn remove_dir_if_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn extracts_dll_and_skips_metadata() {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default();
            writer.start_file("glfw.dll", options).unwrap();
            writer.write_all(b"native-bytes").unwrap();
            writer.start_file("META-INF/MANIFEST.MF", options).unwrap();
            writer.write_all(b"Manifest-Version: 1.0").unwrap();
            writer.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!(
            "winmoding-natives-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let jar = dir.join("sample.jar");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&jar, cursor.get_ref()).unwrap();
        let dest = dir.join("out");
        let count = extract_natives(&jar, &dest).unwrap();
        assert_eq!(count, 1);
        assert_eq!(fs::read(dest.join("glfw.dll")).unwrap(), b"native-bytes");
        assert!(!dest.join("MANIFEST.MF").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
