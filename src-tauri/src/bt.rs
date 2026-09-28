//! Read the battery level Windows already caches for a paired BLE keyboard.
//!
//! A BLE HID keyboard exposes the standard GATT Battery Service (0x180F), and
//! Windows keeps the last value in the device node's property store under
//! `DEVPKEY_Bluetooth_Battery` (`{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2`) —
//! the very same value the Bluetooth settings page shows.
//!
//! The 2.4 GHz dongle has no such thing: its vendor channel is write-only
//! (see OpenALUA's docs/PROTOCOL.md §10), so this is the only way to get a
//! battery reading out of this keyboard.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BtBattery {
    /// `None` when Windows has no cached value
    pub percent: Option<u8>,
    /// `DEVPKEY_Bluetooth_Battery, 3` — believed to mean "charging"
    pub charging: Option<bool>,
    /// Bluetooth address, e.g. `000029f00000`
    pub address: String,
}

#[cfg(windows)]
mod win {
    use super::BtBattery;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    pub const CR_SUCCESS: u32 = 0;
    pub const CM_GETIDLIST_FILTER_ENUMERATOR: u32 = 0x0000_0001;
    pub const CM_LOCATE_DEVNODE_NORMAL: u32 = 0;
    pub const DEVPROP_TYPE_BYTE: u32 = 0x0000_0003;
    pub const DEVPROP_TYPE_UINT32: u32 = 0x0000_0007;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Guid {
        pub d1: u32,
        pub d2: u16,
        pub d3: u16,
        pub d4: [u8; 8],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct DevPropKey {
        pub fmtid: Guid,
        pub pid: u32,
    }

    /// DEVPKEY_Bluetooth_Battery — battery percentage.
    pub const KEY_BATTERY: DevPropKey = DevPropKey {
        fmtid: Guid {
            d1: 0x104E_A319,
            d2: 0x6EE2,
            d3: 0x4701,
            d4: [0xBD, 0x47, 0x8D, 0xDB, 0xF4, 0x25, 0xBB, 0xE5],
        },
        pid: 2,
    };

    /// DEVPKEY_Bluetooth_Battery (second member) — observed as a bool.
    pub const KEY_CHARGING: DevPropKey = DevPropKey {
        fmtid: Guid {
            d1: 0x104E_A319,
            d2: 0x6EE2,
            d3: 0x4701,
            d4: [0xBD, 0x47, 0x8D, 0xDB, 0xF4, 0x25, 0xBB, 0xE5],
        },
        pid: 3,
    };

    #[link(name = "cfgmgr32")]
    extern "system" {
        fn CM_Get_Device_ID_List_SizeW(len: *mut u32, filter: *const u16, flags: u32) -> u32;
        fn CM_Get_Device_ID_ListW(
            filter: *const u16,
            buffer: *mut u16,
            buffer_len: u32,
            flags: u32,
        ) -> u32;
        fn CM_Locate_DevNodeW(node: *mut u32, device_id: *mut u16, flags: u32) -> u32;
        fn CM_Get_DevNode_PropertyW(
            node: u32,
            key: *const DevPropKey,
            prop_type: *mut u32,
            buffer: *mut u8,
            buffer_size: *mut u32,
            flags: u32,
        ) -> u32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Every device instance id under the `BTHLE` enumerator.
    fn ble_device_ids() -> Vec<String> {
        let filter = wide("BTHLE");
        let mut len: u32 = 0;
        unsafe {
            if CM_Get_Device_ID_List_SizeW(&mut len, filter.as_ptr(), CM_GETIDLIST_FILTER_ENUMERATOR)
                != CR_SUCCESS
            {
                return Vec::new();
            }
        }
        if len == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        unsafe {
            if CM_Get_Device_ID_ListW(
                filter.as_ptr(),
                buf.as_mut_ptr(),
                len,
                CM_GETIDLIST_FILTER_ENUMERATOR,
            ) != CR_SUCCESS
            {
                return Vec::new();
            }
        }
        // MULTI_SZ: split on NUL, stop at the empty entry
        let mut out = Vec::new();
        let mut start = 0usize;
        for i in 0..buf.len() {
            if buf[i] == 0 {
                if i == start {
                    break;
                }
                let s = OsString::from_wide(&buf[start..i]);
                out.push(s.to_string_lossy().to_string());
                start = i + 1;
            }
        }
        out
    }

    fn read_prop(node: u32, key: &DevPropKey) -> Option<(u32, Vec<u8>)> {
        let mut ty: u32 = 0;
        let mut size: u32 = 0;
        unsafe {
            let rc = CM_Get_DevNode_PropertyW(
                node,
                key,
                &mut ty,
                std::ptr::null_mut(),
                &mut size,
                0,
            );
            // CR_BUFFER_SMALL is expected on the probe call
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            let rc2 =
                CM_Get_DevNode_PropertyW(node, key, &mut ty, buf.as_mut_ptr(), &mut size, 0);
            let _ = rc;
            if rc2 != CR_SUCCESS {
                return None;
            }
            buf.truncate(size as usize);
            Some((ty, buf))
        }
    }

    fn as_u8(ty: u32, buf: &[u8]) -> Option<u8> {
        match ty {
            DEVPROP_TYPE_BYTE => buf.first().copied(),
            DEVPROP_TYPE_UINT32 if buf.len() >= 4 => {
                Some(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as u8)
            }
            _ => buf.first().copied(),
        }
    }

    pub fn read() -> Option<BtBattery> {
        for id in ble_device_ids() {
            let idw = wide(&id);
            let mut node: u32 = 0;
            unsafe {
                if CM_Locate_DevNodeW(&mut node, idw.as_ptr() as *mut u16, CM_LOCATE_DEVNODE_NORMAL)
                    != CR_SUCCESS
                {
                    continue;
                }
            }
            let percent = read_prop(node, &KEY_BATTERY).and_then(|(t, b)| as_u8(t, &b));
            if percent.is_none() {
                continue;
            }
            let charging = read_prop(node, &KEY_CHARGING).and_then(|(_t, b)| b.first().copied())
                .map(|v| v != 0);
            // the address is the trailing part of BTHLE\Dev_<addr>\...
            let address = id
                .split('\\')
                .nth(1)
                .unwrap_or("")
                .trim_start_matches("Dev_")
                .to_string();
            return Some(BtBattery {
                percent,
                charging,
                address,
            });
        }
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    /// Smoke test for the CfgMgr32 plumbing — needs a paired BLE keyboard,
    /// so it only reports, it never asserts.
    #[test]
    fn reads_battery_from_ble_node() {
        println!("bt battery probe => {:?}", super::win::read());
    }
}

/// Battery of a paired BLE keyboard, if Windows has one cached.
///
/// Returns `None` when nothing BLE is paired — that is not an error, the
/// keyboard simply is not in Bluetooth mode.
#[tauri::command]
pub fn bt_battery() -> Option<BtBattery> {
    #[cfg(windows)]
    {
        win::read()
    }
    #[cfg(not(windows))]
    {
        None
    }
}
