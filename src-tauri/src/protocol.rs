//! AULA keyboard protocol (reverse-engineered, verified against the official
//! ShinetekTools.exe traffic captured with frida).
//!
//! Frame format (8-byte HID feature report, report id = 7):
//!
//! ```text
//!   07 FF FF <effect> <brightness> <speed> <d3> <d4>
//! ```
//!
//! * `effect`     – effect index, **0-based**, matching the value the official
//!   tool puts on the wire (0..=19). See [`EFFECTS`].
//! * `brightness` – 0..=5, where 0 turns the backlight **off**
//! * `speed`      – 0..=2
//! * `d3`, `d4`   – unused by the official UI (always 0 in captured traffic)
//!
//! Device handshake seen in the official tool:
//! ```text
//!   output report (64 B): 05 01 .. / 05 02 .. / 05 03 .. / 05 04 ..
//!   feature: 07 F1 00 00 00 00 00 00  + get_feature(7)   (read device info)
//!   feature: 07 FF FF 00 05 00 00 00                     (reg 0 = global)
//! ```
//!
//! The official driver throttles writes to >= 100 ms to avoid wedging the
//! firmware; the same delay is enforced here.

use std::time::Duration;

/// Vendor ids of AULA devices.
pub const VENDOR_ID: u16 = 0x1A2C;

/// Wired F3009 product ids (both observed in the wild).
pub const F3009_WIRED_PIDS: [u16; 2] = [0x7F05, 0x7F07];

/// 2.4 GHz dongle product id.
pub const F3009_DONGLE_PID: u16 = 0x7FFF;

/// F2087Pro (different MCU vendor, protocol not yet reverse-engineered).
pub const F2087PRO_USB: (u16, u16) = (0x0C45, 0x800A);
/// F2087Pro 2.4 GHz dongle.
pub const F2087PRO_DONGLE: (u16, u16) = (0x05AC, 0x024F);

/// Vendor HID collection used for control (interface 1, collection 7).
pub const CTRL_USAGE_PAGE: u16 = 0xFF01;
pub const CTRL_USAGE: u16 = 0x0001;

/// Boot-keyboard input collection — used to measure the real polling rate and
/// to detect user activity (the firmware exposes no register for either).
pub const INPUT_USAGE_PAGE: u16 = 0x0001;
pub const INPUT_USAGE: u16 = 0x0006;

/// Minimum interval between frames, as enforced by the official driver.
pub const MIN_SEND_INTERVAL: Duration = Duration::from_millis(110);

/// Minimum interval between 2.4 GHz output reports.
///
/// The official tool sleeps **1500 ms** (`push 0x5dc`) after sending a
/// lighting envelope. Sending faster wedges the dongle's radio link and the
/// keyboard starts dropping/sticking keys, so this is not just politeness.
pub const MIN_SEND_INTERVAL_24G: Duration = Duration::from_millis(1500);

/// One main-light effect (0-based index == the byte on the wire).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Effect {
    pub index: u8,
    pub zh: &'static str,
    pub en: &'static str,
    /// whether the effect honours the speed byte
    pub has_speed: bool,
    /// the official UI hides this row for the F3009, but the firmware
    /// accepts the value — exposed by MiaHUB as a bonus
    pub hidden: bool,
}

macro_rules! effects {
    ($(($idx:expr, $zh:literal, $en:literal, $spd:expr, $hid:expr)),* $(,)?) => {
        pub const EFFECTS: &[Effect] = &[
            $(Effect { index: $idx, zh: $zh, en: $en, has_speed: $spd, hidden: $hid },)*
        ];
    };
}

// The wire index is the `data` attribute of the official UI rows, which is
// *not* the row number: row 1 (常亮/Steady) carries data=0, the 
// commented-out row carries data=1, and every later row carries data == row
// number. `uires/Translator/lang_cn.xml` lists them in exactly this order.
effects! {
    (0,  "常亮",       "Steady",                    false, false),
    (1,  "指点江山",   "Gaming Special Key",        false, true),
    (2,  "呼吸",       "Breathing",                 true,  false),
    (3,  "随按随灭",   "Press and destroy",         true,  false),
    (4,  "随波逐流",   "Neon stream",               true,  false),
    (5,  "流光模式",   "Streamer",                  true,  false),
    (6,  "流光溢彩",   "Flowing light and color",   true,  false),
    (7,  "滴水涟漪",   "Dripping ripples",          true,  false),
    (8,  "点彩夺目",   "Brilliant point",           true,  false),
    (9,  "一触即发",   "Flash away",                true,  false),
    (10, "踏雪无痕",   "Shadow disappear",          true,  false),
    (11, "按键涟漪",   "Ripples shining",           true,  false),
    (12, "花开富贵",   "Rich and honored",          true,  false),
    (13, "跑马灯效",   "Marquee effect",            true,  false),
    (14, "旋转风暴",   "Rotating storm",            true,  false),
    (15, "蛇形跑马",   "Serpentine horse race",     true,  false),
    (16, "繁星点点",   "Stars twinkle",             true,  false),
    (17, "川流不息",   "Retro snake",               true,  false),
    (18, "斜拉变幻",   "Diagonal transformation",   true,  false),
    (19, "正弦光波",   "Sine wave",                 true,  false),
}

/// Effect the keyboard boots into / the official tool restores.
pub const DEFAULT_EFFECT: u8 = 0;

/// Magic header of every 2.4 GHz output report.
///
/// The dongle's vendor collection has **no feature reports at all**
/// (`FeatureReportByteLength == 0`), only 65-byte input/output reports. The
/// official tool wraps the same logical command into this envelope:
///
/// ```text
///   00 BB AA 99 88 AA <effect> <brightness> <speed> 00 00  ... zero padded
///   └┬┘ └───┬───┘ └┬┘ └────────────┬─────────────┘
/// reportid magic   cmd        same bytes as the wired frame[3..7]
/// ```
///
/// The magic is **4 bytes**; the `AA` that follows it at offset 5 *is* the
/// command discriminator, not a fifth magic byte. Writing it twice shifts the
/// whole payload one byte to the right and the firmware silently ignores the
/// frame — which is exactly the bug that made the keyboard look stuck on its
/// own default effect.
///
/// Verified against `capture_24g.log` (official ShinetekTools.exe, 2.4G mode):
///
/// ```text
///   00 bb aa 99 88 aa 0a 05 02 00 …   effect 10, brightness 5, speed 2
///   00 bb aa 99 88 aa 00 05 00 00 …   effect  0, brightness 5, speed 0
///   00 bb aa 99 88 aa 13 05 02 00 …   effect 19, brightness 5, speed 2
/// ```
pub const MAGIC_24G: [u8; 4] = [0xBB, 0xAA, 0x99, 0x88];

/// Command discriminator for "set lighting" inside a 2.4 GHz envelope.
///
/// Sits at offset 5, immediately after the 4-byte magic.
pub const CMD_24G_LIGHTING: u8 = 0xAA;

/// Length of a 2.4 GHz output report (report id + 64 bytes).
pub const REPORT_24G_LEN: usize = 65;

/// Build the 65-byte 2.4 GHz output report for the main lighting.
pub fn envelope_24g(effect: u8, brightness: u8, speed: u8) -> [u8; REPORT_24G_LEN] {
    let mut out = [0u8; REPORT_24G_LEN];
    out[0] = 0x00; // report id
    out[1..5].copy_from_slice(&MAGIC_24G); // 4-byte magic, [4] is 0x88
    out[5] = CMD_24G_LIGHTING;
    // bytes 6..=8 mirror the wired frame's [3..=5]
    out[6] = effect;
    out[7] = brightness.min(MAX_BRIGHTNESS);
    out[8] = speed.min(MAX_SPEED);
    out
}

pub const MAX_BRIGHTNESS: u8 = 5;
pub const MAX_SPEED: u8 = 2;

/// Build the 8-byte feature frame for the main lighting.
pub fn lighting_frame(effect: u8, brightness: u8, speed: u8) -> [u8; 8] {
    [
        0x07,
        0xFF,
        0xFF,
        effect,
        brightness.min(MAX_BRIGHTNESS),
        speed.min(MAX_SPEED),
        0x00,
        0x00,
    ]
}

/// Build the "read device info" frame.
pub fn read_info_frame() -> [u8; 8] {
    [0x07, 0xF1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
}

/// The 4 initialisation output reports sent by the official tool on open.
pub fn init_output_reports() -> [[u8; 64]; 4] {
    let mut out = [[0u8; 64]; 4];
    for (i, frame) in out.iter_mut().enumerate() {
        frame[0] = 0x05;
        frame[1] = (i + 1) as u8;
    }
    out
}

/// Turn the backlight off (brightness 0 keeps the current effect selection).
#[allow(dead_code)]
pub fn off_frame(effect: u8) -> [u8; 8] {
    lighting_frame(effect, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breathing_frame_matches_official_traffic() {
        // captured from ShinetekTools.exe when selecting effect 2 at full
        // brightness and full speed
        assert_eq!(
            lighting_frame(2, 5, 2),
            [0x07, 0xFF, 0xFF, 0x02, 0x05, 0x02, 0x00, 0x00]
        );
    }

    #[test]
    fn values_are_clamped() {
        assert_eq!(lighting_frame(5, 99, 99)[4], MAX_BRIGHTNESS);
        assert_eq!(lighting_frame(5, 99, 99)[5], MAX_SPEED);
    }

    #[test]
    fn effect_table_is_contiguous_and_unique() {
        for (i, e) in EFFECTS.iter().enumerate() {
            assert_eq!(e.index as usize, i, "effects must be 0-based and contiguous");
        }
        // 常亮 sits at 0 — the row-1 / data-0 quirk that used to make
        // MiaHUB light only the gaming keys
        assert_eq!(EFFECTS[0].index, DEFAULT_EFFECT);
        assert_eq!(EFFECTS[0].en, "Steady");
        assert!(EFFECTS[1].hidden);
    }

    #[test]
    fn envelope_matches_captured_dongle_traffic() {
        // All three lines are verbatim from research/capture_24g.log, captured
        // while driving the official ShinetekTools.exe in 2.4G mode.
        //
        // 00 bb aa 99 88 aa 0a 05 02 00 …   踏雪无痕
        // 00 bb aa 99 88 aa 00 05 00 00 …   常亮
        // 00 bb aa 99 88 aa 13 05 02 00 …   正弦光波
        let f = envelope_24g(10, 5, 2);
        assert_eq!(f.len(), 65);
        assert_eq!(
            &f[..10],
            &[0x00, 0xBB, 0xAA, 0x99, 0x88, 0xAA, 0x0A, 0x05, 0x02, 0x00]
        );
        assert!(f[10..].iter().all(|b| *b == 0));

        assert_eq!(
            &envelope_24g(0, 5, 0)[..9],
            &[0x00, 0xBB, 0xAA, 0x99, 0x88, 0xAA, 0x00, 0x05, 0x00]
        );
        assert_eq!(
            &envelope_24g(19, 5, 2)[..9],
            &[0x00, 0xBB, 0xAA, 0x99, 0x88, 0xAA, 0x13, 0x05, 0x02]
        );
    }

    /// The payload must sit at [6]/[7]/[8] — an extra magic byte shifting it
    /// right is the bug that left the keyboard stuck on its own default effect.
    #[test]
    fn envelope_payload_offsets_are_fixed() {
        let f = envelope_24g(7, 3, 1);
        assert_eq!(f[5], CMD_24G_LIGHTING, "cmd belongs at offset 5");
        assert_eq!(f[6], 7, "effect belongs at offset 6");
        assert_eq!(f[7], 3, "brightness belongs at offset 7");
        assert_eq!(f[8], 1, "speed belongs at offset 8");
        assert_eq!(MAGIC_24G.len(), 4);
        // the magic's own last byte must not be confused with the cmd byte
        assert_eq!(f[1..5], MAGIC_24G);
    }

    #[test]
    fn envelope_clamps_like_the_wired_frame() {
        let f = envelope_24g(2, 99, 99);
        assert_eq!(f[7], MAX_BRIGHTNESS);
        assert_eq!(f[8], MAX_SPEED);
    }
}
