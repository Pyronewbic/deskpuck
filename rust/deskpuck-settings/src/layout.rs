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
