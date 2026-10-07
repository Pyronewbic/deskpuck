//! Uses the physical key: config.json stores key positions, so a recorded
//! shortcut types the same thing on any layout.

use crate::keys::code_for;
use deskpuck_core::config::{Mapping, Shortcut};
use deskpuck_core::mapping::Modifiers;
use eframe::egui::{self, Key};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    Shortcut(Mapping),
    Cancel,
    Unknown,
}

pub fn mac_name(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::A => "ANSI_A",
        Key::B => "ANSI_B",
        Key::C => "ANSI_C",
        Key::D => "ANSI_D",
        Key::E => "ANSI_E",
        Key::F => "ANSI_F",
        Key::G => "ANSI_G",
        Key::H => "ANSI_H",
        Key::I => "ANSI_I",
        Key::J => "ANSI_J",
        Key::K => "ANSI_K",
        Key::L => "ANSI_L",
        Key::M => "ANSI_M",
        Key::N => "ANSI_N",
        Key::O => "ANSI_O",
        Key::P => "ANSI_P",
        Key::Q => "ANSI_Q",
        Key::R => "ANSI_R",
        Key::S => "ANSI_S",
        Key::T => "ANSI_T",
        Key::U => "ANSI_U",
        Key::V => "ANSI_V",
        Key::W => "ANSI_W",
        Key::X => "ANSI_X",
        Key::Y => "ANSI_Y",
        Key::Z => "ANSI_Z",
        Key::Num0 => "ANSI_0",
        Key::Num1 => "ANSI_1",
        Key::Num2 => "ANSI_2",
        Key::Num3 => "ANSI_3",
        Key::Num4 => "ANSI_4",
        Key::Num5 => "ANSI_5",
        Key::Num6 => "ANSI_6",
        Key::Num7 => "ANSI_7",
        Key::Num8 => "ANSI_8",
        Key::Num9 => "ANSI_9",
        Key::Minus => "ANSI_Minus",
        Key::Equals => "ANSI_Equal",
        // As a physical key, egui reports Plus only for the keypad's +.
        Key::Plus => "ANSI_KeypadPlus",
        Key::OpenBracket => "ANSI_LeftBracket",
        Key::CloseBracket => "ANSI_RightBracket",
        Key::Backslash => "ANSI_Backslash",
        Key::Semicolon => "ANSI_Semicolon",
        Key::Quote => "ANSI_Quote",
        Key::Backtick => "ANSI_Grave",
        Key::Comma => "ANSI_Comma",
        Key::Period => "ANSI_Period",
        Key::Slash => "ANSI_Slash",
        Key::Enter => "Return",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Backspace => "Delete",
        Key::Delete => "ForwardDelete",
        Key::Insert => "Help",
        Key::Escape => "Escape",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::ArrowUp => "UpArrow",
        Key::ArrowDown => "DownArrow",
        Key::ArrowLeft => "LeftArrow",
        Key::ArrowRight => "RightArrow",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::F13 => "F13",
        Key::F14 => "F14",
        Key::F15 => "F15",
        Key::F16 => "F16",
        Key::F17 => "F17",
        Key::F18 => "F18",
        Key::F19 => "F19",
        Key::F20 => "F20",
        _ => return None,
    })
}

/// Ctrl, Alt and Shift; egui does not report the Super/Windows key, so a
/// shortcut using it is chosen from the menu's modifier entries instead.
pub fn modifiers(held: egui::Modifiers) -> Modifiers {
    [(held.ctrl, Modifiers::CONTROL), (held.alt, Modifiers::OPTION), (held.shift, Modifiers::SHIFT)]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(Modifiers::NONE, |all, (_, m)| all.with(m))
}

/// egui turns Ctrl+C/X/V (and on Windows Ctrl+Insert, Shift+Delete, Shift+Insert)
/// into these events instead of key presses; Ctrl is held.
pub fn clipboard_key(event: &egui::Event) -> Option<Key> {
    match event {
        egui::Event::Copy => Some(Key::C),
        egui::Event::Cut => Some(Key::X),
        egui::Event::Paste(_) => Some(Key::V),
        _ => None,
    }
}

pub fn record(key: Key, physical_key: Option<Key>, held: egui::Modifiers) -> Recorded {
    let key = physical_key.unwrap_or(key);
    let modifiers = modifiers(held);
    if key == Key::Escape && modifiers.is_empty() {
        return Recorded::Cancel;
    }
    match mac_name(key).and_then(code_for) {
        Some(code) => Recorded::Shortcut(Mapping::Shortcut(Shortcut { key: code, modifiers })),
        None => Recorded::Unknown,
    }
}
