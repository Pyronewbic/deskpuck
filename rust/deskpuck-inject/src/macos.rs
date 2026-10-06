//! CGEvent backend, mirroring the Mac app's DPController.

use crate::{InjectError, InputEvent, Sink};
use deskpuck_core::mapping::{MouseButton, MoveKind, Point, Rect, clamp_to_displays};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGError, CGEvent, CGEventField, CGEventFlags,
    CGEventTapLocation, CGEventType, CGGetDisplaysWithPoint, CGMainDisplayID, CGMouseButton,
    CGScrollEventUnit,
};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

/// Posts at the HID level, like a real device, so the Dock and hot corners react.
pub struct MacSink {
    /// Modifier keys this sink has pressed and not released. A posted
    /// modifier key-down does not mark later key events, so they are set here.
    held: CGEventFlags,
}

impl MacSink {
    /// Fails with `NotPermitted` without Accessibility access: CGEventPost
    /// would otherwise drop every event silently.
    pub fn new() -> Result<Self, InjectError> {
        // SAFETY: takes no arguments and only reads this process's TCC state.
        if unsafe { AXIsProcessTrusted() } == 0 {
            return Err(InjectError::NotPermitted(
                "Accessibility access is off for this terminal app. Turn it on in System Settings > \
                 Privacy & Security > Accessibility, then run again."
                    .into(),
            ));
        }
        Ok(Self::unchecked())
    }

    /// For an app that asks for Accessibility itself: events posted before
    /// the user grants it are dropped by macOS, and start working once granted.
    pub fn unchecked() -> Self {
        Self { held: CGEventFlags::empty() }
    }
}

/// The flag a modifier key sets, left or right.
fn modifier_flag(key_code: u16) -> Option<CGEventFlags> {
    match key_code {
        54 | 55 => Some(CGEventFlags::MaskCommand),
        56 | 60 => Some(CGEventFlags::MaskShift),
        58 | 61 => Some(CGEventFlags::MaskAlternate),
        59 | 62 => Some(CGEventFlags::MaskControl),
        _ => None,
    }
}

/// What the event already carries (e.g. a modifier held on the real
/// keyboard) plus what this sink holds; a modifier's own key-up drops its flag.
fn key_flags(
    existing: CGEventFlags,
    held: CGEventFlags,
    modifier: Option<CGEventFlags>,
) -> CGEventFlags {
    existing.difference(modifier.unwrap_or(CGEventFlags::empty())) | held
}

fn to_rect(r: CGRect) -> Rect {
    Rect { x: r.origin.x, y: r.origin.y, width: r.size.width, height: r.size.height }
}

fn cursor() -> Result<CGPoint, InjectError> {
    let event = CGEvent::new(None).ok_or_else(|| failed("read the cursor position"))?;
    Ok(CGEvent::location(Some(&event)))
}

fn display_at(p: Point) -> Option<Rect> {
    let mut display: CGDirectDisplayID = 0;
    let mut count: u32 = 0;
    // SAFETY: both out-pointers are valid locals with room for one display.
    let err =
        unsafe { CGGetDisplaysWithPoint(CGPoint::new(p.x, p.y), 1, &mut display, &mut count) };
    (err == CGError::Success && count > 0).then(|| to_rect(CGDisplayBounds(display)))
}

fn failed(what: &str) -> InjectError {
    InjectError::Failed(format!("Could not {what}."))
}

fn post(event: Option<CFRetained<CGEvent>>, what: &str) -> Result<(), InjectError> {
    let event = event.ok_or_else(|| failed(what))?;
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    Ok(())
}

fn cg_button(button: MouseButton) -> CGMouseButton {
    match button {
        MouseButton::Left => CGMouseButton::Left,
        MouseButton::Right => CGMouseButton::Right,
        MouseButton::Middle => CGMouseButton::Center,
    }
}

impl Sink for MacSink {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        match *event {
            InputEvent::Move { dx, dy, kind } => {
                let previous = cursor()?;
                let from = Point { x: previous.x, y: previous.y };
                let fallback = to_rect(CGDisplayBounds(CGMainDisplayID()));
                let target = clamp_to_displays(
                    Point { x: from.x + dx, y: from.y + dy },
                    from,
                    display_at,
                    fallback,
                );
                // A real move or drag event, not a warp, so the Dock, hot corners and drags see it.
                let (event_type, button) = match kind {
                    MoveKind::Moved => (CGEventType::MouseMoved, CGMouseButton::Left),
                    MoveKind::Dragged(MouseButton::Left) => {
                        (CGEventType::LeftMouseDragged, CGMouseButton::Left)
                    }
                    MoveKind::Dragged(MouseButton::Right) => {
                        (CGEventType::RightMouseDragged, CGMouseButton::Right)
                    }
                    MoveKind::Dragged(MouseButton::Middle) => {
                        (CGEventType::OtherMouseDragged, CGMouseButton::Center)
                    }
                };
                let cg_event = CGEvent::new_mouse_event(
                    None,
                    event_type,
                    CGPoint::new(target.x, target.y),
                    button,
                )
                .ok_or_else(|| failed("create a pointer event"))?;
                let delta = |to: f64, from: f64| (to - from).round() as i64;
                CGEvent::set_integer_value_field(
                    Some(&cg_event),
                    CGEventField::MouseEventDeltaX,
                    delta(target.x, from.x),
                );
                CGEvent::set_integer_value_field(
                    Some(&cg_event),
                    CGEventField::MouseEventDeltaY,
                    delta(target.y, from.y),
                );
                post(Some(cg_event), "create a pointer event")
            }
            InputEvent::Button { button, down } => {
                let event_type = match (button, down) {
                    (MouseButton::Left, true) => CGEventType::LeftMouseDown,
                    (MouseButton::Left, false) => CGEventType::LeftMouseUp,
                    (MouseButton::Right, true) => CGEventType::RightMouseDown,
                    (MouseButton::Right, false) => CGEventType::RightMouseUp,
                    (MouseButton::Middle, true) => CGEventType::OtherMouseDown,
                    (MouseButton::Middle, false) => CGEventType::OtherMouseUp,
                };
                post(
                    CGEvent::new_mouse_event(None, event_type, cursor()?, cg_button(button)),
                    "create a click event",
                )
            }
            InputEvent::Scroll { up } => post(
                CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Pixel, 1, up, 0, 0),
                "create a scroll event",
            ),
            InputEvent::Key { key_code, down, repeat } => {
                let cg_event = CGEvent::new_keyboard_event(None, key_code, down)
                    .ok_or_else(|| failed("create a key event"))?;
                let modifier = modifier_flag(key_code);
                if let Some(flag) = modifier {
                    self.held.set(flag, down);
                }
                if modifier.is_some() || !self.held.is_empty() {
                    let existing = CGEvent::flags(Some(&cg_event));
                    CGEvent::set_flags(Some(&cg_event), key_flags(existing, self.held, modifier));
                }
                if repeat {
                    CGEvent::set_integer_value_field(
                        Some(&cg_event),
                        CGEventField::KeyboardEventAutorepeat,
                        1,
                    );
                }
                post(Some(cg_event), "create a key event")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: CGEventFlags = CGEventFlags::MaskCommand;
    const CTRL: CGEventFlags = CGEventFlags::MaskControl;
    const SHIFT: CGEventFlags = CGEventFlags::MaskShift;

    #[test]
    fn modifier_keys_map_to_their_flags() {
        for (code, flag) in [(55, CMD), (54, CMD), (56, SHIFT), (60, SHIFT), (59, CTRL), (62, CTRL)]
        {
            assert_eq!(modifier_flag(code), Some(flag), "{code}");
        }
        assert_eq!(modifier_flag(58), Some(CGEventFlags::MaskAlternate));
        assert_eq!(modifier_flag(8), None, "C is not a modifier");
    }

    #[test]
    fn key_events_carry_held_modifiers() {
        let none = CGEventFlags::empty();
        // Ctrl+C: Control down marks itself, C carries it, Control up clears it.
        assert_eq!(key_flags(none, CTRL, Some(CTRL)), CTRL);
        assert_eq!(key_flags(none, CTRL, None), CTRL);
        assert_eq!(key_flags(CTRL, none, Some(CTRL)), none);
        // A modifier held on the real keyboard is kept, not replaced.
        assert_eq!(key_flags(SHIFT, CTRL, None), SHIFT | CTRL);
        assert_eq!(key_flags(SHIFT | CTRL, none, Some(CTRL)), SHIFT);
    }
}
