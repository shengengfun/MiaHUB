//! "Start with Windows" through the `HKCU\...\Run` key.
//!
//! Shells out to `reg.exe` instead of pulling in a registry crate: it is one
//! value under one key, and this way there is no dependency and no unsafe.
//!
//! The stored command is `"<exe>" --silent`. That flag is what makes the app
//! come up **hidden in the tray** — otherwise logging in would pop a window.

use std::process::Command;

use tauri::AppHandle;

/// Command-line flag that means "start hidden in the tray".
pub const SILENT_FLAG: &str = "--silent";

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Value name under the Run key. Keep this stable — it is what uninstall
/// instructions and the "did it stick?" check look for.
#[cfg(windows)]
const VALUE: &str = "MiaHUB";

/// Is the Run entry present right now?
///
/// Read from the registry rather than from our own settings file: the user can
/// remove the entry from Task Manager at any time, and the UI should show the
/// truth.
pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        Command::new("reg")
            .args(["query", RUN_KEY, "/v", VALUE])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Register (or remove) the Run entry. `--silent` is appended when enabling.
pub fn set_enabled(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let _ = app;
    #[cfg(windows)]
    {
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = format!("\"{}\" {}", exe.display(), SILENT_FLAG);
            let out = Command::new("reg")
                .args([
                    "add", RUN_KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &command, "/f",
                ])
                .output()
                .map_err(|e| format!("调用 reg.exe 失败：{e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "写注册表失败：{}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
        } else {
            // deleting a value that is not there is not an error we care about
            let _ = Command::new("reg")
                .args(["delete", RUN_KEY, "/v", VALUE, "/f"])
                .output();
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if enabled {
            Err("开机自启目前只在 Windows 上实现".into())
        } else {
            Ok(())
        }
    }
}
