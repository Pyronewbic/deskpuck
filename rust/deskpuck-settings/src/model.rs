//! The settings window's state with no UI: the values each control shows,
//! how edits become a `Config`, and when it is written. Mirrors the Mac
//! app's SettingsModel.

use deskpuck_core::config::{Config, Mapping};
use std::path::PathBuf;

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
}

fn default_rate() -> f64 {
    1.0 / Config::default().repeat_interval
}

impl Values {
    pub fn from_config(config: Config) -> Self {
        let interval = config.repeat_interval;
        let repeat_enabled = interval > 0.0;
        let repeat_rate = if repeat_enabled { 1.0 / interval } else { default_rate() };
        Self { config, repeat_enabled, repeat_rate }
    }

    /// The config to save: turning repeat off stores interval 0, and the rate
    /// is kept for when it comes back on.
    pub fn to_config(&self) -> Config {
        let mut config = self.config.clone();
        config.repeat_interval = if self.repeat_enabled && self.repeat_rate > 0.0 {
            1.0 / self.repeat_rate
        } else {
            0.0
        };
        config
    }
}

pub struct Model {
    pub values: Values,
    path: Option<PathBuf>,
    /// Problems found loading config.json; defaults were used for these.
    pub load_warnings: Vec<String>,
    pub save_error: Option<String>,
    /// The button whose shortcut is being recorded.
    pub recording: Option<&'static str>,
    /// When the last unsaved edit was made.
    edited_at: Option<f64>,
}

impl Model {
    /// Loads `path`; a missing file shows the defaults and is written on the first edit.
    pub fn load(path: Option<PathBuf>) -> Self {
        let (config, load_warnings) = match &path {
            Some(path) if path.exists() => Config::load(path),
            Some(_) => (Config::default(), Vec::new()),
            None => (Config::default(), vec!["No settings folder on this system.".into()]),
        };
        Self {
            values: Values::from_config(config),
            path,
            load_warnings,
            save_error: None,
            recording: None,
            edited_at: None,
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

    /// Call after changing `values` directly (sliders and toggles).
    pub fn edited(&mut self, now: f64) {
        self.edited_at = Some(now);
    }

    pub fn restore_defaults(&mut self, now: f64) {
        self.values = Values::from_config(Config::default());
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
        self.flush()
    }

    /// Writes any pending edit now, e.g. as the window closes. True if it wrote.
    pub fn flush(&mut self) -> bool {
        if self.edited_at.take().is_none() {
            return false;
        }
        let Some(path) = &self.path else {
            self.save_error = Some("No settings folder on this system.".into());
            return false;
        };
        match self.values.to_config().save(path) {
            Ok(()) => {
                self.save_error = None;
                true
            }
            Err(e) => {
                self.save_error = Some(format!("Could not save settings: {e}"));
                false
            }
        }
    }
}
