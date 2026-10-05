//! SendInput backend. Moves are relative, so the user's pointer speed and
//! "Enhance pointer precision" settings apply on top of Deskpuck's own.

use crate::keymap::lookup;
use crate::{Accumulator, InjectError, InputEvent, Sink, wheel_units};
use deskpuck_core::mapping::MouseButton;
use std::io;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MapVirtualKeyW, SendInput,
};

#[derive(Default)]
pub struct WindowsSink {
    x: Accumulator,
    y: Accumulator,
}

impl WindowsSink {
    pub fn new() -> Result<Self, InjectError> {
        Ok(Self::default())
    }
}

fn mouse(dx: i32, dy: i32, data: i32, flags: u32) -> INPUT {
    // mouseData is a DWORD that Windows reads as signed for wheel deltas.
    let mi = MOUSEINPUT { dx, dy, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 };
    INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi } }
}

fn send(input: INPUT) -> Result<(), InjectError> {
    // SAFETY: one fully initialised INPUT, with its exact size as cbSize.
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 1 {
        Ok(())
    } else {
        Err(InjectError::Failed(format!(
            "SendInput refused the event: {}",
            io::Error::last_os_error()
        )))
    }
}

impl Sink for WindowsSink {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        match *event {
            // Windows tracks held buttons, so a move while one is down is already a drag.
            InputEvent::Move { dx, dy, .. } => {
                let (x, y) = (self.x.take(dx), self.y.take(dy));
                if x == 0 && y == 0 {
                    return Ok(());
                }
                send(mouse(x, y, 0, MOUSEEVENTF_MOVE))
            }
            InputEvent::Button { button, down } => {
                let flags = match (button, down) {
                    (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                    (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                    (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                    (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                    (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                };
                send(mouse(0, 0, 0, flags))
            }
            InputEvent::Scroll { up } => send(mouse(0, 0, wheel_units(up), MOUSEEVENTF_WHEEL)),
            // Synthetic keys do not auto-repeat on Windows, so a repeat is another key-down.
            InputEvent::Key { key_code, down, .. } => {
                let Some(row) = lookup(key_code) else { return Ok(()) };
                let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
                if row.extended {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                // SAFETY: MapVirtualKeyW only reads its integer arguments.
                let scan = unsafe { MapVirtualKeyW(u32::from(row.vk), MAPVK_VK_TO_VSC) } as u16;
                let ki = KEYBDINPUT {
                    wVk: row.vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                };
                send(INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki } })
            }
        }
    }
}
