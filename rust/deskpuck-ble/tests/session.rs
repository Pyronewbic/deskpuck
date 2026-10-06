use deskpuck_ble::receiver::Status;
use deskpuck_ble::{Monitor, Session};
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::{ButtonKeyMapping, Modifiers, MouseButton};
use deskpuck_core::packet::{Report, parse_report};
use deskpuck_inject::{InputEvent, RecordingSink};

const RS: u32 = 0x0004_0000;
const RETURN: u16 = 36;

fn report(buttons: u32, mouse_x: i16) -> Report {
    Report { buttons, mouse_x, left_stick_y: 2047, right_stick_y: 2047, ..Report::default() }
}

fn key(down: bool) -> InputEvent {
    InputEvent::Key { key_code: RETURN, down, repeat: false }
}

#[test]
fn reports_only_count_while_connected() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.report(&report(RS, 0), 0.0).expect("post");
    session.status(Status::Connecting).expect("status");
    session.report(&report(RS, 0), 0.1).expect("post");
    // Positive control: the same report once connected presses the key.
    session.status(Status::Connected).expect("status");
    session.report(&report(RS, 0), 0.2).expect("post");
    assert_eq!(session.sink().events, [key(true)]);
}

#[test]
fn disconnect_releases_held_input() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(RS, 0), 0.0).expect("post");
    session.status(Status::Searching).expect("status");
    // A second non-connected status has nothing left to release.
    session.status(Status::BluetoothOff).expect("status");
    assert_eq!(session.sink().events, [key(true), key(false)]);
}

#[test]
fn shutdown_releases_held_input() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(RS, 0), 0.0).expect("post");
    session.shutdown().expect("release");
    // Reports after shutdown are ignored.
    session.report(&report(RS, 0), 0.1).expect("post");
    assert_eq!(session.sink().events, [key(true), key(false)]);
}

#[test]
fn reconnect_does_not_jump_the_pointer() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(0, 100), 0.0).expect("post");
    session.status(Status::Searching).expect("status");
    session.status(Status::Connected).expect("status");
    // The counter moved while disconnected; the first report is a new baseline.
    session.report(&report(0, 5000), 1.0).expect("post");
    // Positive control: the next report moves from that baseline.
    session.report(&report(0, 5050), 1.1).expect("post");
    let moves: Vec<_> =
        session.sink().events.iter().filter(|e| matches!(e, InputEvent::Move { .. })).collect();
    assert_eq!(moves.len(), 1, "{moves:?}");
    assert!(matches!(moves[0], InputEvent::Move { dx, .. } if *dx == 10.0));
}

#[test]
fn monitor_screen_matches_the_cpp_fields() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/joycon2_r_capture.txt"
    ))
    .expect("fixture");
    let line = text.lines().find(|l| !l.starts_with('#') && !l.is_empty()).expect("a report");
    let hex = line.split(" | ").next().expect("hex");
    let data: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect();
    let report = parse_report(&data).expect("parses");

    let mut monitor = Monitor::default();
    let screen = monitor.screen(Some("Joy-Con 2 (R)"), 1234, &report, &data);
    for expected in [
        "Joy-Con 2 (R) Data:",
        "Elapsed: 1234 ms",
        "Packet_HEX: 68 0D 00 00 00 00 00 E0 FF 0F FF F7 7F",
        "PacketID: 3432",
        "Buttons: 00000000",
        "Pressed: None",
        "Analog_Triggers: L=0, R=0",
        "LeftStick: X=2047, Y=2047",
        "RightStick: X=2006, Y=2100",
        "Mouse: X=0, Y=0, DeltaX=0, DeltaY=0",
        "Battery: 3.41V, 2.56mA",
        "Temperature: 25.0°C",
    ] {
        assert!(screen.contains(expected), "missing {expected:?} in:\n{screen}");
    }

    // Deltas are against the previous screen, wrapping like the 16-bit counter.
    let moved = Report { mouse_x: -32766, ..report };
    let mut monitor = Monitor::default();
    monitor.screen(None, 0, &Report { mouse_x: 32760, ..report }, &data);
    assert!(monitor.screen(None, 0, &moved, &data).contains("DeltaX=10,"));
    assert!(monitor.screen(None, 0, &moved, &data).contains("Unknown Device Data:"));
}

const R: u32 = 0x0000_4000;

#[test]
fn pause_releases_input_and_ignores_reports() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(RS, 0), 0.0).expect("post");
    session.set_paused(true).expect("pause");
    assert!(session.is_paused());
    session.report(&report(RS, 500), 0.1).expect("post");
    // Pausing twice releases nothing more.
    session.set_paused(true).expect("pause");
    assert_eq!(session.sink().events, [key(true), key(false)]);
}

#[test]
fn resume_does_not_jump_the_pointer() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(0, 100), 0.0).expect("post");
    session.set_paused(true).expect("pause");
    session.report(&report(0, 3000), 0.5).expect("post");
    session.set_paused(false).expect("resume");
    // The counter moved while paused; the first report after resuming is a baseline.
    session.report(&report(0, 4000), 1.0).expect("post");
    session.report(&report(0, 4050), 1.1).expect("post");
    let moves: Vec<_> =
        session.sink().events.iter().filter(|e| matches!(e, InputEvent::Move { .. })).collect();
    assert_eq!(moves.len(), 1, "{moves:?}");
    assert!(matches!(moves[0], InputEvent::Move { dx, .. } if *dx == 10.0));
}

#[test]
fn new_settings_release_old_keys_but_keep_a_held_click() {
    let mut session = Session::new(EngineSettings::default(), RecordingSink::default());
    session.status(Status::Connected).expect("status");
    session.report(&report(RS | R, 0), 0.0).expect("post");
    let settings = EngineSettings {
        key_mappings: vec![ButtonKeyMapping {
            button_mask: RS,
            key_code: 49,
            modifiers: Modifiers::NONE,
        }],
        ..EngineSettings::default()
    };
    session.apply_settings(settings).expect("apply");
    assert_eq!(
        session.sink().events,
        [InputEvent::Button { button: MouseButton::Left, down: true }, key(true), key(false)]
    );
    // Still held: the next report presses the newly mapped key and the click stays down.
    session.report(&report(RS | R, 0), 0.1).expect("post");
    assert_eq!(
        session.sink().events.last(),
        Some(&InputEvent::Key { key_code: 49, down: true, repeat: false })
    );
    assert!(
        !session
            .sink()
            .events
            .contains(&InputEvent::Button { button: MouseButton::Left, down: false })
    );
}

#[test]
fn pause_apply_and_shutdown_release_a_held_shortcut() {
    const C: u16 = 8;
    const CONTROL: u16 = 59;
    let copy = EngineSettings {
        key_mappings: vec![ButtonKeyMapping {
            button_mask: RS,
            key_code: C,
            modifiers: Modifiers::CONTROL,
        }],
        ..EngineSettings::default()
    };
    let k = |key_code, down| InputEvent::Key { key_code, down, repeat: false };
    let held = [k(CONTROL, true), k(C, true)];
    let released = [k(C, false), k(CONTROL, false)];
    let pressed = |session: &mut Session<RecordingSink>| {
        session.status(Status::Connected).expect("status");
        session.report(&report(RS, 0), 0.0).expect("post");
        assert_eq!(session.sink().events, held);
    };

    let mut session = Session::new(copy.clone(), RecordingSink::default());
    pressed(&mut session);
    session.set_paused(true).expect("pause");
    assert_eq!(session.sink().events[2..], released, "pause");

    let mut session = Session::new(copy.clone(), RecordingSink::default());
    pressed(&mut session);
    session.apply_settings(EngineSettings::default()).expect("apply");
    assert_eq!(session.sink().events[2..], released, "apply");

    let mut session = Session::new(copy.clone(), RecordingSink::default());
    pressed(&mut session);
    session.shutdown().expect("shutdown");
    assert_eq!(session.sink().events[2..], released, "shutdown");

    let mut session = Session::new(copy, RecordingSink::default());
    pressed(&mut session);
    session.status(Status::Searching).expect("disconnect");
    assert_eq!(session.sink().events[2..], released, "disconnect");
}
