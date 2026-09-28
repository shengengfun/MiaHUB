//! Persisted application preferences.
//!
//! Deliberately tiny: one JSON object under the app config dir
//! (`%APPDATA%\com.shengengfun.miahub\settings.json`).
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
    /// Re-send the remembered lighting as soon as a keyboard connects.
    ///
    /// **Off by default on purpose.** The keyboard does not report its current
    /// lighting back (see PROTOCOL.md §4), so the app cannot know what the user
    /// has it set to. Pushing blindly means "open the app → your effect is
    /// replaced by whatever we happen to remember". The official tool has
    /// exactly this wart (`07 FF FF 00 05 00 00 00` on open). Turning this on
    /// restores the official behaviour for people who want it.
    pub apply_on_connect: bool,
    /// The lighting the user last picked. Persisted because neither the
    /// keyboard nor the Windows HID stack can tell us what is actually set.
    pub lighting: Lighting,
    /// Keep-awake ping enabled (see the frontend's keep-awake loop).
    pub keep_awake: bool,
    /// Index into the frontend's IDLE_STEPS table.
    pub idle_step: usize,
    /// "dark" | "light"
    pub theme_mode: String,
    /// id of the accent preset (see the frontend's ACCENTS table)
    pub accent: String,
    /// "default" | a built-in preset id | "custom"
    pub background: String,
    /// MIME of the custom background, so we can rebuild its data URL
    pub bg_mime: String,
    /// Mirror of the `HKCU\...\Run` entry (see `autostart.rs`). The registry
    /// wins: the user can delete the entry from Task Manager behind our back.
    pub autostart: bool,
}

/// Last-known lighting selection, mirroring the wire frame's three fields.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    pub effect: u8,
    pub brightness: u8,
    pub speed: u8,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            effect: crate::protocol::DEFAULT_EFFECT,
            brightness: crate::protocol::MAX_BRIGHTNESS,
            speed: crate::protocol::MAX_SPEED,
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            // matches the behaviour of every other peripheral utility:
            // ✕ keeps the app alive so lighting/keep-awake keep working
            close_to_tray: true,
            apply_on_connect: false,
            lighting: Lighting::default(),
            keep_awake: false,
            idle_step: 0,
            theme_mode: "dark".into(),
            accent: "classic".into(),
            background: "default".into(),
            bg_mime: "image/png".into(),
            autostart: false,
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
