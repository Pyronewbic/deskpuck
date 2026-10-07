//! The Deskpuck settings window for Linux and Windows: the Mac app's
//! Settings controls, saved to the same config.json.

pub mod fonts;
pub mod keys;
pub mod model;
pub mod recorder;
pub mod text;
pub mod theme;
pub mod widgets;

use deskpuck_core::config::Config;
use eframe::egui::{IconData, ViewportBuilder};
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

/// The window as it opens, `width` wide. It sets no maximum size: Wayland
/// rejects an unbounded one with a protocol error and the window never
/// opens, and the window fits itself to its content anyway.
pub fn viewport(width: f32, icon: Option<IconData>) -> ViewportBuilder {
    let viewport = ViewportBuilder::default()
        .with_title("Deskpuck Settings")
        .with_app_id("deskpuck-settings")
        .with_inner_size([width, 720.0])
        .with_min_inner_size([width, 240.0]);
    match icon {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    }
}
