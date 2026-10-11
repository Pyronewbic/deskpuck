use crate::model::Tab;

pub use deskpuck_core::window::{MIN_HEIGHT, moved_top, wanted_height};

/// The narrowest tab's width, so the window can shrink to it.
pub const MIN_WIDTH: f32 = 440.0;

pub fn width(tab: Tab) -> f32 {
    match tab {
        Tab::Buttons => 640.0,
        Tab::General => MIN_WIDTH,
    }
}

/// A size the window asked for, and whether it knew the screen then.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ask {
    pub size: [f32; 2],
    pub screen_known: bool,
}

/// Wayland reports no screen until the window is mapped, and the compositor's first
/// configure can override a resize sent before that, so it is sent again then.
pub fn ask_again(current: Option<[f32; 2]>, asked: Option<Ask>, wanted: Ask) -> bool {
    current != Some(wanted.size) && asked != Some(wanted)
}
