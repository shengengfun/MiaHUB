//! HID transport for AULA keyboards.

use std::ffi::CString;
use std::time::Instant;

use hidapi::{HidApi, HidDevice};

use crate::protocol;

/// A discovered device, shown in the UI device list.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    /// "wired" | "dongle" | "unsupported"
    pub connection: String,
    /// whether we can actually talk to it
    pub supported: bool,
    pub serial: String,
    /// opaque path used to open the device
    pub path: String,
    /// path of the boot-keyboard input collection, if the device exposes one
    pub input_path: String,
}

/// Result of a polling-rate measurement.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PollingStats {
    pub reports: u32,
    pub seconds: f32,
    /// reports per second seen on the input pipe
    pub hz: f32,
    /// median gap between two consecutive reports, in milliseconds
    pub median_ms: f32,
    pub min_ms: f32,
    pub max_ms: f32,
}

fn pretty_name(vid: u16, pid: u16) -> (String, String, bool) {
    match (vid, pid) {
        (protocol::VENDOR_ID, p) if protocol::F3009_WIRED_PIDS.contains(&p) => {
            ("AULA F3009".into(), "wired".into(), true)
        }
        (protocol::VENDOR_ID, p) if p == protocol::F3009_DONGLE_PID => {
            ("AULA F3009".into(), "dongle".into(), false)
        }
        (protocol::VENDOR_ID, p) => (format!("AULA device {p:04X}"), "unknown".into(), false),
        (v, p) if (v, p) == protocol::F2087PRO_USB => {
            ("AULA F2087Pro".into(), "wired".into(), false)
        }
        (v, p) if (v, p) == protocol::F2087PRO_DONGLE => {
            ("AULA F2087Pro".into(), "dongle".into(), false)
        }
        (v, p) => (format!("Unknown device {v:04X}:{p:04X}"), "unknown".into(), false),
    }
}

/// Enumerate all AULA HID collections and pick the ones worth showing.
pub fn list_devices(api: &HidApi) -> Vec<DeviceInfo> {
    // first pass: remember the boot-keyboard input collection of every vid/pid
    let inputs: Vec<(u16, u16, String)> = api
        .device_list()
        .filter(|i| {
            i.usage_page() == protocol::INPUT_USAGE_PAGE && i.usage() == protocol::INPUT_USAGE
        })
        .map(|i| {
            (
                i.vendor_id(),
                i.product_id(),
                i.path().to_string_lossy().to_string(),
            )
        })
        .collect();

    let mut out: Vec<DeviceInfo> = Vec::new();
    for info in api.device_list() {
        let (name, connection, mut supported) = pretty_name(info.vendor_id(), info.product_id());
        // only the vendor control collection can be used for configuration
        let is_ctrl = info.usage_page() == protocol::CTRL_USAGE_PAGE
            && info.usage() == protocol::CTRL_USAGE;
        if !is_ctrl {
            supported = false;
        }

        // skip duplicate entries for the same product/mode, keeping the control
        // collection when we have it
        if let Some(existing) = out
            .iter_mut()
            .find(|d| d.vid == info.vendor_id() && d.pid == info.product_id() && d.connection == connection)
        {
            if supported && !existing.supported {
                *existing = build_info(&name, info, connection, supported, &inputs);
            }
            continue;
        }
        out.push(build_info(&name, info, connection, supported, &inputs));
    }
    out
}

fn build_info(
    name: &str,
    info: &hidapi::DeviceInfo,
    connection: String,
    supported: bool,
    inputs: &[(u16, u16, String)],
) -> DeviceInfo {
    let input_path = inputs
        .iter()
        .find(|(v, p, _)| *v == info.vendor_id() && *p == info.product_id())
        .map(|(_, _, path)| path.clone())
        .unwrap_or_default();
    DeviceInfo {
        name: name.to_string(),
        vid: info.vendor_id(),
        pid: info.product_id(),
        connection,
        supported,
        serial: info.serial_number().unwrap_or("").to_string(),
        path: info.path().to_string_lossy().to_string(),
        input_path,
    }
}

/// Measure the real polling rate of the keyboard's input pipe.
///
/// Run it while holding a key down: an idle keyboard sends nothing, so the
/// figure is only meaningful under load.
pub fn measure_polling_rate(
    api: &HidApi,
    path: &str,
    seconds: f32,
) -> Result<PollingStats, String> {
    let c_path = CString::new(path).map_err(|e| e.to_string())?;
    let dev = api.open_path(&c_path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64];
    let start = Instant::now();
    let mut stamps: Vec<Instant> = Vec::new();

    while start.elapsed().as_secs_f32() < seconds {
        match dev.read_timeout(&mut buf, 20) {
            Ok(n) if n > 0 => stamps.push(Instant::now()),
            Ok(_) => {}
            Err(e) => return Err(e.to_string()),
        }
    }

    let elapsed = start.elapsed().as_secs_f32().max(0.001);
    // only gaps that look like real poll intervals count towards the median
    let mut gaps: Vec<f32> = stamps
        .windows(2)
        .map(|w| (w[1] - w[0]).as_secs_f32() * 1000.0)
        .filter(|g| (0.2..=50.0).contains(g))
        .collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let median_ms = gaps.get(gaps.len() / 2).copied().unwrap_or(0.0);
    Ok(PollingStats {
        reports: stamps.len() as u32,
        seconds: elapsed,
        hz: stamps.len() as f32 / elapsed,
        median_ms,
        min_ms: gaps.first().copied().unwrap_or(0.0),
        max_ms: gaps.last().copied().unwrap_or(0.0),
    })
}

/// Does a report arrive within `wait_ms`? Used for the idle/auto-dim timer.
pub fn probe_activity(api: &HidApi, path: &str, wait_ms: i32) -> bool {
    let Ok(c_path) = CString::new(path) else {
        return false;
    };
    let Ok(dev) = api.open_path(&c_path) else {
        return false;
    };
    let mut buf = [0u8; 64];
    matches!(dev.read_timeout(&mut buf, wait_ms), Ok(n) if n > 0)
}

/// An open control channel to a keyboard.
pub struct AulaDevice {
    dev: HidDevice,
    last_write: Instant,
}

impl AulaDevice {
    /// Open the vendor control collection of the given device path.
    pub fn open(api: &HidApi, path: &str) -> Result<Self, String> {
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        let dev = api.open_path(&c_path).map_err(|e| e.to_string())?;
        Ok(Self {
            dev,
            // start in the past so the first write is not delayed
            last_write: Instant::now() - protocol::MIN_SEND_INTERVAL,
        })
    }

    /// Send the 4 initialisation output reports used by the official tool.
    pub fn init(&self) -> Result<(), String> {
        for frame in protocol::init_output_reports() {
            // failures here are not fatal: not every firmware revision accepts them
            let _ = self.dev.write(&frame);
        }
        Ok(())
    }

    /// Apply a main-light effect / brightness / speed.
    pub fn set_lighting(
        &mut self,
        effect: u8,
        brightness: u8,
        speed: u8,
    ) -> Result<(), String> {
        let frame = protocol::lighting_frame(effect, brightness, speed);
        self.send_frame(&frame)
    }

    /// Turn the backlight off without changing the selected effect.
    pub fn set_off(&mut self, effect: u8) -> Result<(), String> {
        let frame = protocol::off_frame(effect);
        self.send_frame(&frame)
    }

    /// Read back the device info frame (the firmware reply is exposed by the
    /// driver; may be constant on some revisions).
    pub fn read_info(&mut self) -> Result<Vec<u8>, String> {
        let frame = protocol::read_info_frame();
        self.dev
            .send_feature_report(&frame)
            .map_err(|e| e.to_string())?;
        let mut buf = [0u8; 8];
        let n = self
            .dev
            .get_feature_report(&mut buf)
            .map_err(|e| e.to_string())?;
        Ok(buf[..n].to_vec())
    }

    /// Throttled feature-report write (>= 100 ms, as the firmware requires).
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        let elapsed = self.last_write.elapsed();
        if elapsed < protocol::MIN_SEND_INTERVAL {
            std::thread::sleep(protocol::MIN_SEND_INTERVAL - elapsed);
        }
        self.dev
            .send_feature_report(frame)
            .map_err(|e| e.to_string())?;
        self.last_write = Instant::now();
        Ok(())
    }
}

/// Short-lived helper: run `f` against the first supported device.
#[allow(dead_code)]
pub fn with_default_device<T>(
    f: impl FnOnce(&mut AulaDevice) -> Result<T, String>,
) -> Result<T, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let target = list_devices(&api)
        .into_iter()
        .find(|d| d.supported)
        .ok_or_else(|| "no supported AULA keyboard found (connect it with the USB cable)".to_string())?;
    let mut dev = AulaDevice::open(&api, &target.path)?;
    f(&mut dev)
}
