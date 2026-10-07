//! What the settings window calls keys, shortcuts and modifiers. Key codes
//! are macOS virtual key codes, as in config.json; names follow a PC
//! keyboard, since this window runs on Linux and Windows.

use deskpuck_core::config::Mapping;
use deskpuck_core::mapping::{KeyCode, Modifiers};
use deskpuck_inject::keymap::KEYS;

/// The keys offered in every button's menu, as on the Mac.
pub const MENU_KEYS: [KeyCode; 14] = [
    0x24, // Return
    0x35, // Escape
    0x31, // Space
    0x30, // Tab
    0x33, // Delete (Backspace)
    0x75, // Forward Delete
    0x7E, // Up Arrow
    0x7D, // Down Arrow
    0x7B, // Left Arrow
    0x7C, // Right Arrow
    0x74, // Page Up
    0x79, // Page Down
    0x73, // Home
    0x77, // End
];

/// The key code for a macOS key name as `deskpuck-inject`'s table spells it.
pub fn code_for(mac_name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|row| row.mac_name == mac_name).map(|row| row.mac)
}

pub fn modifier_label(modifier: Modifiers) -> &'static str {
    match modifier {
        Modifiers::CONTROL => "Ctrl",
        Modifiers::OPTION => "Alt",
        Modifiers::SHIFT => "Shift",
        Modifiers::COMMAND if cfg!(windows) => "Windows",
        Modifiers::COMMAND => "Super",
        _ => "?",
    }
}

/// The modifiers in the order the core presses them.
pub fn modifiers() -> impl Iterator<Item = Modifiers> {
    Modifiers::ALL.iter().map(|(_, modifier, _)| *modifier)
}

pub fn key_name(code: KeyCode) -> String {
    let Some(row) = KEYS.iter().find(|row| row.mac == code) else {
        return format!("Key code {code}");
    };
    let name = row.mac_name;
    let fixed = match name {
        "Return" => "Enter",
        "Delete" => "Backspace",
        "ForwardDelete" => "Delete",
        "Help" => "Insert",
        "UpArrow" => "Up Arrow",
        "DownArrow" => "Down Arrow",
        "LeftArrow" => "Left Arrow",
        "RightArrow" => "Right Arrow",
        "PageUp" => "Page Up",
        "PageDown" => "Page Down",
        "CapsLock" => "Caps Lock",
        "VolumeUp" => "Volume Up",
        "VolumeDown" => "Volume Down",
        "Control" => "Ctrl",
        "RightControl" => "Right Ctrl",
        "Option" => "Alt",
        "RightOption" => "Right Alt",
        "RightShift" => "Right Shift",
        "Command" => modifier_label(Modifiers::COMMAND),
        "RightCommand" if cfg!(windows) => "Right Windows",
        "RightCommand" => "Right Super",
        "ISO_Section" => "Section",
        "ANSI_Equal" => "=",
        "ANSI_Minus" => "-",
        "ANSI_LeftBracket" => "[",
        "ANSI_RightBracket" => "]",
        "ANSI_Quote" => "'",
        "ANSI_Semicolon" => ";",
        "ANSI_Backslash" => "\\",
        "ANSI_Comma" => ",",
        "ANSI_Slash" => "/",
        "ANSI_Period" => ".",
        "ANSI_Grave" => "`",
        "ANSI_KeypadDecimal" => "Keypad .",
        "ANSI_KeypadMultiply" => "Keypad *",
        "ANSI_KeypadPlus" => "Keypad +",
        "ANSI_KeypadMinus" => "Keypad -",
        "ANSI_KeypadDivide" => "Keypad /",
        "ANSI_KeypadEquals" => "Keypad =",
        "ANSI_KeypadEnter" => "Keypad Enter",
        "ANSI_KeypadClear" => "Num Lock",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_owned();
    }
    if let Some(digit) = name.strip_prefix("ANSI_Keypad") {
        return format!("Keypad {digit}");
    }
    name.strip_prefix("ANSI_").unwrap_or(name).to_owned()
}

/// "Ctrl+Shift+C", or "Shift (while held)" for a modifier button.
pub fn describe(mapping: &Mapping) -> String {
    match mapping {
        Mapping::Shortcut(shortcut) => {
            let mut parts: Vec<String> = modifiers()
                .filter(|m| shortcut.modifiers.contains(*m))
                .map(|m| modifier_label(m).to_owned())
                .collect();
            parts.push(key_name(shortcut.key));
            parts.join("+")
        }
        Mapping::Modifier { modifier, latch } => {
            let how = if *latch { "tap to latch" } else { "while held" };
            format!("{} ({how})", modifier_label(*modifier))
        }
    }
}

/// Every menu entry except "Record Shortcut...": nothing, the menu keys,
/// then each modifier held and latched.
pub fn choices() -> Vec<Option<Mapping>> {
    let mut all = vec![None];
    all.extend(MENU_KEYS.iter().map(|&key| Some(Mapping::from(key))));
    for latch in [false, true] {
        all.extend(modifiers().map(|modifier| Some(Mapping::Modifier { modifier, latch })));
    }
    all
}
