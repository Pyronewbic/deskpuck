//! Turns engine output into platform-neutral input events and posts them
//! through a `Sink`, one backend per OS.

use deskpuck_core::engine::EngineOutput;
use deskpuck_core::mapping::{KeyCode, MouseButton, MoveKind, mouse_move_kind};
use std::fmt;

pub mod keymap;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    /// Relative pointer motion in screen points.
    Move {
        dx: f64,
        dy: f64,
        kind: MoveKind,
    },
    Button {
        button: MouseButton,
        down: bool,
    },
    /// Pixels; positive scrolls up (wheel turned away from the user).
    Scroll {
        up: i32,
    },
    /// `key_code` is a macOS virtual key code; other backends translate it.
    Key {
        key_code: KeyCode,
        down: bool,
        repeat: bool,
    },
}

#[derive(Debug)]
pub enum InjectError {
    /// The OS refuses synthetic input until the user grants a permission.
    NotPermitted(String),
    Failed(String),
}

impl fmt::Display for InjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InjectError::NotPermitted(why) | InjectError::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for InjectError {}

pub trait Sink {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError>;
}

/// The input backend for this OS. Fails with `NotPermitted` when the OS needs a
/// permission first (Accessibility on macOS, /dev/uinput access on Linux).
pub fn platform_sink() -> Result<Box<dyn Sink>, InjectError> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(macos::MacSink::new()?));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(linux::LinuxSink::new()?));
    #[cfg(windows)]
    return Ok(Box::new(windows::WindowsSink::new()?));
    #[allow(unreachable_code)]
    Err(InjectError::Failed("Input injection is not available on this OS.".into()))
}

/// Carries the fraction of relative motion between events, for backends that
/// can only post whole units, so slow movement adds up instead of vanishing.
#[derive(Debug, Default)]
pub struct Accumulator {
    remainder: f64,
}

impl Accumulator {
    /// Whole units to post now. Non-finite input is dropped and never poisons
    /// later events; values past i32 saturate.
    pub fn take(&mut self, amount: f64) -> i32 {
        if !amount.is_finite() {
            return 0;
        }
        let total = self.remainder + amount;
        let whole = total.trunc();
        self.remainder = total - whole;
        whole as i32
    }
}

/// Hi-res wheel units per notch: Windows WHEEL_DELTA and Linux REL_WHEEL_HI_RES.
pub const WHEEL_UNITS_PER_NOTCH: i32 = 120;
/// Scroll pixels that make one wheel notch where a backend scrolls in notches.
pub const PIXELS_PER_NOTCH: i32 = 40;

/// Hi-res wheel units for a scroll of `pixels`.
pub fn wheel_units(pixels: i32) -> i32 {
    pixels.saturating_mul(WHEEL_UNITS_PER_NOTCH / PIXELS_PER_NOTCH)
}

/// Turns each engine output into events in posting order, remembering which
/// mouse buttons are down so presses and releases are sent once.
#[derive(Debug, Default)]
pub struct Poster {
    posted_buttons: u8,
}

const MOUSE_BUTTONS: [(u8, MouseButton); 3] =
    [(1, MouseButton::Left), (2, MouseButton::Right), (4, MouseButton::Middle)];

impl Poster {
    pub fn events(&mut self, out: &EngineOutput) -> Vec<InputEvent> {
        let mut events = Vec::new();
        if out.dx != 0.0 || out.dy != 0.0 {
            // The move is posted before this report's button changes, as the Mac app does.
            let kind = mouse_move_kind(self.posted_buttons);
            events.push(InputEvent::Move { dx: out.dx, dy: out.dy, kind });
        }
        for (bit, button) in MOUSE_BUTTONS {
            let down = out.mouse_buttons & bit != 0;
            if down != (self.posted_buttons & bit != 0) {
                events.push(InputEvent::Button { button, down });
            }
        }
        self.posted_buttons = out.mouse_buttons;
        if out.wheel != 0 {
            events.push(InputEvent::Scroll { up: -out.wheel });
        }
        events.extend(out.keys.iter().map(|k| InputEvent::Key {
            key_code: k.key_code,
            down: k.is_down,
            repeat: k.is_repeat,
        }));
        events
    }

    /// Posts every event for `out`; stops at the first sink error.
    pub fn post(&mut self, out: &EngineOutput, sink: &mut dyn Sink) -> Result<usize, InjectError> {
        let events = self.events(out);
        for event in &events {
            sink.post(event)?;
        }
        Ok(events.len())
    }
}

/// Collects events instead of posting them, for tests and dry runs.
#[derive(Debug, Default)]
pub struct RecordingSink {
    pub events: Vec<InputEvent>,
}

impl Sink for RecordingSink {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        self.events.push(*event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskpuck_core::mapping::KeyEvent;

    fn out(dx: f64, mouse_buttons: u8, wheel: i32) -> EngineOutput {
        EngineOutput { dx, dy: 0.0, mouse_buttons, wheel, keys: Vec::new() }
    }

    #[test]
    fn idle_output_posts_nothing() {
        assert!(Poster::default().events(&EngineOutput::default()).is_empty());
    }

    #[test]
    fn buttons_post_on_change_only() {
        let mut poster = Poster::default();
        assert_eq!(
            poster.events(&out(0.0, 1, 0)),
            [InputEvent::Button { button: MouseButton::Left, down: true }]
        );
        assert!(poster.events(&out(0.0, 1, 0)).is_empty());
        assert_eq!(
            poster.events(&out(0.0, 2, 0)),
            [
                InputEvent::Button { button: MouseButton::Left, down: false },
                InputEvent::Button { button: MouseButton::Right, down: true },
            ]
        );
        assert_eq!(
            poster.events(&out(0.0, 0, 0)),
            [InputEvent::Button { button: MouseButton::Right, down: false }]
        );
    }

    #[test]
    fn move_kind_uses_buttons_already_down() {
        let mut poster = Poster::default();
        // Press and move in one report: the move goes out before the press, so it is a plain move.
        let first = poster.events(&out(3.0, 1, 0));
        assert_eq!(first[0], InputEvent::Move { dx: 3.0, dy: 0.0, kind: MoveKind::Moved });
        assert_eq!(first[1], InputEvent::Button { button: MouseButton::Left, down: true });
        // While held, moves are drags; this is what the Dock and window drags need.
        assert_eq!(
            poster.events(&out(3.0, 1, 0)),
            [InputEvent::Move { dx: 3.0, dy: 0.0, kind: MoveKind::Dragged(MouseButton::Left) }]
        );
        // Released in the same report: still a drag, then the release.
        let last = poster.events(&out(3.0, 0, 0));
        assert_eq!(
            last[0],
            InputEvent::Move { dx: 3.0, dy: 0.0, kind: MoveKind::Dragged(MouseButton::Left) }
        );
        assert_eq!(last[1], InputEvent::Button { button: MouseButton::Left, down: false });
    }

    #[test]
    fn scroll_sign_and_key_order() {
        let mut poster = Poster::default();
        let keys = vec![
            KeyEvent { key_code: 124, is_down: true, is_repeat: false },
            KeyEvent { key_code: 126, is_down: true, is_repeat: true },
        ];
        // Engine wheel is negative when the stick is pushed up; that scrolls up.
        let events = poster.events(&EngineOutput { wheel: -10, keys, ..EngineOutput::default() });
        assert_eq!(
            events,
            [
                InputEvent::Scroll { up: 10 },
                InputEvent::Key { key_code: 124, down: true, repeat: false },
                InputEvent::Key { key_code: 126, down: true, repeat: true },
            ]
        );
    }

    #[test]
    fn accumulator_keeps_fractions() {
        let mut acc = Accumulator::default();
        assert_eq!([0.4, 0.4, 0.4].map(|v| acc.take(v)), [0, 0, 1]);
        let mut acc = Accumulator::default();
        assert_eq!([-0.6, -0.6].map(|v| acc.take(v)), [0, -1]);
        // Reversing direction cancels the carried fraction instead of adding to it.
        let mut acc = Accumulator::default();
        assert_eq!([0.6, -0.6, 0.6].map(|v| acc.take(v)), [0, 0, 0]);
        // Total over many small steps matches the exact sum.
        let mut acc = Accumulator::default();
        assert_eq!((0..1000).map(|_| acc.take(0.2)).sum::<i32>(), 200);
    }

    #[test]
    fn accumulator_survives_bad_input() {
        let mut acc = Accumulator::default();
        acc.take(0.5);
        assert_eq!(acc.take(f64::NAN), 0);
        assert_eq!(acc.take(f64::INFINITY), 0);
        // Positive control: the 0.5 carried before the bad input still counts.
        assert_eq!(acc.take(0.5), 1);
        assert_eq!(Accumulator::default().take(1e12), i32::MAX);
    }

    #[test]
    fn wheel_unit_scale() {
        assert_eq!(wheel_units(PIXELS_PER_NOTCH), WHEEL_UNITS_PER_NOTCH);
        assert_eq!(wheel_units(-5), -15);
        assert_eq!(wheel_units(i32::MAX), i32::MAX);
    }

    struct FailAfter(usize);
    impl Sink for FailAfter {
        fn post(&mut self, _: &InputEvent) -> Result<(), InjectError> {
            if self.0 == 0 {
                return Err(InjectError::Failed("refused".into()));
            }
            self.0 -= 1;
            Ok(())
        }
    }

    #[test]
    fn post_stops_at_first_sink_error() {
        let busy = out(1.0, 1, 5);
        assert!(Poster::default().post(&busy, &mut FailAfter(1)).is_err());
        // Positive control: the same output posts all three events to a sink that accepts them.
        assert_eq!(Poster::default().post(&busy, &mut FailAfter(3)).ok(), Some(3));
    }
}
