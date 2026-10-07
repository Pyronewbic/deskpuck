use deskpuck_core::config::{Config, Mapping, mappable_buttons};
use deskpuck_core::mapping::Modifiers;
use deskpuck_settings::model::{Model, RIGHT_JOYCON, SAVE_DELAY, Values};
use std::path::{Path, PathBuf};

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
    assert!(!model.flush(), "nothing to save before an edit");
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
    assert!(model.flush());
    assert_eq!(saved(&path).repeat_interval, 0.0);

    let reloaded = Model::load(Some(path.clone()));
    assert!(!reloaded.values.repeat_enabled);
    let mut model = reloaded;
    model.values.repeat_enabled = true;
    model.values.repeat_rate = 20.0;
    model.edited(0.0);
    assert!(model.flush());
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
    assert!(model.flush());
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
    assert!(model.flush());
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
    assert!(model.flush());
    assert_eq!(saved(&path), Config::default());
}

#[test]
fn a_failed_save_is_shown_and_retried_on_the_next_edit() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-folder");
    std::fs::write(&blocker, b"").unwrap();
    let mut model = Model::load(Some(blocker.join("config.json")));
    model.edited(0.0);
    assert!(!model.flush());
    let error = model.save_error.clone().expect("the failure is shown");
    assert!(error.starts_with("Could not save settings"), "{error}");

    model.values.config.pointer_speed = 0.01;
    model.edited(1.0);
    assert!(!model.flush());
    assert!(
        model.save_error.as_deref().is_some_and(|e| e.contains("ointer")),
        "{:?}",
        model.save_error
    );
}
