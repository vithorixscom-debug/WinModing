use crate::error::{io_at, LauncherError};

#[cfg(windows)]
mod win32 {
    //! Tiny FFI surface for `WM_NCLBUTTONDOWN` window dragging.
    //!
    //! This is safe because `hwnd` is a live Win32 handle from WebView2 on the UI
    //! thread (Tauri sync command). `ReleaseCapture` / `SendMessageW` are the
    //! documented way to start a caption drag; we skip the call when hwnd is null.
    #[link(name = "user32")]
    extern "system" {
        pub fn ReleaseCapture() -> i32;
        pub fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
}

pub fn apply_native_material(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        if window_vibrancy::apply_mica(window, Some(true)).is_err() {
            let _ = window_vibrancy::apply_acrylic(window, Some((18, 20, 24, 180)));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// Must run on the UI thread (sync Tauri command). CSS `-webkit-app-region` is not used:
/// on WebView2 it swallows mouse events without actually moving the window.
pub fn start_os_drag(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            let raw = hwnd.0 as isize;
            if raw != 0 {
                // SAFETY: hwnd came from this window on the UI thread and is non-null.
                unsafe {
                    win32::ReleaseCapture();
                    win32::SendMessageW(raw, 0x00A1, 2, 0);
                }
                return;
            }
        }
        let _ = window.start_dragging();
    }
    #[cfg(not(windows))]
    {
        let _ = window.start_dragging();
    }
}

pub fn open_with_os(target: &str) -> Result<(), LauncherError> {
    let target = target.trim();
    if target.is_empty() {
        return Err(LauncherError::Message("empty path".into()));
    }
    #[cfg(windows)]
    {
        let is_url = target.starts_with("http://")
            || target.starts_with("https://")
            || target.starts_with("mailto:");
        let mut command = if is_url {
            let mut cmd = std::process::Command::new("rundll32");
            cmd.arg("url.dll,FileProtocolHandler").arg(target);
            cmd
        } else {
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(target);
            cmd
        };
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|err| io_at(target, err))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|err| io_at(target, err))?;
        Ok(())
    }
}
