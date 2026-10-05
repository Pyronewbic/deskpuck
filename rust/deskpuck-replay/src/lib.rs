//! Replays Joy-Con 2 reports through the engine and a sink, so injection can be
//! tested on any machine without Bluetooth.

use deskpuck_core::engine::{EngineSettings, InputEngine};
use deskpuck_core::packet::{Report, encode_report, parse_report};
use deskpuck_inject::{InjectError, InputEvent, Poster, Sink};

/// Seconds between reports; the Joy-Con 2 sends roughly 66 per second.
pub const TICK: f64 = 0.015;
/// Longest accepted capture line, in bytes of report data.
pub const MAX_REPORT_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Seconds from the start of the replay.
    pub at: f64,
    pub bytes: Vec<u8>,
}

const BUTTON_Y: u32 = 0x0000_0100;
const BUTTON_X: u32 = 0x0000_0200;
const BUTTON_B: u32 = 0x0000_0400;
const BUTTON_A: u32 = 0x0000_0800;
const STICK_CENTRE: u16 = 2047;
/// Optical-sensor counts per tick while travelling; 5 counts are one point at speed 1.
const TRAVEL: i16 = 25;

struct Script {
    report: Report,
    frames: Vec<Frame>,
}

impl Script {
    fn emit(&mut self) {
        self.report.packet_id = (self.report.packet_id + 1) & 0xFF_FFFF;
        let at = self.frames.len() as f64 * TICK;
        self.frames.push(Frame { at, bytes: encode_report(&self.report) });
    }

    fn rest(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.emit();
        }
    }

    fn travel(&mut self, dx: i16, dy: i16, ticks: usize) {
        for _ in 0..ticks {
            self.report.mouse_x = self.report.mouse_x.wrapping_add(dx);
            self.report.mouse_y = self.report.mouse_y.wrapping_add(dy);
            self.emit();
        }
    }

    fn hold(&mut self, buttons: u32, ticks: usize) {
        self.report.buttons = buttons;
        self.rest(ticks);
        self.report.buttons = 0;
    }

    fn tilt_right_stick(&mut self, offset: i16, ticks: usize) {
        self.report.right_stick_y = STICK_CENTRE.wrapping_add_signed(offset);
        self.rest(ticks);
        self.report.right_stick_y = STICK_CENTRE;
    }
}

/// About eight seconds of input that is safe on a live desktop: a 200-point
/// square, a push into the bottom screen edge (the Dock), a scroll down and
/// up, and arrow-key taps with one held long enough to auto-repeat. It never
/// presses a mouse button or Return.
pub fn demo() -> Vec<Frame> {
    let report = Report {
        // The counter is absolute; starting away from zero exercises the baseline.
        mouse_x: 1000,
        mouse_y: -1000,
        left_stick_x: STICK_CENTRE,
        left_stick_y: STICK_CENTRE,
        right_stick_x: STICK_CENTRE,
        right_stick_y: STICK_CENTRE,
        ..Report::default()
    };
    let mut s = Script { report, frames: Vec::new() };
    s.rest(10);

    for (dx, dy) in [(TRAVEL, 0), (0, TRAVEL), (-TRAVEL, 0), (0, -TRAVEL)] {
        s.travel(dx, dy, 40);
    }
    s.rest(20);

    // Far enough to reach the bottom edge from anywhere on a tall display, then
    // dwell there so an auto-hidden Dock has time to appear.
    s.travel(0, 4 * TRAVEL, 60);
    s.rest(70);
    s.travel(0, -4 * TRAVEL, 15);
    s.rest(20);

    s.tilt_right_stick(-300, 20);
    s.rest(10);
    s.tilt_right_stick(300, 20);
    s.rest(20);

    for button in [BUTTON_X, BUTTON_B, BUTTON_Y, BUTTON_A] {
        s.hold(button, 4);
        s.rest(10);
    }
    s.hold(BUTTON_A, 60);
    s.rest(10);
    s.frames
}

/// Reads a capture file: one report per line as hex, optionally followed by
/// " | " and anything else (the golden fixture's expected values). Blank lines
/// and lines starting with '#' are skipped. Reports are spaced `TICK` apart.
pub fn read_capture(text: &str) -> Result<Vec<Frame>, String> {
    let mut frames = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let hex = line.split_once(" | ").map_or(line, |(hex, _)| hex).trim();
        let number = index + 1;
        if hex.len() % 2 != 0 || hex.len() / 2 > MAX_REPORT_BYTES {
            return Err(format!(
                "line {number}: expected an even number of hex digits, at most {MAX_REPORT_BYTES} bytes"
            ));
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| hex.get(i..i + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| format!("line {number}: not hex"))?;
        frames.push(Frame { at: frames.len() as f64 * TICK, bytes });
    }
    Ok(frames)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub reports: usize,
    /// Reports too short to parse; the engine never sees them.
    pub skipped: usize,
    pub moves: usize,
    pub buttons: usize,
    pub scrolls: usize,
    pub keys: usize,
}

impl Summary {
    fn count(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Move { .. } => self.moves += 1,
            InputEvent::Button { .. } => self.buttons += 1,
            InputEvent::Scroll { .. } => self.scrolls += 1,
            InputEvent::Key { .. } => self.keys += 1,
        }
    }

    pub fn events(&self) -> usize {
        self.moves + self.buttons + self.scrolls + self.keys
    }
}

struct Counting<'a> {
    sink: &'a mut dyn Sink,
    summary: Summary,
}

impl Sink for Counting<'_> {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        self.sink.post(event)?;
        self.summary.count(event);
        Ok(())
    }
}

/// Feeds every frame through the engine into `sink`, calling `wait_until`
/// with each frame's time first. Ends as a disconnect would, releasing every
/// held key and button, including after a sink error.
pub fn run(
    frames: &[Frame],
    settings: EngineSettings,
    sink: &mut dyn Sink,
    wait_until: &mut dyn FnMut(f64),
) -> Result<Summary, InjectError> {
    let mut engine = InputEngine::new(settings);
    let mut poster = Poster::default();
    let mut counting = Counting { sink, summary: Summary::default() };

    let mut result = Ok(());
    for frame in frames {
        wait_until(frame.at);
        counting.summary.reports += 1;
        let Some(report) = parse_report(&frame.bytes) else {
            counting.summary.skipped += 1;
            continue;
        };
        if let Err(e) = poster.post(&engine.process(&report, frame.at), &mut counting) {
            result = Err(e);
            break;
        }
    }
    let released = poster.post(&engine.disconnected(), &mut counting);
    result.and(released).map(|_| counting.summary)
}
