//! User settings stored as JSON, compatible with the Mac app's config.json.
//! Loading never fails: anything unusable falls back to its default and is
//! described in the returned warnings.

use crate::engine::{EngineSettings, mouse_buttons_for_joycon_buttons};
use crate::files;
use crate::mapping::{ButtonKeyMapping, KeyCode};
use crate::packet::{button_mask, button_name, button_names};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub const POINTER_SPEED_MIN: f64 = 0.1;
pub const POINTER_SPEED_MAX: f64 = 10.0;
pub const REPEAT_DELAY_MAX: f64 = 5.0;
/// 0 turns repeat off.
pub const REPEAT_INTERVAL_MAX: f64 = 2.0;
pub const KEY_CODE_MAX: KeyCode = 127;
pub const MAX_CONFIG_BYTES: u64 = 64 * 1024;

const CONFIG_VERSION: f64 = 1.0;
const VERSION: &str = "version";
const KEY_MAPPINGS: &str = "keyMappings";
const POINTER_SPEED: &str = "pointerSpeed";
const REPEAT_DELAY: &str = "repeatDelay";
const REPEAT_INTERVAL: &str = "repeatInterval";
const SCROLL_ENABLED: &str = "scrollEnabled";
const KNOWN_KEYS: [&str; 6] =
    [VERSION, KEY_MAPPINGS, POINTER_SPEED, REPEAT_DELAY, REPEAT_INTERVAL, SCROLL_ENABLED];

/// Joy-Con button names that can be mapped to keys, in a fixed order. R, ZR,
/// ZL, L and LS are mouse buttons and cannot be mapped.
pub fn mappable_buttons() -> Vec<&'static str> {
    button_names(u32::MAX)
        .into_iter()
        .filter(|name| button_mask(name).is_some_and(|m| mouse_buttons_for_joycon_buttons(m) == 0))
        .collect()
}

fn is_mouse_button(name: &str) -> bool {
    button_mask(name).is_some_and(|m| mouse_buttons_for_joycon_buttons(m) != 0)
}

/// Booleans are not numbers here, unlike in some JSON libraries.
fn read_number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Button name -> macOS virtual key code (0-127).
    pub key_mappings: BTreeMap<String, KeyCode>,
    pub pointer_speed: f64,
    pub repeat_delay: f64,
    pub repeat_interval: f64,
    pub scroll_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        let defaults = EngineSettings::default();
        Self {
            key_mappings: defaults
                .key_mappings
                .iter()
                .filter_map(|m| button_name(m.button_mask).map(|n| (n.to_owned(), m.key_code)))
                .collect(),
            pointer_speed: defaults.pointer_speed,
            repeat_delay: defaults.repeat_delay,
            repeat_interval: defaults.repeat_interval,
            scroll_enabled: defaults.scroll_enabled,
        }
    }
}

#[derive(Debug)]
pub enum SaveError {
    Invalid(Vec<String>),
    Io(io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Invalid(problems) => f.write_str(&problems.join(" ")),
            SaveError::Io(e) => write!(f, "Could not save the config: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        SaveError::Io(e)
    }
}

impl Config {
    /// Per-user config file: Application Support on macOS, XDG config on Linux,
    /// roaming AppData on Windows.
    pub fn default_path() -> Option<PathBuf> {
        let dir = if cfg!(target_os = "linux") { "deskpuck" } else { "Deskpuck" };
        dirs::config_dir().map(|base| base.join(dir).join("config.json"))
    }

    pub fn from_json(bytes: &[u8]) -> (Config, Vec<String>) {
        let mut warnings = Vec::new();
        let config = match serde_json::from_slice::<Value>(bytes) {
            Ok(value) => Self::from_value(&value, &mut warnings),
            Err(e) => {
                warnings.push(format!("Config is not valid JSON ({e}); using defaults."));
                Config::default()
            }
        };
        (config, warnings)
    }

    fn from_value(value: &Value, warnings: &mut Vec<String>) -> Config {
        let mut config = Config::default();
        let Some(dict) = value.as_object() else {
            warnings.push("Config is not a JSON object; using defaults.".to_owned());
            return config;
        };

        if dict.get(VERSION).and_then(read_number) != Some(CONFIG_VERSION) {
            warnings.push(
                "Config version is missing or unsupported (expected 1); using defaults.".to_owned(),
            );
            return config;
        }

        let mut keys: Vec<&String> = dict.keys().collect();
        keys.sort();
        for key in keys.into_iter().filter(|k| !KNOWN_KEYS.contains(&k.as_str())) {
            warnings.push(format!("Unknown setting \"{key}\" ignored."));
        }

        match dict.get(KEY_MAPPINGS) {
            None => {}
            Some(Value::Object(mappings)) => {
                config.key_mappings = parse_key_mappings(mappings, warnings);
            }
            Some(_) => warnings
                .push("keyMappings must be an object; using the default mappings.".to_owned()),
        }

        read_setting(
            dict,
            POINTER_SPEED,
            POINTER_SPEED_MIN,
            POINTER_SPEED_MAX,
            &mut config.pointer_speed,
            warnings,
        );
        read_setting(dict, REPEAT_DELAY, 0.0, REPEAT_DELAY_MAX, &mut config.repeat_delay, warnings);
        read_setting(
            dict,
            REPEAT_INTERVAL,
            0.0,
            REPEAT_INTERVAL_MAX,
            &mut config.repeat_interval,
            warnings,
        );

        match dict.get(SCROLL_ENABLED) {
            None => {}
            Some(Value::Bool(enabled)) => config.scroll_enabled = *enabled,
            Some(_) => warnings.push("scrollEnabled must be true or false; using true.".to_owned()),
        }
        config
    }

    /// A missing file gives the defaults without a warning. Reads at most
    /// `MAX_CONFIG_BYTES` and refuses anything but a regular file.
    pub fn load(path: &Path) -> (Config, Vec<String>) {
        match files::read_capped(path, MAX_CONFIG_BYTES) {
            Ok(Some(bytes)) => Self::from_json(&bytes),
            Ok(None) => (Config::default(), Vec::new()),
            Err(problem) => {
                (Config::default(), vec![format!("{} {problem}; using defaults.", path.display())])
            }
        }
    }

    /// The config as a JSON value, valid or not; `validation_problems` says which.
    pub fn to_value(&self) -> Value {
        json!({
            VERSION: CONFIG_VERSION as u8,
            KEY_MAPPINGS: self.key_mappings,
            POINTER_SPEED: self.pointer_speed,
            REPEAT_DELAY: self.repeat_delay,
            REPEAT_INTERVAL: self.repeat_interval,
            SCROLL_ENABLED: self.scroll_enabled,
        })
    }

    /// Empty when every value is in range. Uses the loading rules, so what can
    /// be saved is exactly what can be loaded.
    pub fn validation_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        Self::from_value(&self.to_value(), &mut problems);
        problems
    }

    /// Pretty JSON with sorted keys; `None` if `validation_problems` is not empty.
    pub fn to_json(&self) -> Option<Vec<u8>> {
        if !self.validation_problems().is_empty() {
            return None;
        }
        serde_json::to_vec_pretty(&self.to_value()).ok()
    }

    /// Refuses invalid settings. Writes a temp file beside `path` and renames it
    /// over the target, so a symlink at `path` is replaced, never written through.
    pub fn save(&self, path: &Path) -> Result<(), SaveError> {
        let problems = self.validation_problems();
        if !problems.is_empty() {
            return Err(SaveError::Invalid(problems));
        }
        let data = serde_json::to_vec_pretty(&self.to_value()).map_err(io::Error::other)?;
        files::write_private(path, &data)?;
        Ok(())
    }

    /// Engine settings in the fixed mappable-button order, so simultaneous key
    /// events come out in a stable order.
    pub fn engine_settings(&self) -> EngineSettings {
        let key_mappings = mappable_buttons()
            .into_iter()
            .filter_map(|name| {
                let key_code = *self.key_mappings.get(name)?;
                Some(ButtonKeyMapping { button_mask: button_mask(name)?, key_code })
            })
            .collect();
        EngineSettings {
            key_mappings,
            pointer_speed: self.pointer_speed,
            repeat_delay: self.repeat_delay,
            repeat_interval: self.repeat_interval,
            scroll_enabled: self.scroll_enabled,
        }
    }
}

fn parse_key_mappings(
    mappings: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, KeyCode> {
    let mappable = mappable_buttons();
    let mut buttons: Vec<&String> = mappings.keys().collect();
    buttons.sort();
    let mut parsed = BTreeMap::new();
    for button in buttons {
        let code = read_number(&mappings[button])
            .filter(|c| c.fract() == 0.0 && *c >= 0.0 && *c <= f64::from(KEY_CODE_MAX));
        if is_mouse_button(button) {
            warnings.push(format!("{button} is a mouse button and cannot be mapped to a key."));
        } else if !mappable.contains(&button.as_str()) {
            warnings.push(format!("Unknown button \"{button}\" in keyMappings ignored."));
        } else if let Some(code) = code {
            parsed.insert(button.clone(), code as KeyCode);
        } else {
            warnings.push(format!(
                "Key code for {button} must be a whole number from 0 to {KEY_CODE_MAX}; mapping ignored."
            ));
        }
    }
    parsed
}

fn read_setting(
    dict: &Map<String, Value>,
    key: &str,
    min: f64,
    max: f64,
    value: &mut f64,
    warnings: &mut Vec<String>,
) {
    let Some(raw) = dict.get(key) else { return };
    match read_number(raw).filter(|n| (min..=max).contains(n)) {
        Some(n) => *value = n,
        None => {
            warnings.push(format!("{key} must be a number from {min} to {max}; using {value}."))
        }
    }
}
