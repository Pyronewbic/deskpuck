//! Kept out of config.json so a settings save can never race a newly paired id.
//! The id is a per-machine Bluetooth peripheral id; it identifies a device, not authenticates it.

use crate::config::Config;
use crate::files;
use serde_json::{Value, json};
use std::io;
use std::path::{Path, PathBuf};

pub const MAX_PAIRING_BYTES: u64 = 4 * 1024;
pub const MAX_ID_LEN: usize = 128;
pub const MAX_NAME_CHARS: usize = 64;

const VERSION: f64 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedDevice {
    pub id: String,
    pub name: Option<String>,
}

fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_graphic())
}

/// Control characters and invisible ones that reorder or hide text (bidi, zero-width, BOM).
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{FEFF}')
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}')
        || matches!(c, '\u{2060}'..='\u{206F}' | '\u{FFF9}'..='\u{FFFB}')
}

/// The name comes from whatever device advertised it, so it is sanitised.
pub fn clean_name(name: &str) -> Option<String> {
    let clean: String = name.chars().filter(|c| !is_hidden(*c)).take(MAX_NAME_CHARS).collect();
    (!clean.is_empty()).then_some(clean)
}

fn valid_name(name: &str) -> bool {
    clean_name(name).as_deref() == Some(name)
}

impl PairedDevice {
    /// The name is cleaned to what `from_json` accepts, so a saved record always loads.
    pub fn new(id: &str, name: Option<&str>) -> Option<Self> {
        if !valid_id(id) {
            return None;
        }
        let name = name.and_then(clean_name);
        Some(Self { id: id.to_owned(), name })
    }

    pub fn default_path() -> Option<PathBuf> {
        Config::default_path().map(|p| p.with_file_name("pairing.json"))
    }

    /// Read back as untrusted: anything unexpected rejects the whole record.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let value: Value = serde_json::from_slice(bytes).map_err(|_| "is not valid JSON")?;
        let dict = value.as_object().ok_or("is not a JSON object")?;
        if dict.keys().any(|k| !["version", "id", "name"].contains(&k.as_str())) {
            return Err("has an unknown field".into());
        }
        if dict.get("version").and_then(Value::as_f64) != Some(VERSION) {
            return Err("has a missing or unsupported version".into());
        }
        let id = dict.get("id").and_then(Value::as_str).filter(|id| valid_id(id));
        let id = id.ok_or("has a missing or invalid id")?;
        let name = match dict.get("name") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) if valid_name(name) => Some(name.clone()),
            Some(_) => return Err("has an invalid name".into()),
        };
        Ok(Self { id: id.to_owned(), name })
    }

    pub fn to_json(&self) -> Vec<u8> {
        let value = json!({ "version": VERSION as u8, "id": self.id, "name": self.name });
        serde_json::to_vec_pretty(&value).unwrap_or_default()
    }

    /// `Err` is a file that exists but is unusable; callers treat it as not paired.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let fail = |problem: String| format!("{} {problem}", path.display());
        match files::read_capped(path, MAX_PAIRING_BYTES).map_err(fail)? {
            None => Ok(None),
            Some(bytes) => Self::from_json(&bytes).map(Some).map_err(fail),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        files::write_private(path, &self.to_json())
    }
}
