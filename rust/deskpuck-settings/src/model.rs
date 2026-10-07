//! The settings window's state with no UI: the values each control shows,
//! how edits become a `Config`, and when it is written. Mirrors the Mac
//! app's SettingsModel.

use deskpuck_core::config::{Config, Mapping, Shortcut};
use deskpuck_core::mapping::Modifiers;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A right Joy-Con button shown in the window, in the order they sit on the
/// controller. Mappings for other buttons (a left Joy-Con's) are kept but not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonRow {
    pub id: &'static str,
    pub label: &'static str,
}

pub const RIGHT_JOYCON: [ButtonRow; 10] = [
    ButtonRow { id: "RS", label: "Stick click" },
    ButtonRow { id: "A", label: "A" },
    ButtonRow { id: "B", label: "B" },
    ButtonRow { id: "X", label: "X" },
    ButtonRow { id: "Y", label: "Y" },
    ButtonRow { id: "PLUS", label: "+" },
    ButtonRow { id: "HOME", label: "Home" },
    ButtonRow { id: "CHAT", label: "C" },
    ButtonRow { id: "SL", label: "SL" },
    ButtonRow { id: "SR", label: "SR" },
];

pub const SPEED_RANGE: (f64, f64, f64) = (0.25, 4.0, 0.25);
pub const DELAY_RANGE: (f64, f64, f64) = (0.15, 1.0, 0.05);
/// Keys per second while held; stored as repeatInterval = 1 / rate.
pub const RATE_RANGE: (f64, f64, f64) = (4.0, 30.0, 1.0);
/// Edits are written once they have settled, so a slider drag is one write.
pub const SAVE_DELAY: f64 = 0.4;

/// What the controls show.
#[derive(Clone, Debug, PartialEq)]
pub struct Values {
    pub config: Config,
    pub repeat_enabled: bool,
    pub repeat_rate: f64,
    /// The rate as loaded: until the slider moves, the file's own interval is
    /// kept, even one the slider cannot show.
    loaded_rate: f64,
}

fn default_rate() -> f64 {
    1.0 / Config::default().repeat_interval
}

impl Values {
    pub fn from_config(config: Config) -> Self {
        let interval = config.repeat_interval;
        let repeat_enabled = interval > 0.0;
        let repeat_rate = if !repeat_enabled {
            default_rate()
        } else if (1.0 / interval).is_finite() {
            1.0 / interval
        } else {
            // A tiny interval the slider cannot show; kept as it is until moved.
            RATE_RANGE.1
        };
        Self { config, repeat_enabled, repeat_rate, loaded_rate: repeat_rate }
    }

    /// The config to save: turning repeat off stores interval 0, and the rate
    /// is kept for when it comes back on.
    pub fn to_config(&self) -> Config {
        let mut config = self.config.clone();
        let loaded = self.config.repeat_interval;
        config.repeat_interval = if !self.repeat_enabled {
            0.0
        } else if loaded > 0.0 && self.repeat_rate == self.loaded_rate {
            loaded
        } else if self.repeat_rate > 0.0 {
            1.0 / self.repeat_rate
        } else {
            0.0
        };
        config
    }
}

/// The file's modification time and size, to notice edits from outside.
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

pub struct Model {
    pub values: Values,
    path: Option<PathBuf>,
    /// Problems found loading config.json; defaults were used for these.
    pub load_warnings: Vec<String>,
    pub save_error: Option<String>,
    /// Something to tell once, such as an outside edit being picked up.
    pub notice: Option<String>,
    /// The button whose shortcut is being recorded.
    pub recording: Option<&'static str>,
    /// When the last unsaved edit was made.
    edited_at: Option<f64>,
    /// The file as this window last read or wrote it.
    seen: Stamp,
    /// The file had problems, so it is copied to config.json.bak before the
    /// first write replaces what could not be read.
    needs_backup: bool,
}

impl Model {
    /// Loads `path`; a missing file shows the defaults and is written on the first edit.
    pub fn load(path: Option<PathBuf>) -> Self {
        let mut model = Self {
            values: Values::from_config(Config::default()),
            path,
            load_warnings: Vec::new(),
            save_error: None,
            notice: None,
            recording: None,
            edited_at: None,
            seen: None,
            needs_backup: false,
        };
        model.read();
        model
    }

    fn read(&mut self) {
        let Some(path) = &self.path else {
            self.load_warnings = vec!["No settings folder on this system.".into()];
            return;
        };
        let (config, warnings) = Config::load(path);
        self.seen = stamp(path);
        self.needs_backup = !warnings.is_empty() && self.seen.is_some();
        self.load_warnings = warnings;
        self.values = Values::from_config(config);
    }

    /// Whether the next save first copies the file to config.json.bak.
    pub fn backs_up(&self) -> bool {
        self.needs_backup
    }

    fn changed_outside(&self) -> bool {
        self.path.as_deref().is_some_and(|path| stamp(path) != self.seen)
    }

    /// Picks up an edit made outside the window (an editor, another window)
    /// while nothing here is waiting to be saved. Call about once a second.
    pub fn check_file(&mut self) {
        if self.edited_at.is_none() && self.changed_outside() {
            self.read();
            self.notice = Some("Showing config.json as it was changed outside this window.".into());
        }
    }

    pub fn mapping(&self, button: &str) -> Option<Mapping> {
        self.values.config.key_mappings.get(button).copied()
    }

    pub fn set_mapping(&mut self, button: &str, mapping: Option<Mapping>, now: f64) {
        match mapping {
            Some(mapping) => self.values.config.key_mappings.insert(button.to_owned(), mapping),
            None => self.values.config.key_mappings.remove(button),
        };
        self.edited(now);
    }

    /// Adds or removes one modifier on a button's shortcut, for those the
    /// recorder cannot see (Super/Windows).
    pub fn toggle_modifier(&mut self, button: &str, modifier: Modifiers, now: f64) {
        if let Some(Mapping::Shortcut(Shortcut { key, modifiers })) = self.mapping(button) {
            let modifiers = if modifiers.contains(modifier) {
                Modifiers::ALL
                    .iter()
                    .map(|(_, m, _)| *m)
                    .filter(|m| *m != modifier && modifiers.contains(*m))
                    .fold(Modifiers::NONE, Modifiers::with)
            } else {
                modifiers.with(modifier)
            };
            self.set_mapping(button, Some(Mapping::Shortcut(Shortcut { key, modifiers })), now);
        }
    }

    /// Call after changing `values` directly (sliders and toggles).
    pub fn edited(&mut self, now: f64) {
        self.edited_at = Some(now);
        self.notice = None;
    }

    /// The defaults for everything the window shows; mappings for buttons it
    /// does not show (a left Joy-Con's) are kept.
    pub fn restore_defaults(&mut self, now: f64) {
        let shown = |button: &str| RIGHT_JOYCON.iter().any(|row| row.id == button);
        let hidden: Vec<(String, Mapping)> = self
            .values
            .config
            .key_mappings
            .iter()
            .filter(|(button, _)| !shown(button))
            .map(|(button, mapping)| (button.clone(), *mapping))
            .collect();
        self.values = Values::from_config(Config::default());
        self.values.config.key_mappings.extend(hidden);
        self.recording = None;
        self.edited(now);
    }

    /// Seconds until the pending edit is due to be written, if one is pending.
    pub fn save_due_in(&self, now: f64) -> Option<f64> {
        self.edited_at.map(|at| (at + SAVE_DELAY - now).max(0.0))
    }

    /// Writes the settings once edits have settled. True if it wrote.
    pub fn save_if_due(&mut self, now: f64) -> bool {
        if self.save_due_in(now) != Some(0.0) {
            return false;
        }
        self.flush(now)
    }

    /// Whether an edit is waiting to be written.
    pub fn pending(&self) -> bool {
        self.edited_at.is_some()
    }

    /// Writes any pending edit now, e.g. as the window closes. True if it
    /// wrote. A failed write stays pending and is tried again.
    pub fn flush(&mut self, now: f64) -> bool {
        if self.edited_at.take().is_none() {
            return false;
        }
        let Some(path) = self.path.clone() else {
            self.save_error = Some("No settings folder on this system.".into());
            return false;
        };
        // A newer file from outside wins over an edit made here meanwhile.
        if self.changed_outside() {
            self.read();
            self.notice = Some(
                "config.json was changed outside this window; showing that instead of your last change."
                    .into(),
            );
            return false;
        }
        if self.needs_backup {
            if let Err(e) = Config::back_up(&path) {
                self.save_error =
                    Some(format!("Not saved: could not keep a copy of the old file: {e}"));
                self.edited_at = Some(now);
                return false;
            }
            self.needs_backup = false;
        }
        match self.values.to_config().save(&path) {
            Ok(()) => {
                self.seen = stamp(&path);
                self.save_error = None;
                true
            }
            Err(e) => {
                self.save_error = Some(format!("Could not save settings: {e}"));
                self.edited_at = Some(now);
                false
            }
        }
    }
}
