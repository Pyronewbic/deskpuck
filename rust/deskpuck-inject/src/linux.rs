//! uinput backend: one virtual device with relative pointer axes, mouse
//! buttons, and every key in the keymap. Works under Wayland and X11 alike,
//! since the kernel presents it like a real USB mouse and keyboard.

use crate::keymap::{KEYS, lookup};
use crate::{Accumulator, InjectError, InputEvent, Sink, WHEEL_UNITS_PER_NOTCH, wheel_units};
use deskpuck_core::mapping::MouseButton;
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, KeyCode, RelativeAxisCode};
use std::io;

pub struct LinuxSink {
    device: VirtualDevice,
    x: Accumulator,
    y: Accumulator,
    notches: Accumulator,
}

impl LinuxSink {
    /// Creates the virtual device. Desktops take a moment to pick up a new
    /// device, so events posted in the first few hundred milliseconds may be lost.
    pub fn new() -> Result<Self, InjectError> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in [KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE] {
            keys.insert(code);
        }
        for row in KEYS {
            keys.insert(KeyCode(row.linux));
        }
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        for axis in [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_WHEEL_HI_RES,
        ] {
            axes.insert(axis);
        }
        let device = VirtualDevice::builder()
            .and_then(|b| b.name("Deskpuck").with_keys(&keys)?.with_relative_axes(&axes)?.build())
            .map_err(open_error)?;
        Ok(Self {
            device,
            x: Accumulator::default(),
            y: Accumulator::default(),
            notches: Accumulator::default(),
        })
    }

    fn emit(&mut self, events: &[evdev::InputEvent]) -> Result<(), InjectError> {
        if events.is_empty() {
            return Ok(());
        }
        self.device
            .emit(events)
            .map_err(|e| InjectError::Failed(format!("Could not post to /dev/uinput: {e}")))
    }
}

fn open_error(e: io::Error) -> InjectError {
    match e.kind() {
        io::ErrorKind::PermissionDenied => InjectError::NotPermitted(
            "No write access to /dev/uinput. Add a udev rule such as \
             KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\", OPTIONS+=\"static_node=uinput\" \
             in /etc/udev/rules.d/60-deskpuck.rules, add yourself to the input group, then log in again."
                .into(),
        ),
        io::ErrorKind::NotFound => {
            InjectError::Failed("/dev/uinput does not exist. Load the module with: sudo modprobe uinput".into())
        }
        _ => InjectError::Failed(format!("Could not create the virtual input device: {e}")),
    }
}

fn rel(axis: RelativeAxisCode, value: i32) -> evdev::InputEvent {
    evdev::InputEvent::new(EventType::RELATIVE.0, axis.0, value)
}

fn key(code: u16, down: bool) -> evdev::InputEvent {
    evdev::InputEvent::new(EventType::KEY.0, code, i32::from(down))
}

impl Sink for LinuxSink {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        match *event {
            // The kernel tracks held buttons, so a move while one is down is already a drag.
            InputEvent::Move { dx, dy, .. } => {
                let (x, y) = (self.x.take(dx), self.y.take(dy));
                let mut events = Vec::with_capacity(2);
                if x != 0 {
                    events.push(rel(RelativeAxisCode::REL_X, x));
                }
                if y != 0 {
                    events.push(rel(RelativeAxisCode::REL_Y, y));
                }
                self.emit(&events)
            }
            InputEvent::Button { button, down } => {
                let code = match button {
                    MouseButton::Left => KeyCode::BTN_LEFT,
                    MouseButton::Right => KeyCode::BTN_RIGHT,
                    MouseButton::Middle => KeyCode::BTN_MIDDLE,
                };
                self.emit(&[key(code.0, down)])
            }
            InputEvent::Scroll { up } => {
                // Hi-res units for smooth scrolling, plus whole notches for older apps.
                let units = wheel_units(up);
                let notches =
                    self.notches.take(f64::from(units) / f64::from(WHEEL_UNITS_PER_NOTCH));
                let mut events = vec![rel(RelativeAxisCode::REL_WHEEL_HI_RES, units)];
                if notches != 0 {
                    events.push(rel(RelativeAxisCode::REL_WHEEL, notches));
                }
                self.emit(&events)
            }
            // The desktop repeats held keys itself and libinput drops device repeats,
            // so posting ours would at best do nothing.
            InputEvent::Key { repeat: true, .. } => Ok(()),
            // Keys with no Linux equivalent (Fn, JIS) are skipped; callers warn up front.
            InputEvent::Key { key_code, down, .. } => match lookup(key_code) {
                Some(row) => self.emit(&[key(row.linux, down)]),
                None => Ok(()),
            },
        }
    }
}
