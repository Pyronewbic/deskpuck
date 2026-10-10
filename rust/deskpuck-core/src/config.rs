//! Compatible with the Mac app's config.json; unusable values fall back to defaults with warnings.

use crate::engine::{EngineSettings, mouse_buttons_for_joycon_buttons};
use crate::files;
use crate::mapping::{ButtonAction, ButtonKeyMapping, KeyCode, Modifiers};
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
const APPEARANCE: &str = "appearance";
const CHECK_UPDATES: &str = "checkUpdates";
const KNOWN_KEYS: [&str; 8] = [
    VERSION,
    KEY_MAPPINGS,
    POINTER_SPEED,
    REPEAT_DELAY,
    REPEAT_INTERVAL,
    SCROLL_ENABLED,
    APPEARANCE,
    CHECK_UPDATES,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Appearance> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// R, ZR, ZL, L and LS are mouse buttons and cannot be mapped.
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

/// Stored as a bare key code when there are no modifiers, so files without
/// shortcuts are unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

impl From<KeyCode> for Shortcut {
    fn from(key: KeyCode) -> Self {
        Self { key, modifiers: Modifiers::NONE }
    }
}

impl Shortcut {
    fn to_value(self) -> Value {
        if self.modifiers.is_empty() {
            json!(self.key)
        } else {
            json!({ "key": self.key, "modifiers": self.modifiers.names() })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mapping {
    Shortcut(Shortcut),
    Modifier { modifier: Modifiers, latch: bool },
}

impl From<KeyCode> for Mapping {
    fn from(key: KeyCode) -> Self {
        Mapping::Shortcut(Shortcut::from(key))
    }
}

impl From<Shortcut> for Mapping {
    fn from(shortcut: Shortcut) -> Self {
        Mapping::Shortcut(shortcut)
    }
}

impl Mapping {
    fn to_value(self) -> Value {
        match self {
            Mapping::Shortcut(shortcut) => shortcut.to_value(),
            Mapping::Modifier { modifier, latch } => {
                json!({ "modifier": modifier.names().first(), "latch": latch })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub key_mappings: BTreeMap<String, Mapping>,
    pub pointer_speed: f64,
    pub repeat_delay: f64,
    pub repeat_interval: f64,
    pub scroll_enabled: bool,
    pub appearance: Appearance,
    /// Ask github.com once a day whether a newer release exists.
    pub check_updates: bool,
}

impl Default for Config {
    fn default() -> Self {
        let defaults = EngineSettings::default();
        Self {
            key_mappings: defaults
                .key_mappings
                .iter()
                .filter_map(|m| {
                    let mapping = match m.action {
                        ButtonAction::Key { key_code, modifiers } => {
                            Mapping::Shortcut(Shortcut { key: key_code, modifiers })
                        }
                        ButtonAction::Modifier { modifiers, latch } => {
                            Mapping::Modifier { modifier: modifiers, latch }
                        }
                    };
                    button_name(m.button_mask).map(|n| (n.to_owned(), mapping))
                })
                .collect(),
            pointer_speed: defaults.pointer_speed,
            repeat_delay: defaults.repeat_delay,
            repeat_interval: defaults.repeat_interval,
            scroll_enabled: defaults.scroll_enabled,
            appearance: Appearance::default(),
            check_updates: true,
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
        Self::try_from_json(bytes).unwrap_or_else(Self::fallback)
    }

    fn fallback(problem: String) -> (Config, Vec<String>) {
        (Config::default(), vec![format!("{problem}; using defaults.")])
    }

    pub fn try_from_json(bytes: &[u8]) -> Result<(Config, Vec<String>), String> {
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|e| format!("Config is not valid JSON ({e})"))?;
        let mut warnings = Vec::new();
        let config = Self::try_from_value(&value, &mut warnings)?;
        Ok((config, warnings))
    }

    fn from_value(value: &Value, warnings: &mut Vec<String>) -> Config {
        Self::try_from_value(value, warnings).unwrap_or_else(|problem| {
            warnings.push(format!("{problem}; using defaults."));
            Config::default()
        })
    }

    fn try_from_value(value: &Value, warnings: &mut Vec<String>) -> Result<Config, String> {
        let mut config = Config::default();
        let Some(dict) = value.as_object() else {
            return Err("Config is not a JSON object".to_owned());
        };

        if dict.get(VERSION).and_then(read_number) != Some(CONFIG_VERSION) {
            return Err("Config version is missing or unsupported (expected 1)".to_owned());
        }

        let mut keys: Vec<&String> = dict.keys().collect();
        keys.sort();
        for key in keys.into_iter().filter(|k| !KNOWN_KEYS.contains(&k.as_str())) {
            warnings.push(format!("Unknown setting {key:?} ignored."));
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

        match dict.get(CHECK_UPDATES) {
            None => {}
            Some(Value::Bool(enabled)) => config.check_updates = *enabled,
            // Off, unlike other bad values: a mistyped "false" must not mean checking.
            Some(_) => {
                config.check_updates = false;
                warnings.push("checkUpdates must be true or false; using false.".to_owned());
            }
        }

        match dict.get(APPEARANCE) {
            None => {}
            Some(value) => match value.as_str().and_then(Appearance::from_name) {
                Some(appearance) => config.appearance = appearance,
                None => warnings.push(
                    "appearance must be \"system\", \"light\" or \"dark\"; using system."
                        .to_owned(),
                ),
            },
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> (Config, Vec<String>) {
        Self::try_load(path).unwrap_or_else(Self::fallback)
    }

    pub fn try_load(path: &Path) -> Result<(Config, Vec<String>), String> {
        match files::read_capped(path, MAX_CONFIG_BYTES) {
            Ok(Some(bytes)) => Self::try_from_json(&bytes),
            Ok(None) => Ok((Config::default(), Vec::new())),
            Err(problem) => Err(format!("{} {problem}", path.display())),
        }
    }

    pub fn back_up(path: &Path) -> Result<PathBuf, String> {
        let mut name = path.as_os_str().to_owned();
        name.push(".bak");
        let backup = PathBuf::from(name);
        let bytes = files::read_capped(path, MAX_CONFIG_BYTES)
            .map_err(|problem| format!("{} {problem}", path.display()))?
            .ok_or_else(|| format!("{} does not exist", path.display()))?;
        files::write_private(&backup, &bytes)
            .map_err(|e| format!("Could not write {}: {e}", backup.display()))?;
        Ok(backup)
    }

    pub fn to_value(&self) -> Value {
        json!({
            VERSION: CONFIG_VERSION as u8,
            KEY_MAPPINGS: self
                .key_mappings
                .iter()
                .map(|(button, mapping)| (button.clone(), mapping.to_value()))
                .collect::<Map<String, Value>>(),
            POINTER_SPEED: self.pointer_speed,
            REPEAT_DELAY: self.repeat_delay,
            REPEAT_INTERVAL: self.repeat_interval,
            SCROLL_ENABLED: self.scroll_enabled,
            APPEARANCE: self.appearance.name(),
            CHECK_UPDATES: self.check_updates,
        })
    }

    /// Uses the loading rules, so what can be saved is exactly what can be loaded.
    pub fn validation_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        Self::from_value(&self.to_value(), &mut problems);
        problems
    }

    pub fn to_json(&self) -> Option<Vec<u8>> {
        if !self.validation_problems().is_empty() {
            return None;
        }
        serde_json::to_vec_pretty(&self.to_value()).ok()
    }

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
                let mask = button_mask(name)?;
                Some(match *self.key_mappings.get(name)? {
                    Mapping::Shortcut(s) => ButtonKeyMapping::shortcut(mask, s.key, s.modifiers),
                    Mapping::Modifier { modifier, latch } => {
                        ButtonKeyMapping::modifier(mask, modifier, latch)
                    }
                })
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

fn read_key_code(value: &Value) -> Option<KeyCode> {
    read_number(value)
        .filter(|c| c.fract() == 0.0 && *c >= 0.0 && *c <= f64::from(KEY_CODE_MAX))
        .map(|c| c as KeyCode)
}

const MODIFIER_NAMES: &str = "control, option, shift or command";

fn parse_modifier_button(button: &str, fields: &Map<String, Value>) -> Result<Mapping, String> {
    if let Some(field) = fields.keys().find(|k| !["modifier", "latch"].contains(&k.as_str())) {
        return Err(format!("Modifier button {button} has an unknown field {field:?}"));
    }
    let name = &fields["modifier"];
    let Some(modifier) = name.as_str().and_then(Modifiers::from_name) else {
        return Err(format!("Unknown modifier {name} for {button} (use {MODIFIER_NAMES})"));
    };
    let latch = match fields.get("latch") {
        None => false,
        Some(Value::Bool(latch)) => *latch,
        Some(_) => return Err(format!("latch for {button} must be true or false")),
    };
    Ok(Mapping::Modifier { modifier, latch })
}

/// `Err` says why not, without the "mapping ignored" ending.
fn parse_mapping(button: &str, value: &Value) -> Result<Mapping, String> {
    match value {
        Value::Object(fields) if fields.contains_key("modifier") => {
            parse_modifier_button(button, fields)
        }
        _ => parse_shortcut(button, value).map(Mapping::Shortcut),
    }
}

fn parse_shortcut(button: &str, value: &Value) -> Result<Shortcut, String> {
    let bad_key =
        || format!("Key code for {button} must be a whole number from 0 to {KEY_CODE_MAX}");
    let Value::Object(fields) = value else {
        return read_key_code(value).map(Shortcut::from).ok_or_else(bad_key);
    };
    if let Some(field) = fields.keys().find(|k| !["key", "modifiers"].contains(&k.as_str())) {
        return Err(format!("Shortcut for {button} has an unknown field {field:?}"));
    }
    let key = fields.get("key").and_then(read_key_code).ok_or_else(bad_key)?;
    let mut modifiers = Modifiers::NONE;
    match fields.get("modifiers") {
        None => {}
        Some(Value::Array(names)) => {
            for name in names {
                let shown = name.as_str().map_or_else(|| name.to_string(), str::to_owned);
                let Some(modifier) = name.as_str().and_then(Modifiers::from_name) else {
                    return Err(format!(
                        "Unknown modifier {shown:?} for {button} (use {MODIFIER_NAMES})"
                    ));
                };
                if modifiers.contains(modifier) {
                    return Err(format!("Modifier {shown:?} is listed twice for {button}"));
                }
                modifiers = modifiers.with(modifier);
            }
        }
        Some(_) => return Err(format!("Modifiers for {button} must be a list")),
    }
    Ok(Shortcut { key, modifiers })
}

fn parse_key_mappings(
    mappings: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, Mapping> {
    let mappable = mappable_buttons();
    let mut buttons: Vec<&String> = mappings.keys().collect();
    buttons.sort();
    let mut parsed = BTreeMap::new();
    for button in buttons {
        if is_mouse_button(button) {
            warnings.push(format!("{button} is a mouse button and cannot be mapped to a key."));
        } else if !mappable.contains(&button.as_str()) {
            warnings.push(format!("Unknown button {button:?} in keyMappings ignored."));
        } else {
            match parse_mapping(button, &mappings[button]) {
                Ok(mapping) => {
                    parsed.insert(button.clone(), mapping);
                }
                Err(problem) => warnings.push(format!("{problem}; mapping ignored.")),
            }
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
