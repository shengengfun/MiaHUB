//! Persisted application preferences.
//!
//! Deliberately tiny: one JSON object under the app config dir
//! (`%APPDATA%\com.shengengfun.miakeydrv\settings.json`).
//! Everything here is a *UI* preference — nothing that touches the keyboard.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Closing the main window hides it to the tray instead of quitting.
    /// Turn this off and the ✕ button really exits.
    pub close_to_tray: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            // matches the behaviour of every other peripheral utility:
            // ✕ keeps the app alive so lighting/keep-awake keep working
            close_to_tray: true,
        }
    }
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

/// Read the settings, falling back to defaults for a missing or corrupt file.
pub fn load(app: &AppHandle) -> UiSettings {
    file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, s: &UiSettings) -> Result<(), String> {
    let path = file(app).ok_or("拿不到应用配置目录")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
