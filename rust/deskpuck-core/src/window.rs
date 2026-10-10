//! The settings window's height and placement, shared by the Mac window
//! and the Linux/Windows one. Sizes are in points.

pub const MIN_HEIGHT: f32 = 240.0;

/// The window's inner height: everything when the screen has room, else 0.85
/// of it, which leaves room for a taskbar or panel the size may include; only
/// the content scrolls. An unknown or nonsense screen size caps nothing.
pub fn wanted_height(content: f32, header: f32, footer: f32, screen: Option<f32>) -> f32 {
    let limit = screen.filter(|s| known(*s)).map_or(f32::INFINITY, |s| s * 0.85);
    (content + header + footer).min(limit).max(MIN_HEIGHT).round()
}

/// Where to move the window's top, measured down from the screen's, once it
/// is `height` tall, frame included: centred on the first fit, later lifted
/// only if a taller tab would push its bottom past the screen's. An unknown
/// screen size moves nothing.
pub fn moved_top(first: bool, top: f32, height: f32, screen: f32) -> Option<f32> {
    if !known(screen) {
        None
    } else if first {
        Some(((screen - height) / 2.0).max(0.0))
    } else if top + height > screen {
        Some((screen - height).max(0.0))
    } else {
        None
    }
}

fn known(screen: f32) -> bool {
    screen.is_finite() && screen > 0.0
}
