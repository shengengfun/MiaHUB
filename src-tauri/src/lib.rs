//! MiaKeyDrv — open-source manager for AULA keyboards.

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
    state: State<'_, AppState>,
    effect: u8,
    brightness: u8,
    speed: u8,
) -> Result<(), String> {
    let mut guard = state.device.lock().unwrap();
    let dev = guard.as_mut().ok_or("no keyboard connected")?;
    dev.set_lighting(effect, brightness, speed)?;
    *state.current_effect.lock().unwrap() = effect;
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

#[tauri::command]
fn set_close_to_tray(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<UiSettings, String> {
    let next = {
        let mut s = state.settings.lock().unwrap();
        s.close_to_tray = enabled;
        s.clone()
    };
    settings::save(&app, &next)?;
    Ok(next)
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
            set_close_to_tray
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
                .tooltip("MiaKeyDrv — AULA 键盘管理器")
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
        .expect("error while running MiaKeyDrv");
}
