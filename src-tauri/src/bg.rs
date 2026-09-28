//! Storage for the user's custom background image.
//!
//! Kept as a raw file next to `settings.json` rather than inside it: a 4 MB
//! photo is ~5.3 MB of base64, which has no business living in a JSON config
//! that gets rewritten every time you nudge a slider.
//!
//! Base64 is hand-rolled (~40 lines) instead of pulling in a crate — the only
//! thing we need it for is the one-shot IPC trip in each direction.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn decode(s: &str) -> Result<Vec<u8>, String> {
    let mut rev = [255u8; 256];
    for (i, c) in ALPHABET.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for c in s.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' {
            continue;
        }
        let v = rev[c as usize];
        if v == 255 {
            return Err("背景图数据不是合法的 base64".into());
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("background.bin"))
}

pub fn save(app: &AppHandle, bytes: &[u8]) -> Result<(), String> {
    let p = path(app).ok_or("拿不到应用配置目录")?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, bytes).map_err(|e| e.to_string())
}

pub fn load(app: &AppHandle) -> Option<Vec<u8>> {
    let p = path(app)?;
    std::fs::read(p).ok().filter(|b| !b.is_empty())
}

pub fn clear(app: &AppHandle) {
    if let Some(p) = path(app) {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_padding_case() {
        for raw in [
            &b""[..],
            &b"f"[..],
            &b"fo"[..],
            &b"foo"[..],
            &b"foob"[..],
            &b"fooba"[..],
            &b"foobar"[..],
            &[0u8, 255, 128, 1, 2, 3, 254][..],
        ] {
            let enc = encode(raw);
            assert_eq!(decode(&enc).unwrap(), raw, "round trip failed for {raw:?}");
        }
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn decode_rejects_junk() {
        assert!(decode("!!!!").is_err());
    }
}
