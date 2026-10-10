use crate::model::Tab;

pub const MIN_HEIGHT: f32 = 240.0;
/// The narrowest tab's width, so the window can shrink to it.
pub const MIN_WIDTH: f32 = 440.0;

pub fn width(tab: Tab) -> f32 {
    match tab {
        Tab::Buttons => 640.0,
        Tab::General => MIN_WIDTH,
    }
}

/// The window's inner height: everything when the screen has room, else 0.85
/// of the monitor, which leaves room for a taskbar or panel it includes; only
/// the content scrolls. An unknown or nonsense monitor size caps nothing.
pub fn wanted_height(content: f32, header: f32, footer: f32, monitor: Option<f32>) -> f32 {
    let limit = monitor.filter(|m| m.is_finite() && *m > 0.0).map_or(f32::INFINITY, |m| m * 0.85);
    (content + header + footer).min(limit).max(MIN_HEIGHT).round()
}
