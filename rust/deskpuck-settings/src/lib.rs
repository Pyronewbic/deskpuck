//! The Deskpuck settings window for Linux and Windows: the Mac app's
//! Settings controls, saved to the same config.json.

pub mod keys;
pub mod model;
pub mod recorder;

use deskpuck_core::config::Config;
use std::ffi::OsString;
use std::path::PathBuf;

/// The settings file from the arguments: none for the default, or
/// `--config PATH`. Paths are raw OS strings, as they need not be Unicode.
/// `None` for anything else.
pub fn config_path(args: &[OsString]) -> Option<Option<PathBuf>> {
    match args {
        [] => Some(Config::default_path()),
        [flag, path] if flag == "--config" => Some(Some(PathBuf::from(path))),
        _ => None,
    }
}
