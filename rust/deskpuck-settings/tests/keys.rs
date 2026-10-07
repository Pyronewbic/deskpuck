use deskpuck_core::config::{Mapping, Shortcut};
use deskpuck_core::mapping::Modifiers;
use deskpuck_inject::keymap::KEYS;
use deskpuck_settings::keys::{MENU_KEYS, choices, describe, key_name};
use deskpuck_settings::recorder::{Recorded, clipboard_key, mac_name, record};
use eframe::egui::{Key, Modifiers as Held};
use std::collections::HashSet;

#[test]
fn every_key_has_a_distinct_readable_name() {
    let names: Vec<String> = KEYS.iter().map(|row| key_name(row.mac)).collect();
    for (row, name) in KEYS.iter().zip(&names) {
        assert!(!name.is_empty() && !name.contains('_'), "{} -> {name:?}", row.mac_name);
        assert!(!name.starts_with("Key code"), "{} has no name", row.mac_name);
    }
    assert_eq!(names.iter().collect::<HashSet<_>>().len(), names.len(), "{names:?}");
    assert_eq!(key_name(0x7F), "Key code 127", "control: a code outside the table");
}

#[test]
fn the_menu_offers_the_mac_keys_under_pc_names() {
    let names: Vec<String> = MENU_KEYS.iter().map(|&k| key_name(k)).collect();
    assert_eq!(
        names,
        [
            "Enter",
            "Escape",
            "Space",
            "Tab",
            "Backspace",
            "Delete",
            "Up Arrow",
            "Down Arrow",
            "Left Arrow",
            "Right Arrow",
            "Page Up",
            "Page Down",
            "Home",
            "End"
        ]
    );
    let all = choices();
    assert_eq!(all.len(), 1 + MENU_KEYS.len() + 8);
    assert_eq!(all[0], None);
    for (i, choice) in all.iter().enumerate() {
        assert!(!all[i + 1..].contains(choice), "{choice:?} is listed twice");
    }
}

#[test]
fn mappings_read_like_the_keys_you_press() {
    let shortcut = |key, modifiers| Mapping::Shortcut(Shortcut { key, modifiers });
    let ctrl_shift = Modifiers::SHIFT.with(Modifiers::CONTROL);
    assert_eq!(describe(&shortcut(0x08, ctrl_shift)), "Ctrl+Shift+C");
    assert_eq!(describe(&shortcut(0x24, Modifiers::NONE)), "Enter");
    let held = Mapping::Modifier { modifier: Modifiers::OPTION, latch: false };
    assert_eq!(describe(&held), "Alt (while held)");
    let latched = Mapping::Modifier { modifier: Modifiers::SHIFT, latch: true };
    assert_eq!(describe(&latched), "Shift (tap to latch)");
    let super_key = Mapping::Modifier { modifier: Modifiers::COMMAND, latch: false };
    let want = if cfg!(windows) { "Windows (while held)" } else { "Super (while held)" };
    assert_eq!(describe(&super_key), want);
}

#[test]
fn every_recordable_key_exists_in_the_key_table() {
    let mut codes = HashSet::new();
    let mut recordable = 0;
    for &key in Key::ALL {
        let Some(name) = mac_name(key) else { continue };
        recordable += 1;
        let row = KEYS.iter().find(|row| row.mac_name == name);
        let row = row.unwrap_or_else(|| panic!("{key:?} -> {name} is not in the key table"));
        assert!(codes.insert(row.mac), "{key:?} shares a key code");
    }
    assert!(recordable >= 80, "only {recordable} keys can be recorded");
}

fn held(ctrl: bool, alt: bool, shift: bool) -> Held {
    Held { ctrl, alt, shift, ..Held::NONE }
}

#[test]
fn a_key_press_becomes_a_shortcut_by_position() {
    let shortcut =
        |key, modifiers| Recorded::Shortcut(Mapping::Shortcut(Shortcut { key, modifiers }));
    assert_eq!(
        record(Key::C, Some(Key::C), held(true, false, true)),
        shortcut(0x08, Modifiers::CONTROL.with(Modifiers::SHIFT))
    );
    // On AZERTY the key in Q's place types A: the position is what is stored.
    assert_eq!(record(Key::A, Some(Key::Q), Held::NONE), shortcut(0x0C, Modifiers::NONE));
    assert_eq!(record(Key::A, None, held(false, true, false)), shortcut(0x00, Modifiers::OPTION));
    assert_eq!(record(Key::F5, Some(Key::F5), Held::NONE), shortcut(0x60, Modifiers::NONE));
}

#[test]
fn escape_alone_cancels_and_unknown_keys_are_skipped() {
    assert_eq!(record(Key::Escape, Some(Key::Escape), Held::NONE), Recorded::Cancel);
    assert_eq!(
        record(Key::Escape, Some(Key::Escape), held(false, false, true)),
        Recorded::Shortcut(Mapping::Shortcut(Shortcut { key: 0x35, modifiers: Modifiers::SHIFT }))
    );
    assert_eq!(record(Key::F35, Some(Key::F35), Held::NONE), Recorded::Unknown);
    assert_eq!(record(Key::Copy, None, held(true, false, false)), Recorded::Unknown);
}

#[test]
fn keypad_plus_and_clipboard_shortcuts_can_be_recorded() {
    let plain =
        |key| Recorded::Shortcut(Mapping::Shortcut(Shortcut { key, modifiers: Modifiers::NONE }));
    assert_eq!(record(Key::Plus, Some(Key::Plus), Held::NONE), plain(0x45), "keypad +");
    assert_eq!(record(Key::Equals, Some(Key::Equals), Held::NONE), plain(0x18), "control: =");

    use eframe::egui::Event;
    assert_eq!(clipboard_key(&Event::Copy), Some(Key::C));
    assert_eq!(clipboard_key(&Event::Cut), Some(Key::X));
    assert_eq!(clipboard_key(&Event::Paste(String::new())), Some(Key::V));
    assert_eq!(clipboard_key(&Event::Text("c".into())), None);
}
