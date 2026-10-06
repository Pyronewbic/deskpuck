//! Bluetooth LE for the Joy-Con 2: the connection state machine, the session
//! that turns its reports into input, and the monitor readout.

pub mod controller;
pub mod receiver;

use deskpuck_core::engine::{EngineSettings, InputEngine};
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::packet::{Report, button_names};
use deskpuck_inject::{InjectError, InputEvent, Poster, Sink};
use receiver::Status;
use std::fmt::Write;

/// Mirrors the Mac app's DPController: connection changes and reports in,
/// posted input out. Every disconnect, pause and the shutdown release held input.
pub struct Session<S: Sink> {
    engine: InputEngine,
    poster: Poster,
    sink: S,
    connected: bool,
    paused: bool,
}

impl<S: Sink> Session<S> {
    pub fn new(settings: EngineSettings, sink: S) -> Self {
        Self {
            engine: InputEngine::new(settings),
            poster: Poster::default(),
            sink,
            connected: false,
            paused: false,
        }
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn status(&mut self, status: Status) -> Result<(), InjectError> {
        match status {
            Status::Connected => {
                // Re-baseline so the first report after (re)connecting does not jump.
                self.engine.connection_started();
                self.connected = true;
                Ok(())
            }
            _ if self.connected => {
                self.connected = false;
                self.release()
            }
            _ => Ok(()),
        }
    }

    pub fn report(&mut self, report: &Report, now: f64) -> Result<(), InjectError> {
        if !self.connected || self.paused {
            return Ok(());
        }
        let out = self.engine.process(report, now);
        self.poster.post(&out, &mut self.sink).map(|_| ())
    }

    /// Pausing releases held input and ignores reports; resuming re-baselines
    /// so motion while paused does not jump the pointer.
    pub fn set_paused(&mut self, paused: bool) -> Result<(), InjectError> {
        if paused == self.paused {
            return Ok(());
        }
        self.paused = paused;
        if paused {
            self.release()
        } else {
            self.engine.connection_started();
            Ok(())
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Modifiers latched on by a modifier button; pausing or a disconnect clears them.
    pub fn latched(&self) -> Modifiers {
        self.engine.latched()
    }

    /// Takes effect immediately, releasing keys held under the old mapping.
    /// Only keys are posted: a held mouse button stays held, as in the Mac app.
    pub fn apply_settings(&mut self, settings: EngineSettings) -> Result<(), InjectError> {
        for key in self.engine.apply_settings(settings) {
            let event = InputEvent::Key {
                key_code: key.key_code,
                down: key.is_down,
                repeat: key.is_repeat,
            };
            self.sink.post(&event)?;
        }
        Ok(())
    }

    /// Releases every held key and button, e.g. on Ctrl+C.
    pub fn shutdown(&mut self) -> Result<(), InjectError> {
        self.connected = false;
        self.release()
    }

    fn release(&mut self) -> Result<(), InjectError> {
        self.poster.post(&self.engine.disconnected(), &mut self.sink).map(|_| ())
    }
}

/// The live readout for `--monitor`.
#[derive(Debug, Default)]
pub struct Monitor {
    last_mouse: (i16, i16),
}

impl Monitor {
    pub fn screen(
        &mut self,
        name: Option<&str>,
        elapsed_ms: u128,
        report: &Report,
        data: &[u8],
    ) -> String {
        let mut s = String::from("\x1b[2J\x1b[1;1H");
        let rule = "=================================================";
        let hex: Vec<String> = data.iter().map(|b| format!("{b:02X}")).collect();
        let pressed = button_names(report.buttons);
        let (dx, dy) = (
            report.mouse_x.wrapping_sub(self.last_mouse.0),
            report.mouse_y.wrapping_sub(self.last_mouse.1),
        );
        self.last_mouse = (report.mouse_x, report.mouse_y);
        let r = report;
        let _ = writeln!(s, "{rule}\n{} Data:\n{rule}", name.unwrap_or("Unknown Device"));
        let _ = writeln!(s, "Elapsed: {elapsed_ms} ms");
        let _ = writeln!(s, "Packet_HEX: {}", hex.join(" "));
        let _ = writeln!(s, "PacketID: {}", r.packet_id);
        let _ = writeln!(s, "Buttons: {:08X}", r.buttons);
        let _ = writeln!(
            s,
            "Pressed: {}",
            if pressed.is_empty() { "None".to_owned() } else { pressed.join(", ") }
        );
        let _ = writeln!(s, "Analog_Triggers: L={}, R={}", r.trigger_l, r.trigger_r);
        let _ = writeln!(s, "LeftStick: X={}, Y={}", r.left_stick_x, r.left_stick_y);
        let _ = writeln!(s, "RightStick: X={}, Y={}", r.right_stick_x, r.right_stick_y);
        let _ = writeln!(s, "Accel: X={}, Y={}, Z={}", r.accel_x, r.accel_y, r.accel_z);
        let _ = writeln!(s, "Gyro: X={}, Y={}, Z={}", r.gyro_x, r.gyro_y, r.gyro_z);
        let _ = writeln!(s, "Mag: X={}, Y={}, Z={}", r.mag_x, r.mag_y, r.mag_z);
        let _ = writeln!(s, "Mouse: X={}, Y={}, DeltaX={dx}, DeltaY={dy}", r.mouse_x, r.mouse_y);
        let _ = writeln!(s, "Battery: {:.2}V, {:.2}mA", r.battery_voltage(), r.battery_current());
        let _ = writeln!(s, "Temperature: {:.1}°C", r.temperature());
        s
    }
}
