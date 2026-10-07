use deskpuck_core::config::{Config, Mapping, Shortcut, mappable_buttons};
use deskpuck_core::mapping::Modifiers;
use deskpuck_settings::model::{Model, RIGHT_JOYCON, SAVE_DELAY, Values};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn temp() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deskpuck").join("config.json");
    (dir, path)
}

fn saved(path: &Path) -> Config {
    let (config, warnings) = Config::load(path);
    assert!(warnings.is_empty(), "{warnings:?}");
    config
}

#[test]
fn every_shown_button_can_be_mapped() {
    let mappable = mappable_buttons();
    for row in RIGHT_JOYCON {
        assert!(mappable.contains(&row.id), "{} is not a mappable button", row.id);
    }
}

#[test]
fn a_missing_file_shows_defaults_and_is_written_on_the_first_edit() {
    let (_dir, path) = temp();
    let mut model = Model::load(Some(path.clone()));
    assert_eq!(model.values, Values::from_config(Config::default()));
    assert!(model.load_warnings.is_empty());
    assert!(!model.flush(9.0), "nothing to save before an edit");
    assert!(!path.exists());

    model.values.config.pointer_speed = 2.5;
    model.edited(1.0);
    assert!(model.save_if_due(1.0 + SAVE_DELAY));
    assert_eq!(saved(&path).pointer_speed, 2.5);
}

#[test]
fn edits_are_written_once_they_settle() {
    let (_dir, path) = temp();
    let mut model = Model::load(Some(path.clone()));
    model.set_mapping("A", Some(Mapping::from(0x31)), 1.0);
    model.set_mapping("B", None, 1.2);
    assert!(!model.save_if_due(1.2 + SAVE_DELAY - 0.01));
    assert!(!path.exists());
    let due = model.save_due_in(1.3).expect("an edit is pending");
    assert!((due - (SAVE_DELAY - 0.1)).abs() < 1e-9, "{due}");
    assert!(model.save_if_due(1.2 + SAVE_DELAY));
    let config = saved(&path);
    assert_eq!(config.key_mappings.get("A"), Some(&Mapping::from(0x31)));
    assert_eq!(config.key_mappings.get("B"), None);
    assert!(!model.save_if_due(10.0), "nothing pending after a save");
}

#[test]
fn repeat_off_saves_no_interval_and_keeps_the_rate_for_later() {
    let (_dir, path) = temp();
    let mut model = Model::load(Some(path.clone()));
    model.values.repeat_rate = 20.0;
    model.values.repeat_enabled = false;
    model.edited(0.0);
    assert!(model.flush(9.0));
    assert_eq!(saved(&path).repeat_interval, 0.0);

    let reloaded = Model::load(Some(path.clone()));
    assert!(!reloaded.values.repeat_enabled);
    let mut model = reloaded;
    model.values.repeat_enabled = true;
    model.values.repeat_rate = 20.0;
    model.edited(0.0);
    assert!(model.flush(9.0));
    assert!((saved(&path).repeat_interval - 0.05).abs() < 1e-12);
    let back = Model::load(Some(path));
    assert!((back.values.repeat_rate - 20.0).abs() < 1e-9);
}

#[test]
fn mappings_for_buttons_not_shown_are_kept() {
    let (_dir, path) = temp();
    let shown: Vec<&str> = RIGHT_JOYCON.iter().map(|r| r.id).collect();
    let hidden =
        *mappable_buttons().iter().find(|b| !shown.contains(b)).expect("a left Joy-Con button");
    let mut config = Config::default();
    let latch = Mapping::Modifier { modifier: Modifiers::SHIFT, latch: true };
    config.key_mappings.insert(hidden.to_owned(), latch);
    config.save(&path).unwrap();

    let mut model = Model::load(Some(path.clone()));
    model.set_mapping("A", None, 0.0);
    assert!(model.flush(9.0));
    assert_eq!(saved(&path).key_mappings.get(hidden), Some(&latch));
}

#[test]
fn a_bad_file_shows_its_problems_and_values_outside_the_sliders_survive() {
    let (_dir, path) = temp();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, br#"{"version": 1, "pointerSpeed": 6.0, "repeatDelay": "soon"}"#)
        .unwrap();
    let mut model = Model::load(Some(path.clone()));
    assert!(!model.load_warnings.is_empty(), "control: the bad delay is reported");
    assert_eq!(model.values.config.pointer_speed, 6.0, "beyond the slider, still valid");
    model.values.config.scroll_enabled = false;
    model.edited(0.0);
    assert!(model.flush(9.0));
    assert_eq!(saved(&path).pointer_speed, 6.0);
}

#[test]
fn restore_defaults_saves_the_defaults() {
    let (_dir, path) = temp();
    let mut model = Model::load(Some(path.clone()));
    model.set_mapping("A", None, 0.0);
    model.recording = Some("B");
    model.restore_defaults(1.0);
    assert_eq!(model.recording, None);
    assert!(model.flush(9.0));
    assert_eq!(saved(&path), Config::default());
}

#[test]
fn a_failed_save_is_shown_and_retried_on_the_next_edit() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-folder");
    std::fs::write(&blocker, b"").unwrap();
    let mut model = Model::load(Some(blocker.join("config.json")));
    model.edited(0.0);
    assert!(!model.flush(9.0));
    let error = model.save_error.clone().expect("the failure is shown");
    assert!(error.starts_with("Could not save settings"), "{error}");

    model.values.config.pointer_speed = 0.01;
    model.edited(1.0);
    assert!(!model.flush(9.0));
    assert!(
        model.save_error.as_deref().is_some_and(|e| e.contains("ointer")),
        "{:?}",
        model.save_error
    );
}

fn write(path: &Path, json: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, json).unwrap();
}

fn edit_outside(path: &Path, json: &str) {
    let later = std::fs::metadata(path).unwrap().modified().unwrap() + Duration::from_secs(5);
    std::fs::write(path, json).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(later))
        .unwrap();
}

#[test]
fn a_file_with_problems_is_kept_as_a_backup_before_the_first_save() {
    let (_dir, path) = temp();
    let broken = r#"{"version": 1, "keyMappings": {"A": 124}, "pointerSpeed": 3,"#;
    write(&path, broken);
    let mut model = Model::load(Some(path.clone()));
    assert!(model.backs_up() && !model.load_warnings.is_empty());
    model.values.config.scroll_enabled = false;
    model.edited(0.0);
    assert!(model.flush(1.0));
    let backup = path.with_file_name("config.json.bak");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), broken);
    assert!(!model.backs_up(), "only before the first save");

    let (_dir2, clean) = temp();
    write(&clean, r#"{"version": 1}"#);
    let mut model = Model::load(Some(clean.clone()));
    assert!(!model.backs_up());
    model.edited(0.0);
    assert!(model.flush(1.0));
    assert!(!clean.with_file_name("config.json.bak").exists(), "control");
}

#[test]
fn nothing_is_saved_when_the_backup_cannot_be_made() {
    let (_dir, path) = temp();
    write(&path, "{not json");
    std::fs::create_dir(path.with_file_name("config.json.bak")).unwrap();
    let mut model = Model::load(Some(path.clone()));
    model.edited(0.0);
    assert!(!model.flush(1.0));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json", "the old file is untouched");
    assert!(model.save_error.as_deref().is_some_and(|e| e.starts_with("Not saved")));
    assert!(model.pending(), "tried again later");
}

#[test]
fn edits_made_outside_the_window_are_picked_up_not_overwritten() {
    let (_dir, path) = temp();
    write(&path, r#"{"version": 1, "pointerSpeed": 2}"#);
    let mut model = Model::load(Some(path.clone()));
    model.check_file();
    assert!(model.notice.is_none(), "control: nothing changed yet");

    edit_outside(&path, r#"{"version": 1, "pointerSpeed": 3}"#);
    model.check_file();
    assert_eq!(model.values.config.pointer_speed, 3.0);
    assert!(model.notice.is_some());

    model.values.config.pointer_speed = 0.5;
    model.edited(1.0);
    edit_outside(&path, r#"{"version": 1, "pointerSpeed": 4}"#);
    assert!(!model.flush(2.0));
    assert_eq!(saved(&path).pointer_speed, 4.0);
    assert_eq!(model.values.config.pointer_speed, 4.0, "showing the newer file");
    assert!(!model.pending());
}

#[test]
fn the_windows_own_save_is_not_an_outside_edit() {
    let (_dir, path) = temp();
    write(&path, r#"{"version": 1}"#);
    let mut model = Model::load(Some(path.clone()));
    model.values.config.pointer_speed = 2.0;
    model.edited(0.0);
    assert!(model.flush(1.0));
    model.check_file();
    assert!(model.notice.is_none());
    model.values.config.pointer_speed = 2.5;
    model.edited(2.0);
    assert!(model.flush(3.0), "a second save is not blocked by the first");
}

#[test]
fn a_failed_save_stays_pending_and_is_tried_again() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-folder");
    std::fs::write(&blocker, b"").unwrap();
    let mut model = Model::load(Some(blocker.join("config.json")));
    model.edited(0.0);
    assert!(!model.flush(1.0));
    assert!(model.pending());
    let due = model.save_due_in(1.0).expect("pending");
    assert!((due - SAVE_DELAY).abs() < 1e-9, "retried after the save delay: {due}");
}

#[test]
fn a_hand_set_repeat_interval_is_kept_until_the_rate_moves() {
    // 2.0 is below the slider's range; 1e-310 is subnormal, so 1/x is infinite
    // and the old code saved it as repeat off.
    for interval in [2.0, 1e-310] {
        let (_dir, path) = temp();
        write(&path, &format!(r#"{{"version": 1, "repeatInterval": {interval:e}}}"#));
        let mut model = Model::load(Some(path.clone()));
        assert!(model.values.repeat_enabled, "{interval}");
        model.values.config.scroll_enabled = false;
        model.edited(0.0);
        assert!(model.flush(1.0));
        assert_eq!(saved(&path).repeat_interval, interval, "unrelated edit");

        model.values.repeat_rate = 10.0;
        model.edited(2.0);
        assert!(model.flush(3.0));
        assert!((saved(&path).repeat_interval - 0.1).abs() < 1e-12, "the rate moved");
    }
}

#[test]
fn restore_defaults_keeps_buttons_the_window_does_not_show() {
    let (_dir, path) = temp();
    let shown: Vec<&str> = RIGHT_JOYCON.iter().map(|r| r.id).collect();
    let hidden = *mappable_buttons().iter().find(|b| !shown.contains(b)).unwrap();
    let mut config = Config { pointer_speed: 3.0, ..Config::default() };
    config.key_mappings.insert(hidden.to_owned(), Mapping::from(0x31));
    config.key_mappings.insert("A".to_owned(), Mapping::from(0x31));
    config.save(&path).unwrap();

    let mut model = Model::load(Some(path.clone()));
    model.restore_defaults(0.0);
    assert!(model.flush(1.0));
    let restored = saved(&path);
    assert_eq!(restored.pointer_speed, Config::default().pointer_speed);
    assert_eq!(restored.key_mappings.get("A"), Config::default().key_mappings.get("A"));
    assert_eq!(restored.key_mappings.get(hidden), Some(&Mapping::from(0x31)));
}

#[test]
fn modifiers_can_be_added_to_a_recorded_key() {
    let (_dir, path) = temp();
    let mut model = Model::load(Some(path));
    model.set_mapping("A", Some(Mapping::from(0x08)), 0.0);
    model.toggle_modifier("A", Modifiers::COMMAND, 0.0);
    model.toggle_modifier("A", Modifiers::SHIFT, 0.0);
    let both = Modifiers::COMMAND.with(Modifiers::SHIFT);
    assert_eq!(
        model.mapping("A"),
        Some(Mapping::Shortcut(Shortcut { key: 0x08, modifiers: both }))
    );
    model.toggle_modifier("A", Modifiers::COMMAND, 0.0);
    assert_eq!(
        model.mapping("A"),
        Some(Mapping::Shortcut(Shortcut { key: 0x08, modifiers: Modifiers::SHIFT }))
    );
    let held = Mapping::Modifier { modifier: Modifiers::SHIFT, latch: false };
    model.set_mapping("B", Some(held), 0.0);
    model.toggle_modifier("B", Modifiers::COMMAND, 0.0);
    model.toggle_modifier("RS", Modifiers::COMMAND, 0.0);
    assert_eq!(model.mapping("B"), Some(held));
}

#[cfg(unix)]
#[test]
fn a_config_path_need_not_be_unicode() {
    use deskpuck_settings::config_path;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let odd = OsString::from_vec(b"/home/j\xe9r\xf4me/config.json".to_vec());
    let args = [OsString::from("--config"), odd.clone()];
    assert_eq!(config_path(&args), Some(Some(PathBuf::from(odd))));
    assert_eq!(config_path(&[]), Some(Config::default_path()));
    assert_eq!(config_path(&[OsString::from("--config")]), None);
    assert_eq!(config_path(&[OsString::from("--other"), OsString::from("x")]), None);
}
