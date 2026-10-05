//! Translates macOS virtual key codes (the config's key space) to Linux evdev
//! key codes and Windows virtual keys. Each row keeps the header name of every
//! number so tests can check the table against each OS's own definitions.

use deskpuck_core::mapping::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRow {
    /// `kVK_` name in HIToolbox/Events.h, without the prefix.
    pub mac_name: &'static str,
    pub mac: KeyCode,
    /// Name in linux/input-event-codes.h.
    pub linux_name: &'static str,
    pub linux: u16,
    /// windows-sys constant name, or the character for letters and digits.
    pub vk_name: &'static str,
    pub vk: u16,
    /// Windows needs KEYEVENTF_EXTENDEDKEY for these to reach apps as the right key.
    pub extended: bool,
}

macro_rules! rows {
    ($( $mac_name:literal $mac:literal $linux_name:literal $linux:literal $vk_name:literal $vk:literal $ext:literal ),* $(,)?) => {
        &[$( KeyRow {
            mac_name: $mac_name, mac: $mac, linux_name: $linux_name, linux: $linux,
            vk_name: $vk_name, vk: $vk, extended: $ext,
        } ),*]
    };
}

/// Fn and the JIS-only keys have no PC equivalent and are left out.
#[rustfmt::skip]
pub const KEYS: &[KeyRow] = rows![
    "ANSI_A" 0x00 "KEY_A" 30 "A" 0x41 false,
    "ANSI_S" 0x01 "KEY_S" 31 "S" 0x53 false,
    "ANSI_D" 0x02 "KEY_D" 32 "D" 0x44 false,
    "ANSI_F" 0x03 "KEY_F" 33 "F" 0x46 false,
    "ANSI_H" 0x04 "KEY_H" 35 "H" 0x48 false,
    "ANSI_G" 0x05 "KEY_G" 34 "G" 0x47 false,
    "ANSI_Z" 0x06 "KEY_Z" 44 "Z" 0x5A false,
    "ANSI_X" 0x07 "KEY_X" 45 "X" 0x58 false,
    "ANSI_C" 0x08 "KEY_C" 46 "C" 0x43 false,
    "ANSI_V" 0x09 "KEY_V" 47 "V" 0x56 false,
    "ISO_Section" 0x0A "KEY_102ND" 86 "VK_OEM_102" 0xE2 false,
    "ANSI_B" 0x0B "KEY_B" 48 "B" 0x42 false,
    "ANSI_Q" 0x0C "KEY_Q" 16 "Q" 0x51 false,
    "ANSI_W" 0x0D "KEY_W" 17 "W" 0x57 false,
    "ANSI_E" 0x0E "KEY_E" 18 "E" 0x45 false,
    "ANSI_R" 0x0F "KEY_R" 19 "R" 0x52 false,
    "ANSI_Y" 0x10 "KEY_Y" 21 "Y" 0x59 false,
    "ANSI_T" 0x11 "KEY_T" 20 "T" 0x54 false,
    "ANSI_1" 0x12 "KEY_1" 2 "1" 0x31 false,
    "ANSI_2" 0x13 "KEY_2" 3 "2" 0x32 false,
    "ANSI_3" 0x14 "KEY_3" 4 "3" 0x33 false,
    "ANSI_4" 0x15 "KEY_4" 5 "4" 0x34 false,
    "ANSI_6" 0x16 "KEY_6" 7 "6" 0x36 false,
    "ANSI_5" 0x17 "KEY_5" 6 "5" 0x35 false,
    "ANSI_Equal" 0x18 "KEY_EQUAL" 13 "VK_OEM_PLUS" 0xBB false,
    "ANSI_9" 0x19 "KEY_9" 10 "9" 0x39 false,
    "ANSI_7" 0x1A "KEY_7" 8 "7" 0x37 false,
    "ANSI_Minus" 0x1B "KEY_MINUS" 12 "VK_OEM_MINUS" 0xBD false,
    "ANSI_8" 0x1C "KEY_8" 9 "8" 0x38 false,
    "ANSI_0" 0x1D "KEY_0" 11 "0" 0x30 false,
    "ANSI_RightBracket" 0x1E "KEY_RIGHTBRACE" 27 "VK_OEM_6" 0xDD false,
    "ANSI_O" 0x1F "KEY_O" 24 "O" 0x4F false,
    "ANSI_U" 0x20 "KEY_U" 22 "U" 0x55 false,
    "ANSI_LeftBracket" 0x21 "KEY_LEFTBRACE" 26 "VK_OEM_4" 0xDB false,
    "ANSI_I" 0x22 "KEY_I" 23 "I" 0x49 false,
    "ANSI_P" 0x23 "KEY_P" 25 "P" 0x50 false,
    "Return" 0x24 "KEY_ENTER" 28 "VK_RETURN" 0x0D false,
    "ANSI_L" 0x25 "KEY_L" 38 "L" 0x4C false,
    "ANSI_J" 0x26 "KEY_J" 36 "J" 0x4A false,
    "ANSI_Quote" 0x27 "KEY_APOSTROPHE" 40 "VK_OEM_7" 0xDE false,
    "ANSI_K" 0x28 "KEY_K" 37 "K" 0x4B false,
    "ANSI_Semicolon" 0x29 "KEY_SEMICOLON" 39 "VK_OEM_1" 0xBA false,
    "ANSI_Backslash" 0x2A "KEY_BACKSLASH" 43 "VK_OEM_5" 0xDC false,
    "ANSI_Comma" 0x2B "KEY_COMMA" 51 "VK_OEM_COMMA" 0xBC false,
    "ANSI_Slash" 0x2C "KEY_SLASH" 53 "VK_OEM_2" 0xBF false,
    "ANSI_N" 0x2D "KEY_N" 49 "N" 0x4E false,
    "ANSI_M" 0x2E "KEY_M" 50 "M" 0x4D false,
    "ANSI_Period" 0x2F "KEY_DOT" 52 "VK_OEM_PERIOD" 0xBE false,
    "Tab" 0x30 "KEY_TAB" 15 "VK_TAB" 0x09 false,
    "Space" 0x31 "KEY_SPACE" 57 "VK_SPACE" 0x20 false,
    "ANSI_Grave" 0x32 "KEY_GRAVE" 41 "VK_OEM_3" 0xC0 false,
    // Mac Delete is the backspace key.
    "Delete" 0x33 "KEY_BACKSPACE" 14 "VK_BACK" 0x08 false,
    "Escape" 0x35 "KEY_ESC" 1 "VK_ESCAPE" 0x1B false,
    "RightCommand" 0x36 "KEY_RIGHTMETA" 126 "VK_RWIN" 0x5C true,
    "Command" 0x37 "KEY_LEFTMETA" 125 "VK_LWIN" 0x5B true,
    "Shift" 0x38 "KEY_LEFTSHIFT" 42 "VK_LSHIFT" 0xA0 false,
    "CapsLock" 0x39 "KEY_CAPSLOCK" 58 "VK_CAPITAL" 0x14 false,
    "Option" 0x3A "KEY_LEFTALT" 56 "VK_LMENU" 0xA4 false,
    "Control" 0x3B "KEY_LEFTCTRL" 29 "VK_LCONTROL" 0xA2 false,
    "RightShift" 0x3C "KEY_RIGHTSHIFT" 54 "VK_RSHIFT" 0xA1 false,
    "RightOption" 0x3D "KEY_RIGHTALT" 100 "VK_RMENU" 0xA5 true,
    "RightControl" 0x3E "KEY_RIGHTCTRL" 97 "VK_RCONTROL" 0xA3 true,
    "F17" 0x40 "KEY_F17" 187 "VK_F17" 0x80 false,
    "ANSI_KeypadDecimal" 0x41 "KEY_KPDOT" 83 "VK_DECIMAL" 0x6E false,
    "ANSI_KeypadMultiply" 0x43 "KEY_KPASTERISK" 55 "VK_MULTIPLY" 0x6A false,
    "ANSI_KeypadPlus" 0x45 "KEY_KPPLUS" 78 "VK_ADD" 0x6B false,
    // Clear sits where PC keyboards have Num Lock.
    "ANSI_KeypadClear" 0x47 "KEY_NUMLOCK" 69 "VK_NUMLOCK" 0x90 true,
    "VolumeUp" 0x48 "KEY_VOLUMEUP" 115 "VK_VOLUME_UP" 0xAF true,
    "VolumeDown" 0x49 "KEY_VOLUMEDOWN" 114 "VK_VOLUME_DOWN" 0xAE true,
    "Mute" 0x4A "KEY_MUTE" 113 "VK_VOLUME_MUTE" 0xAD true,
    "ANSI_KeypadDivide" 0x4B "KEY_KPSLASH" 98 "VK_DIVIDE" 0x6F true,
    "ANSI_KeypadEnter" 0x4C "KEY_KPENTER" 96 "VK_RETURN" 0x0D true,
    "ANSI_KeypadMinus" 0x4E "KEY_KPMINUS" 74 "VK_SUBTRACT" 0x6D false,
    "F18" 0x4F "KEY_F18" 188 "VK_F18" 0x81 false,
    "F19" 0x50 "KEY_F19" 189 "VK_F19" 0x82 false,
    "ANSI_KeypadEquals" 0x51 "KEY_KPEQUAL" 117 "VK_OEM_NEC_EQUAL" 0x92 false,
    "ANSI_Keypad0" 0x52 "KEY_KP0" 82 "VK_NUMPAD0" 0x60 false,
    "ANSI_Keypad1" 0x53 "KEY_KP1" 79 "VK_NUMPAD1" 0x61 false,
    "ANSI_Keypad2" 0x54 "KEY_KP2" 80 "VK_NUMPAD2" 0x62 false,
    "ANSI_Keypad3" 0x55 "KEY_KP3" 81 "VK_NUMPAD3" 0x63 false,
    "ANSI_Keypad4" 0x56 "KEY_KP4" 75 "VK_NUMPAD4" 0x64 false,
    "ANSI_Keypad5" 0x57 "KEY_KP5" 76 "VK_NUMPAD5" 0x65 false,
    "ANSI_Keypad6" 0x58 "KEY_KP6" 77 "VK_NUMPAD6" 0x66 false,
    "ANSI_Keypad7" 0x59 "KEY_KP7" 71 "VK_NUMPAD7" 0x67 false,
    "F20" 0x5A "KEY_F20" 190 "VK_F20" 0x83 false,
    "ANSI_Keypad8" 0x5B "KEY_KP8" 72 "VK_NUMPAD8" 0x68 false,
    "ANSI_Keypad9" 0x5C "KEY_KP9" 73 "VK_NUMPAD9" 0x69 false,
    "F5" 0x60 "KEY_F5" 63 "VK_F5" 0x74 false,
    "F6" 0x61 "KEY_F6" 64 "VK_F6" 0x75 false,
    "F7" 0x62 "KEY_F7" 65 "VK_F7" 0x76 false,
    "F3" 0x63 "KEY_F3" 61 "VK_F3" 0x72 false,
    "F8" 0x64 "KEY_F8" 66 "VK_F8" 0x77 false,
    "F9" 0x65 "KEY_F9" 67 "VK_F9" 0x78 false,
    "F11" 0x67 "KEY_F11" 87 "VK_F11" 0x7A false,
    "F13" 0x69 "KEY_F13" 183 "VK_F13" 0x7C false,
    "F16" 0x6A "KEY_F16" 186 "VK_F16" 0x7F false,
    "F14" 0x6B "KEY_F14" 184 "VK_F14" 0x7D false,
    "F10" 0x6D "KEY_F10" 68 "VK_F10" 0x79 false,
    "F12" 0x6F "KEY_F12" 88 "VK_F12" 0x7B false,
    "F15" 0x71 "KEY_F15" 185 "VK_F15" 0x7E false,
    // Help sits where PC keyboards have Insert.
    "Help" 0x72 "KEY_INSERT" 110 "VK_INSERT" 0x2D true,
    "Home" 0x73 "KEY_HOME" 102 "VK_HOME" 0x24 true,
    "PageUp" 0x74 "KEY_PAGEUP" 104 "VK_PRIOR" 0x21 true,
    "ForwardDelete" 0x75 "KEY_DELETE" 111 "VK_DELETE" 0x2E true,
    "F4" 0x76 "KEY_F4" 62 "VK_F4" 0x73 false,
    "End" 0x77 "KEY_END" 107 "VK_END" 0x23 true,
    "F2" 0x78 "KEY_F2" 60 "VK_F2" 0x71 false,
    "PageDown" 0x79 "KEY_PAGEDOWN" 109 "VK_NEXT" 0x22 true,
    "F1" 0x7A "KEY_F1" 59 "VK_F1" 0x70 false,
    "LeftArrow" 0x7B "KEY_LEFT" 105 "VK_LEFT" 0x25 true,
    "RightArrow" 0x7C "KEY_RIGHT" 106 "VK_RIGHT" 0x27 true,
    "DownArrow" 0x7D "KEY_DOWN" 108 "VK_DOWN" 0x28 true,
    "UpArrow" 0x7E "KEY_UP" 103 "VK_UP" 0x26 true,
];

pub fn lookup(mac: KeyCode) -> Option<&'static KeyRow> {
    KEYS.iter().find(|row| row.mac == mac)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_column_is_unique() {
        assert_eq!(KEYS.iter().map(|r| r.mac).collect::<HashSet<_>>().len(), KEYS.len());
        assert_eq!(KEYS.iter().map(|r| r.linux).collect::<HashSet<_>>().len(), KEYS.len());
        // Return and keypad Enter share VK_RETURN and differ by the extended flag.
        assert_eq!(
            KEYS.iter().map(|r| (r.vk, r.extended)).collect::<HashSet<_>>().len(),
            KEYS.len()
        );
        assert!(KEYS.iter().all(|r| r.mac <= deskpuck_core::config::KEY_CODE_MAX));
    }

    #[test]
    fn letters_and_digits_use_their_ascii_virtual_key() {
        for row in KEYS.iter().filter(|r| r.vk_name.len() == 1) {
            let c = row.vk_name.as_bytes()[0];
            assert!(c.is_ascii_uppercase() || c.is_ascii_digit(), "{row:?}");
            assert_eq!(row.vk, u16::from(c), "{row:?}");
            assert_eq!(row.linux_name, format!("KEY_{}", row.vk_name), "{row:?}");
        }
        assert_eq!(KEYS.iter().filter(|r| r.vk_name.len() == 1).count(), 36);
    }

    #[test]
    fn every_key_the_mac_settings_offer_translates() {
        // Key codes from Sources/Deskpuck/KeyChoices.swift, read from the file so
        // a key added there without a row here fails this test.
        let swift = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Sources/Deskpuck/KeyChoices.swift"
        ))
        .expect("KeyChoices.swift readable");
        let codes: Vec<KeyCode> = swift
            .split("KeyChoice(code: ")
            .skip(1)
            .filter_map(|rest| rest.split(',').next()?.trim().parse().ok())
            .collect();
        // Positive control: the 14 offered keys were found (the -1 "None" entry is not a key).
        assert_eq!(codes.len(), 14, "{codes:?}");
        for code in codes {
            assert!(
                lookup(code).is_some(),
                "key code {code} offered in Settings has no translation"
            );
        }
        assert!(lookup(0x3F).is_none(), "Fn has no PC equivalent");
    }

    /// Checks each row's mac code against the SDK header it names.
    #[cfg(target_os = "macos")]
    #[test]
    fn mac_codes_match_the_sdk_header() {
        let sdk = std::process::Command::new("xcrun")
            .args(["--show-sdk-path"])
            .output()
            .expect("xcrun runs");
        let sdk = String::from_utf8(sdk.stdout).expect("utf-8").trim().to_owned();
        let header = std::fs::read_to_string(format!(
            "{sdk}/System/Library/Frameworks/Carbon.framework/Frameworks/HIToolbox.framework/Headers/Events.h"
        ))
        .expect("Events.h readable");
        let defined = parse_defines(&header, "kVK_");
        assert!(defined.len() > 100, "parsed only {} kVK_ names", defined.len());
        for row in KEYS {
            assert_eq!(defined.get(row.mac_name).copied(), Some(row.mac), "kVK_{}", row.mac_name);
        }
    }

    /// Checks each row's evdev code against the kernel header (linux-libc-dev).
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_codes_match_the_kernel_header() {
        let header = std::fs::read_to_string("/usr/include/linux/input-event-codes.h")
            .expect("/usr/include/linux/input-event-codes.h missing: install linux-libc-dev");
        let defined = parse_defines(&header, "#define ");
        assert!(defined.len() > 300, "parsed only {} defines", defined.len());
        for row in KEYS {
            assert_eq!(defined.get(row.linux_name).copied(), Some(row.linux), "{}", row.linux_name);
        }
    }

    /// Collects `<prefix>NAME = value` or `<prefix>NAME<whitespace>value` lines
    /// with decimal or hex values; anything else is skipped.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn parse_defines(header: &str, prefix: &str) -> std::collections::HashMap<String, u16> {
        header
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix(prefix)?;
                let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
                let (name, value) = rest.split_at(end);
                let value = value.trim_start().trim_start_matches('=').split_whitespace().next()?;
                let value = value.trim_end_matches(',');
                let number = match value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
                    Some(hex) => u16::from_str_radix(hex, 16).ok()?,
                    None => value.parse().ok()?,
                };
                Some((name.to_owned(), number))
            })
            .collect()
    }
}
