//! MiaKeyDrv — open-source manager for AULA keyboards.

mod bt;
mod hid;
mod protocol;

use std::sync::Mutex;

use hidapi::HidApi;
use tauri::State;

use hid::{AulaDevice, DeviceInfo};

/// Shared application state: an optional open control channel.
#[derive(Default)]
pub struct AppState {
    device: Mutex<Option<AulaDevice>>,
    /// currently selected effect, so "light off" can restore it
    current_effect: Mutex<u8>,
    /// input collection of the connected keyboard (polling rate / activity)
    input_path: Mutex<String>,
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
            bt::bt_battery
        ])
        .run(tauri::generate_context!())
        .expect("error while running MiaKeyDrv");
}
