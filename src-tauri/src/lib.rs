//! MiaHUB — open-source manager for AULA keyboards.

mod bg;
mod bt;
mod hid;
mod protocol;
mod settings;

use std::sync::Mutex;

use hidapi::HidApi;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};

use hid::{AulaDevice, DeviceInfo};
use settings::UiSettings;

/// Shared application state: an optional open control channel.
#[derive(Default)]
pub struct AppState {
    device: Mutex<Option<AulaDevice>>,
    /// currently selected effect, so "light off" can restore it
    current_effect: Mutex<u8>,
    /// input collection of the connected keyboard (polling rate / activity)
    input_path: Mutex<String>,
    /// UI preferences, mirrored from `settings.json`
    settings: Mutex<UiSettings>,
}

/// Bring the main window back from the tray.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    Ok(hid::list_devices(&api))
}

#[tauri::command]
fn list_effects() -> Vec<protocol::Effect> {
    protocol::EFFECTS.to_vec()
}

#[tauri::command]
fn connect(state: State<'_, AppState>, path: String) -> Result<DeviceInfo, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let info = hid::list_devices(&api)
        .into_iter()
        .find(|d| d.path == path)
        .ok_or_else(|| "device not found (re-plug the USB cable?)".to_string())?;
    if !info.supported {
        return Err(format!(
            "{}（{}）没有暴露厂商控制集合，当前连接模式下无法控制。\n若是 2.4G / 蓝牙模式，请切回 USB 有线后再试。",
            info.name, info.connection
        ));
    }
    let dev = AulaDevice::open(&api, &path, info.pid)?;
    dev.init()?;
    *state.device.lock().unwrap() = Some(dev);
    *state.current_effect.lock().unwrap() = protocol::DEFAULT_EFFECT;
    *state.input_path.lock().unwrap() = info.input_path.clone();
    Ok(info)
}

#[tauri::command]
fn set_lighting(
    app: AppHandle,
    state: State<'_, AppState>,
    effect: u8,
    brightness: u8,
    speed: u8,
) -> Result<(), String> {
    let mut guard = state.device.lock().unwrap();
    let dev = guard.as_mut().ok_or("no keyboard connected")?;
    dev.set_lighting(effect, brightness, speed)?;
    drop(guard);
    *state.current_effect.lock().unwrap() = effect;
    // Remember it: neither the keyboard nor the HID stack can read the current
    // lighting back (PROTOCOL.md §4), so this app has to be the source of truth.
    persist(&app, &state, |s| {
        s.lighting = settings::Lighting {
            effect,
            brightness,
            speed,
        }
    })?;
    Ok(())
}

#[tauri::command]
fn set_light_off(state: State<'_, AppState>) -> Result<(), String> {
    let effect = *state.current_effect.lock().unwrap();
    let mut guard = state.device.lock().unwrap();
    let dev = guard.as_mut().ok_or("no keyboard connected")?;
    dev.set_off(effect)
}

#[tauri::command]
fn read_info(state: State<'_, AppState>) -> Result<String, String> {
    let mut guard = state.device.lock().unwrap();
    let dev = guard.as_mut().ok_or("no keyboard connected")?;
    let data = dev.read_info()?;
    Ok(data.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))
}

/// Measure the real polling rate of the keyboard's input pipe.
#[tauri::command]
fn measure_polling(state: State<'_, AppState>, seconds: f32) -> Result<hid::PollingStats, String> {
    let path = state.input_path.lock().unwrap().clone();
    if path.is_empty() {
        return Err("这台设备没有暴露出键盘输入集合".into());
    }
    let api = HidApi::new().map_err(|e| e.to_string())?;
    hid::measure_polling_rate(&api, &path, seconds)
}

/// Did the keyboard send anything within `wait_ms`? Drives the idle timer.
#[tauri::command]
fn probe_activity(state: State<'_, AppState>, wait_ms: i32) -> bool {
    let path = state.input_path.lock().unwrap().clone();
    if path.is_empty() {
        return false;
    }
    let Ok(api) = HidApi::new() else { return false };
    hid::probe_activity(&api, &path, wait_ms)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> UiSettings {
    state.settings.lock().unwrap().clone()
}

/// Mutate the settings, then write them straight back to disk.
fn persist(
    app: &AppHandle,
    state: &State<'_, AppState>,
    mutate: impl FnOnce(&mut UiSettings),
) -> Result<UiSettings, String> {
    let snapshot = {
        let mut s = state.settings.lock().unwrap();
        mutate(&mut s);
        s.clone()
    };
    settings::save(app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
fn set_close_to_tray(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<UiSettings, String> {
    persist(&app, &state, |s| s.close_to_tray = enabled)
}

#[tauri::command]
fn set_apply_on_connect(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<UiSettings, String> {
    persist(&app, &state, |s| s.apply_on_connect = enabled)
}

#[tauri::command]
fn set_keep_awake(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<UiSettings, String> {
    persist(&app, &state, |s| s.keep_awake = enabled)
}

#[tauri::command]
fn set_idle_step(
    app: AppHandle,
    state: State<'_, AppState>,
    step: usize,
) -> Result<UiSettings, String> {
    persist(&app, &state, |s| s.idle_step = step)
}

/// 外观：浅色/深色、主题色、背景皮肤。三项一起写，前端只改 CSS 变量。
#[tauri::command]
fn set_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
    accent: String,
    background: String,
) -> Result<UiSettings, String> {
    persist(&app, &state, |s| {
        s.theme_mode = mode;
        s.accent = accent;
        s.background = background;
    })
}

/// 存下用户上传的背景图（base64，不带 data: 前缀）。
#[tauri::command]
fn save_background(
    app: AppHandle,
    state: State<'_, AppState>,
    data: String,
    mime: String,
) -> Result<UiSettings, String> {
    let bytes = bg::decode(&data)?;
    if bytes.is_empty() {
        return Err("图片是空的".into());
    }
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("图片太大了（上限 8MB）".into());
    }
    bg::save(&app, &bytes)?;
    persist(&app, &state, |s| {
        s.background = "custom".into();
        s.bg_mime = mime;
    })
}

/// 返回已保存背景图的 data URL，没存过就返回 None。
#[tauri::command]
fn background_image(app: AppHandle, state: State<'_, AppState>) -> Option<String> {
    let bytes = bg::load(&app)?;
    let mime = {
        let s = state.settings.lock().ok()?;
        if s.background != "custom" {
            return None;
        }
        if s.bg_mime.is_empty() {
            "image/png".to_string()
        } else {
            s.bg_mime.clone()
        }
    };
    Some(format!("data:{mime};base64,{}", bg::encode(&bytes)))
}

#[tauri::command]
fn clear_background(app: AppHandle, state: State<'_, AppState>) -> Result<UiSettings, String> {
    bg::clear(&app);
    persist(&app, &state, |s| {
        s.background = "default".into();
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            list_effects,
            connect,
            set_lighting,
            set_light_off,
            read_info,
            measure_polling,
            probe_activity,
            bt::bt_battery,
            get_settings,
            set_close_to_tray,
            set_apply_on_connect,
            set_keep_awake,
            set_idle_step,
            set_theme,
            save_background,
            background_image,
            clear_background
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // pull the persisted prefs in before anything can consult them
            *app.state::<AppState>().settings.lock().unwrap() =
                settings::load(&handle);

            let menu = Menu::with_items(
                &handle,
                &[
                    &MenuItem::with_id(&handle, "show", "显示主界面", true, None::<&str>)?,
                    &PredefinedMenuItem::separator(&handle)?,
                    &MenuItem::with_id(&handle, "quit", "退出", true, None::<&str>)?,
                ],
            )?;

            let mut builder = TrayIconBuilder::with_id("main")
                .tooltip("MiaHUB — AULA 键盘管理器")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // left click restores, right click opens the menu
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            // image-png backs this; without the feature it is None
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .state::<AppState>()
                    .settings
                    .lock()
                    .map(|s| s.close_to_tray)
                    .unwrap_or(false);
                if close_to_tray {
                    // keep the process (and the keyboard session) alive in the tray
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MiaHUB");
}
