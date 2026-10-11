//! Kept out of config.json so a settings save can never race a newly paired id.
//! The id is a per-machine Bluetooth peripheral id; it identifies a device, not authenticates it.
//! A host address means the Joy-Con was vendor-paired to that computer and reconnects to it on a
//! button press.

use crate::config::Config;
use crate::files;
use serde_json::{Value, json};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub const MAX_PAIRING_BYTES: u64 = 4 * 1024;
pub const MAX_ID_LEN: usize = 128;
pub const MAX_NAME_CHARS: usize = 64;

const VERSION: f64 = 1.0;

/// A Bluetooth address as displayed, most significant byte first. Never all zeros, which is what
/// macOS reports while Bluetooth is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostAddress([u8; 6]);

impl HostAddress {
    /// Only `AA:BB:CC:DD:EE:FF`, in either case.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 17 {
            return None;
        }
        let mut address = [0u8; 6];
        for (i, byte) in address.iter_mut().enumerate() {
            let pair = &bytes[i * 3..i * 3 + 2];
            if !pair.iter().all(u8::is_ascii_hexdigit) || (i < 5 && bytes[i * 3 + 2] != b':') {
                return None;
            }
            *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        (address != [0; 6]).then_some(Self(address))
    }

    /// Least significant byte first, as the address travels over the air.
    pub fn wire(self) -> [u8; 6] {
        let mut bytes = self.0;
        bytes.reverse();
        bytes
    }

    pub fn from_wire(mut bytes: [u8; 6]) -> Option<Self> {
        bytes.reverse();
        (bytes != [0; 6]).then_some(Self(bytes))
    }
}

impl fmt::Display for HostAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{g:02X}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedDevice {
    pub id: String,
    pub name: Option<String>,
    pub host: Option<HostAddress>,
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
        Some(Self { id: id.to_owned(), name, host: None })
    }

    pub fn with_host(self, host: Option<HostAddress>) -> Self {
        Self { host, ..self }
    }

    pub fn default_path() -> Option<PathBuf> {
        Config::default_path().map(|p| p.with_file_name("pairing.json"))
    }

    /// Read back as untrusted: anything unexpected rejects the whole record.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let value: Value = serde_json::from_slice(bytes).map_err(|_| "is not valid JSON")?;
        let dict = value.as_object().ok_or("is not a JSON object")?;
        if dict.keys().any(|k| !["version", "id", "name", "host"].contains(&k.as_str())) {
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
        let host = match dict.get("host") {
            None | Some(Value::Null) => None,
            Some(Value::String(host)) => {
                Some(HostAddress::parse(host).ok_or("has an invalid host")?)
            }
            Some(_) => return Err("has an invalid host".into()),
        };
        Ok(Self { id: id.to_owned(), name, host })
    }

    /// Without a host the record is as before vendor pairing, so an older app still loads it.
    pub fn to_json(&self) -> Vec<u8> {
        let mut value = json!({ "version": VERSION as u8, "id": self.id, "name": self.name });
        if let (Some(host), Some(dict)) = (self.host, value.as_object_mut()) {
            dict.insert("host".into(), host.to_string().into());
        }
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
