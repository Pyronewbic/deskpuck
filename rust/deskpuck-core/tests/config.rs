use deskpuck_core::config::*;
use deskpuck_core::engine::{EngineSettings, InputEngine};
use deskpuck_core::mapping::ButtonAction;
use deskpuck_core::packet::Report;
use std::collections::BTreeMap;

fn parse(json: &str) -> (Config, Vec<String>) {
    Config::from_json(json.as_bytes())
}

fn warned(warnings: &[String], fragment: &str) -> bool {
    warnings.iter().any(|w| w.contains(fragment))
}

fn mappings(pairs: &[(&str, u16)]) -> BTreeMap<String, Mapping> {
    pairs.iter().map(|&(name, code)| (name.to_owned(), Mapping::from(code))).collect()
}

#[test]
fn defaults_match_engine() {
    let config = Config::default();
    assert_eq!(
        config.key_mappings,
        mappings(&[("RS", 36), ("X", 126), ("B", 125), ("Y", 123), ("A", 124)])
    );
    assert!(config.validation_problems().is_empty());

    let from_config = config.engine_settings();
    let engine_defaults = EngineSettings::default();
    assert_eq!(from_config.key_mappings.len(), engine_defaults.key_mappings.len());
    for mapping in &engine_defaults.key_mappings {
        assert!(from_config.key_mappings.contains(mapping), "{mapping:?}");
    }
    assert_eq!(from_config.pointer_speed, engine_defaults.pointer_speed);
    assert_eq!(from_config.repeat_delay, engine_defaults.repeat_delay);
    assert_eq!(from_config.repeat_interval, engine_defaults.repeat_interval);
    assert_eq!(from_config.scroll_enabled, engine_defaults.scroll_enabled);
}

#[test]
fn mappable_button_list() {
    let buttons = mappable_buttons();
    assert!(buttons.contains(&"RS") && buttons.contains(&"A") && buttons.contains(&"HOME"));
    for mouse in ["R", "ZR", "ZL", "L", "LS"] {
        assert!(!buttons.contains(&mouse), "{mouse}");
    }
    assert_eq!(buttons.len(), 18);
}

#[test]
fn round_trip() {
    let config = Config {
        key_mappings: mappings(&[("HOME", 53), ("PLUS", 49)]),
        pointer_speed: 2.5,
        repeat_delay: 0.25,
        repeat_interval: 0.0,
        scroll_enabled: false,
    };
    let (back, warnings) = Config::from_json(&config.to_json().expect("valid config serializes"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(back, config);
}

#[test]
fn reads_the_mac_apps_output() {
    // Shape NSJSONSerialization writes: integral doubles without a fraction.
    let (config, warnings) = parse(
        r#"{
  "keyMappings" : { "A" : 49, "RS" : 36 },
  "pointerSpeed" : 1,
  "repeatDelay" : 0.4,
  "repeatInterval" : 0.06,
  "scrollEnabled" : true,
  "version" : 1
}"#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(config.key_mappings, mappings(&[("A", 49), ("RS", 36)]));
    assert_eq!(config.pointer_speed, 1.0);
}

#[test]
fn whole_file_fallbacks() {
    let (c, w) = parse("{not json");
    assert!(c == Config::default() && warned(&w, "not valid JSON"));
    let (c, w) = parse("[1, 2]");
    assert!(c == Config::default() && warned(&w, "not a JSON object"));
    let (c, w) = Config::from_json(b"");
    assert!(c == Config::default() && w.len() == 1);

    // Missing or future versions ignore otherwise valid settings.
    for json in [
        r#"{"pointerSpeed": 3}"#,
        r#"{"version": 2, "pointerSpeed": 3}"#,
        r#"{"version": true, "pointerSpeed": 3}"#,
        r#"{"version": "1", "pointerSpeed": 3}"#,
    ] {
        let (c, w) = parse(json);
        assert!(c == Config::default() && warned(&w, "version"), "{json}");
    }

    // Positive control: version 1 applies the same setting.
    let (c, w) = parse(r#"{"version": 1, "pointerSpeed": 3}"#);
    assert!(c.pointer_speed == 3.0 && w.is_empty());
}

#[test]
fn number_bounds() {
    for bad in ["0", "0.05", "10.5", "-1", "\"fast\"", "true", "null", "[]"] {
        let (c, w) = parse(&format!(r#"{{"version": 1, "pointerSpeed": {bad}}}"#));
        assert_eq!(c.pointer_speed, 1.0, "{bad}");
        assert!(warned(&w, "pointerSpeed"), "{bad}");
    }
    let (c, w) = parse(r#"{"version": 1, "pointerSpeed": 0.1}"#);
    assert!(c.pointer_speed == 0.1 && w.is_empty());
    let (c, w) = parse(r#"{"version": 1, "pointerSpeed": 10}"#);
    assert!(c.pointer_speed == 10.0 && w.is_empty());

    let (c, w) = parse(r#"{"version": 1, "repeatDelay": 5.01}"#);
    assert!(c.repeat_delay == 0.4 && warned(&w, "repeatDelay"));
    let (c, w) = parse(r#"{"version": 1, "repeatDelay": 0}"#);
    assert!(c.repeat_delay == 0.0 && w.is_empty());
    let (c, w) = parse(r#"{"version": 1, "repeatInterval": -0.1}"#);
    assert!(c.repeat_interval == 0.06 && warned(&w, "repeatInterval"));
    let (c, w) = parse(r#"{"version": 1, "repeatInterval": 3}"#);
    assert!(c.repeat_interval == 0.06 && warned(&w, "repeatInterval"));

    // The warning names the range and the value kept.
    let (_, w) = parse(r#"{"version": 1, "pointerSpeed": 0}"#);
    assert_eq!(w, ["pointerSpeed must be a number from 0.1 to 10; using 1."]);
}

#[test]
fn repeat_off_sentinel() {
    // 0 is the documented "repeat off" value: it must pass through unclamped and
    // switch repeat off, not fall back to the default (which repeats).
    let (config, w) = parse(r#"{"version": 1, "repeatInterval": 0}"#);
    assert!(config.repeat_interval == 0.0 && w.is_empty());

    let held = Report { buttons: 0x0004_0000, ..Report::default() };
    let events_while_held = |mut engine: InputEngine| {
        let mut events = 0;
        let mut t = 0.0;
        while t < 3.0 {
            events += engine.process(&held, t).keys.len();
            t += 0.03;
        }
        events
    };
    assert_eq!(events_while_held(InputEngine::new(config.engine_settings())), 1);

    // Positive control: the default interval repeats under the same hold.
    assert!(events_while_held(InputEngine::new(Config::default().engine_settings())) > 1);
}

#[test]
fn key_mappings() {
    // An explicit empty object clears every mapping; that is a choice, not an error.
    let (cleared, w) = parse(r#"{"version": 1, "keyMappings": {}}"#);
    assert!(cleared.key_mappings.is_empty() && w.is_empty());

    // A malformed section never means "no mappings".
    for bad in ["\"RS\"", "[36]", "36", "null"] {
        let (c, w) = parse(&format!(r#"{{"version": 1, "keyMappings": {bad}}}"#));
        assert_eq!(c.key_mappings, Config::default().key_mappings, "{bad}");
        assert!(warned(&w, "keyMappings must be an object"), "{bad}");
    }

    // Bad entries are dropped one by one; good ones survive.
    let (mixed, w) = parse(
        r#"{"version": 1, "keyMappings": {
            "A": 49, "Q": 36, "rs": 36, "R": 36, "LS": 36,
            "B": 128, "X": -1, "Y": 36.5, "HOME": true, "PLUS": "36", "CHAT": 0}}"#,
    );
    assert_eq!(mixed.key_mappings, mappings(&[("A", 49), ("CHAT", 0)]));
    assert!(warned(&w, "Unknown button \"Q\""));
    assert!(warned(&w, "Unknown button \"rs\""));
    assert!(warned(&w, "R is a mouse button"));
    assert!(warned(&w, "LS is a mouse button"));
    for button in ["B", "X", "Y", "HOME", "PLUS"] {
        assert!(warned(&w, &format!("Key code for {button}")), "{button}");
    }
    assert_eq!(w.len(), 9, "{w:?}");

    // Integral floats are whole numbers, as NSJSONSerialization treats them.
    let (c, w) = parse(r#"{"version": 1, "keyMappings": {"A": 36.0, "B": 127}}"#);
    assert!(c.key_mappings == mappings(&[("A", 36), ("B", 127)]) && w.is_empty());
}

#[test]
fn other_fields() {
    let (c, w) = parse(r#"{"version": 1, "scrollEnabled": false}"#);
    assert!(!c.scroll_enabled && w.is_empty());
    let (c, w) = parse(r#"{"version": 1, "scrollEnabled": 0}"#);
    assert!(c.scroll_enabled && warned(&w, "scrollEnabled"));

    let (c, w) = parse(r#"{"version": 1, "pointerSpeed": 2, "pointerSpeeed": 9}"#);
    assert!(c.pointer_speed == 2.0 && warned(&w, "Unknown setting \"pointerSpeeed\""));
}

#[test]
fn validation_problems() {
    let mut config = Config::default();
    assert!(config.validation_problems().is_empty());
    config.pointer_speed = f64::NAN;
    assert_eq!(config.validation_problems().len(), 1);
    assert!(config.to_json().is_none());

    config.pointer_speed = 1.0;
    config.key_mappings = mappings(&[("ZR", 36)]);
    assert_eq!(config.validation_problems().len(), 1);

    config.key_mappings = mappings(&[("A", 200)]);
    assert_eq!(config.validation_problems().len(), 1);

    // Positive control: the same config with a valid mapping has no problems.
    config.key_mappings = mappings(&[("A", 127)]);
    assert!(config.validation_problems().is_empty());
}

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn file_loading() {
    let dir = temp_dir();

    // Missing file: defaults, silently.
    let (c, w) = Config::load(&dir.path().join("none.json"));
    assert!(c == Config::default() && w.is_empty());

    // A directory is refused (on Windows it cannot even be opened as a file).
    let (c, w) = Config::load(dir.path());
    assert!(c == Config::default() && !w.is_empty());
    #[cfg(unix)]
    assert!(warned(&w, "not a regular file"), "{w:?}");

    // Oversized, even though it would parse.
    let mut big = String::from(r#"{"version": 1, "pointerSpeed": 3"#);
    while big.len() as u64 <= MAX_CONFIG_BYTES {
        big.push_str("                ");
    }
    big.push('}');
    let big_path = dir.path().join("big.json");
    std::fs::write(&big_path, &big).expect("write big");
    let (c, w) = Config::load(&big_path);
    assert!(c == Config::default() && warned(&w, "larger than 64 KB"), "{w:?}");

    // Positive control: a small valid file loads.
    let good_path = dir.path().join("good.json");
    std::fs::write(&good_path, r#"{"version": 1, "pointerSpeed": 3}"#).expect("write good");
    let (c, w) = Config::load(&good_path);
    assert!(c.pointer_speed == 3.0 && w.is_empty(), "{w:?}");
}

#[cfg(unix)]
#[test]
fn fifo_refused_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let dir = temp_dir();
    let fifo = dir.path().join("fifo.json");
    let c_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).expect("path");
    // SAFETY: c_path is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);

    // A blocking open would hang the test here, so the timeout is the failure signal.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(Config::load(&fifo)).expect("send"));
    let (c, w) = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("load did not block");
    assert!(c == Config::default() && warned(&w, "not a regular file"), "{w:?}");
}

#[test]
fn file_writing() {
    let dir = temp_dir();

    // Creates missing parent directories.
    let path = dir.path().join("Deskpuck").join("config.json");
    let config = Config { pointer_speed: 4.0, ..Config::default() };
    config.save(&path).expect("save");
    let (loaded, w) = Config::load(&path);
    assert!(loaded.pointer_speed == 4.0 && w.is_empty(), "{w:?}");

    // Invalid settings are refused and leave the existing file alone.
    let invalid = Config { pointer_speed: 50.0, ..Config::default() };
    assert!(matches!(invalid.save(&path), Err(SaveError::Invalid(p)) if p.len() == 1));
    assert_eq!(Config::load(&path).0.pointer_speed, 4.0);

    // No temp files left behind.
    let entries = std::fs::read_dir(path.parent().expect("parent")).expect("read dir").count();
    assert_eq!(entries, 1);
}

#[cfg(unix)]
#[test]
fn file_permissions_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    let path = dir.path().join("Deskpuck").join("config.json");
    Config::default().save(&path).expect("save");
    let mode =
        |p: &std::path::Path| std::fs::metadata(p).expect("stat").permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().expect("parent")), 0o700);
}

#[cfg(unix)]
#[test]
fn symlink_target_replaced_not_followed() {
    let dir = temp_dir();
    let victim = dir.path().join("victim.txt");
    std::fs::write(&victim, "untouched").expect("write victim");
    let link = dir.path().join("link.json");
    std::os::unix::fs::symlink(&victim, &link).expect("symlink");

    Config::default().save(&link).expect("save over symlink");
    assert_eq!(std::fs::read_to_string(&victim).expect("read victim"), "untouched");
    assert!(std::fs::symlink_metadata(&link).expect("lstat").file_type().is_file());
    // Positive control: the replaced file is the config, not an empty or stale one.
    assert_eq!(Config::load(&link), (Config::default(), Vec::new()));
}

#[test]
fn save_failure_reports_io_error() {
    let dir = temp_dir();
    // The parent "directory" is a regular file, so nothing can be created under it.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "file").expect("write blocker");
    let result = Config::default().save(&blocker.join("config.json"));
    assert!(matches!(result, Err(SaveError::Io(_))), "{result:?}");
    assert_eq!(std::fs::read_to_string(&blocker).expect("read blocker"), "file");
    // Only the blocker remains: no stray temp files beside it.
    assert_eq!(std::fs::read_dir(dir.path()).expect("read dir").count(), 1);
}

fn combo(key: u16, names: &[&str]) -> Shortcut {
    let modifiers = names.iter().fold(deskpuck_core::mapping::Modifiers::NONE, |m, n| {
        m.with(deskpuck_core::mapping::Modifiers::from_name(n).unwrap())
    });
    Shortcut { key, modifiers }
}

#[test]
fn shortcuts_load_and_save() {
    let (c, w) = parse(
        r#"{"version": 1, "keyMappings": {
            "A": {"key": 8, "modifiers": ["command", "control"]},
            "B": {"key": 9},
            "X": {"key": 7, "modifiers": []},
            "Y": 36}}"#,
    );
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.key_mappings["A"], Mapping::from(combo(8, &["control", "command"])));
    assert_eq!(c.key_mappings["B"], Mapping::from(9));
    assert_eq!(c.key_mappings["X"], Mapping::from(7));

    // Plain keys stay bare numbers on disk; shortcuts list modifiers in pressing order.
    let value = c.to_value();
    assert_eq!(value["keyMappings"]["Y"], 36);
    assert_eq!(value["keyMappings"]["B"], 9);
    assert_eq!(
        value["keyMappings"]["A"],
        serde_json::json!({"key": 8, "modifiers": ["control", "command"]})
    );
    let (back, w) = Config::from_json(&c.to_json().expect("valid"));
    assert!(w.is_empty() && back == c, "{w:?}");

    // The engine gets the modifiers.
    let settings = c.engine_settings();
    let a = settings.key_mappings.iter().find(|m| m.key_code() == Some(8)).expect("A mapped");
    let modifiers = combo(8, &["control", "command"]).modifiers;
    assert_eq!(a.action, ButtonAction::Key { key_code: 8, modifiers });
}

#[test]
fn bad_shortcuts_are_dropped_one_by_one() {
    let (c, w) = parse(
        r#"{"version": 1, "keyMappings": {
            "A": {"key": 8, "modifiers": ["hyper"]},
            "B": {"key": 8, "modifiers": ["shift", "shift"]},
            "X": {"key": 8, "modifiers": "shift"},
            "Y": {"key": 8, "repeat": true},
            "PLUS": {"modifiers": ["shift"]},
            "HOME": {"key": 300, "modifiers": ["shift"]},
            "CHAT": {"key": 8, "modifiers": [1]},
            "SL": {"key": 8, "modifiers": ["Shift"]},
            "SR": {"key": 9, "modifiers": ["option"]}}}"#,
    );
    assert_eq!(
        c.key_mappings,
        BTreeMap::from([("SR".to_owned(), Mapping::from(combo(9, &["option"])))])
    );
    assert!(warned(&w, "Unknown modifier \"hyper\" for A"), "{w:?}");
    assert!(warned(&w, "Modifier \"shift\" is listed twice for B"), "{w:?}");
    assert!(warned(&w, "Modifiers for X must be a list"), "{w:?}");
    assert!(warned(&w, "Shortcut for Y has an unknown field \"repeat\""), "{w:?}");
    assert!(warned(&w, "Key code for PLUS"), "{w:?}");
    assert!(warned(&w, "Key code for HOME"), "{w:?}");
    assert!(warned(&w, "Unknown modifier \"1\" for CHAT"), "{w:?}");
    assert!(warned(&w, "Unknown modifier \"Shift\" for SL"), "names are lower case: {w:?}");
    assert_eq!(w.len(), 8, "{w:?}");
    assert!(w.iter().all(|w| w.ends_with("mapping ignored.")), "{w:?}");
}

#[test]
fn modifier_buttons_load_and_save() {
    use deskpuck_core::mapping::Modifiers;
    let (c, w) = parse(
        r#"{"version": 1, "keyMappings": {
            "A": {"modifier": "shift"},
            "B": {"modifier": "command", "latch": true}}}"#,
    );
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(c.key_mappings["A"], Mapping::Modifier { modifier: Modifiers::SHIFT, latch: false });
    assert_eq!(
        c.key_mappings["B"],
        Mapping::Modifier { modifier: Modifiers::COMMAND, latch: true }
    );
    assert_eq!(
        c.to_value()["keyMappings"]["A"],
        serde_json::json!({"modifier": "shift", "latch": false})
    );
    let (back, w) = Config::from_json(&c.to_json().expect("valid"));
    assert!(w.is_empty() && back == c, "{w:?}");
    let a_mask = deskpuck_core::packet::button_mask("A");
    let a = c.engine_settings().key_mappings.into_iter().find(|m| Some(m.button_mask) == a_mask);
    assert_eq!(
        a.map(|m| m.action),
        Some(ButtonAction::Modifier { modifiers: Modifiers::SHIFT, latch: false })
    );
}

#[test]
fn bad_modifier_buttons_are_dropped_one_by_one() {
    let (c, w) = parse(
        r#"{"version": 1, "keyMappings": {
            "A": {"modifier": "hyper"},
            "B": {"modifier": ["shift"]},
            "X": {"modifier": "shift", "latch": "yes"},
            "Y": {"modifier": "shift", "key": 8},
            "PLUS": {"modifier": "Shift"},
            "SR": {"modifier": "option", "latch": true}}}"#,
    );
    assert_eq!(c.key_mappings.len(), 1, "{:?}", c.key_mappings);
    assert!(c.key_mappings.contains_key("SR"));
    assert!(warned(&w, r#"Unknown modifier "hyper" for A"#), "{w:?}");
    assert!(warned(&w, r#"Unknown modifier ["shift"] for B"#), "{w:?}");
    assert!(warned(&w, "latch for X must be true or false"), "{w:?}");
    assert!(warned(&w, r#"Modifier button Y has an unknown field "key""#), "{w:?}");
    assert!(warned(&w, r#"Unknown modifier "Shift" for PLUS"#), "{w:?}");
    assert_eq!(w.len(), 5, "{w:?}");
}
