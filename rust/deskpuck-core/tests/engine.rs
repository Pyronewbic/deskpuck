use deskpuck_core::engine::*;
use deskpuck_core::mapping::ButtonKeyMapping;
use deskpuck_core::packet::Report;

fn report(
    buttons: u32,
    mouse_x: i16,
    mouse_y: i16,
    left_stick_y: u16,
    right_stick_y: u16,
) -> Report {
    Report {
        buttons,
        mouse_x,
        mouse_y,
        left_stick_x: 2047,
        right_stick_x: 2047,
        left_stick_y,
        right_stick_y,
        ..Report::default()
    }
}

fn buttons(buttons: u32) -> Report {
    report(buttons, 0, 0, 2047, 2047)
}

fn mouse(x: i16, y: i16) -> Report {
    report(0, x, y, 2047, 2047)
}

fn sticks(left_y: u16, right_y: u16) -> Report {
    report(0, 0, 0, left_y, right_y)
}

const R: u32 = 0x0000_4000;
const ZR: u32 = 0x0000_8000;
const RS: u32 = 0x0004_0000;
const LS: u32 = 0x0008_0000;
const L: u32 = 0x4000_0000;
const ZL: u32 = 0x8000_0000;

#[test]
fn mouse_button_mapping() {
    assert_eq!(mouse_buttons_for_joycon_buttons(0), 0);
    assert_eq!(mouse_buttons_for_joycon_buttons(R), 1);
    assert_eq!(mouse_buttons_for_joycon_buttons(ZL), 1);
    assert_eq!(mouse_buttons_for_joycon_buttons(ZR), 2);
    assert_eq!(mouse_buttons_for_joycon_buttons(L), 2);
    assert_eq!(mouse_buttons_for_joycon_buttons(LS), 4);
    assert_eq!(mouse_buttons_for_joycon_buttons(R | ZR | LS), 7);
    // The right stick click is a key (Return), not a mouse button.
    assert_eq!(mouse_buttons_for_joycon_buttons(RS), 0);
}

#[test]
fn wheel_levels() {
    assert_eq!(wheel_for_stick_deviation(0), 0);
    assert_eq!(wheel_for_stick_deviation(30), 0);
    assert_eq!(wheel_for_stick_deviation(-30), 0);
    // Past the deadzone but under one 60-unit level: still zero.
    assert_eq!(wheel_for_stick_deviation(59), 0);
    assert_eq!(wheel_for_stick_deviation(60), -5);
    assert_eq!(wheel_for_stick_deviation(-60), 5);
    assert_eq!(wheel_for_stick_deviation(1200), -100);
    assert_eq!(wheel_for_stick_deviation(4000), -100);
    assert_eq!(wheel_for_stick_deviation(i32::MIN), 100);
}

#[test]
fn scroll_centres_on_first_report() {
    let mut engine = InputEngine::default();
    // The first report defines the resting position, whatever it is.
    assert_eq!(engine.process(&sticks(2047, 2100), 0.0).wheel, 0);
    assert_eq!(engine.process(&sticks(2047, 2220), 0.1).wheel, -10);
    assert_eq!(engine.process(&sticks(2047, 1980), 0.2).wheel, 10);

    // A new connection recentres on its first report.
    engine.connection_started();
    assert_eq!(engine.process(&sticks(2047, 2300), 0.3).wheel, 0);
    assert_eq!(engine.process(&sticks(2047, 2300), 0.4).wheel, 0);
}

#[test]
fn wheel_clamped() {
    let mut engine = InputEngine::default();
    engine.process(&sticks(2047, 2047), 0.0);
    // Both sticks at full deflection would sum to 200.
    assert_eq!(engine.process(&sticks(747, 747), 0.1).wheel, 127);
    assert_eq!(engine.process(&sticks(3347, 3347), 0.2).wheel, -127);
}

#[test]
fn pointer_deltas() {
    let mut engine = InputEngine::default();
    engine.process(&mouse(100, -40), 0.0);
    let out = engine.process(&mouse(150, -60), 0.1);
    assert_eq!((out.dx, out.dy), (10.0, -4.0));

    // Counters are 16-bit and wrap: 32760 -> -32766 is a step of +10.
    engine.process(&mouse(32760, 0), 0.2);
    let out = engine.process(&mouse(-32766, 0), 0.3);
    assert_eq!((out.dx, out.dy), (2.0, 0.0));

    // No sensor movement, no pointer movement.
    let out = engine.process(&mouse(-32766, 0), 0.4);
    assert_eq!((out.dx, out.dy), (0.0, 0.0));
}

#[test]
fn pointer_speed() {
    let mut engine =
        InputEngine::new(EngineSettings { pointer_speed: 2.0, ..EngineSettings::default() });
    engine.process(&mouse(0, 0), 0.0);
    assert_eq!(engine.process(&mouse(50, 25), 0.1).dx, 20.0);
}

#[test]
fn keys_pass_through() {
    let mut engine = InputEngine::default();
    let out = engine.process(&buttons(RS), 0.0);
    assert_eq!(out.keys.len(), 1);
    assert!(out.keys[0].key_code == 36 && out.keys[0].is_down);
    assert_eq!(out.mouse_buttons, 0);
}

#[test]
fn button_bits_exact() {
    // ZL plus noise in the unmapped low byte. Rounding through float would turn
    // 0x800000FF into 0x80000100 and press Y (Left arrow).
    let mut engine = InputEngine::default();
    let out = engine.process(&buttons(0x8000_00FF), 0.0);
    assert!(out.keys.is_empty());
    assert_eq!(out.mouse_buttons, 1);
}

#[test]
fn no_jump_on_connect() {
    // The sensor counter is absolute; its first value says nothing about motion.
    let mut engine = InputEngine::default();
    let out = engine.process(&mouse(12000, -9000), 0.0);
    assert_eq!((out.dx, out.dy), (0.0, 0.0));

    // Same after a reconnect, where the counter may have moved while disconnected.
    engine.connection_started();
    let out = engine.process(&mouse(5000, 300), 0.1);
    assert_eq!((out.dx, out.dy), (0.0, 0.0));

    // Positive control: the next report moves from the new baseline.
    let out = engine.process(&mouse(5050, 300), 0.2);
    assert_eq!((out.dx, out.dy), (10.0, 0.0));
}

#[test]
fn disconnect_releases_everything() {
    let mut engine = InputEngine::default();
    let held = engine.process(&buttons(R | RS), 0.0);
    assert!(held.mouse_buttons == 1 && held.keys.len() == 1);

    let out = engine.disconnected();
    assert_eq!(out.mouse_buttons, 0);
    assert_eq!(out.keys.len(), 1);
    assert!(out.keys[0].key_code == 36 && !out.keys[0].is_down);
    assert_eq!((out.dx, out.dy, out.wheel), (0.0, 0.0, 0));
}

#[test]
fn no_scroll_while_stick_clicked() {
    let mut engine = InputEngine::default();
    engine.process(&sticks(2047, 2047), 0.0);
    // Pressing the right stick in tilts it slightly; that must not scroll.
    assert_eq!(engine.process(&report(RS, 0, 0, 2047, 2167), 0.1).wheel, 0);
    // Same for the left stick on a left Joy-Con.
    assert_eq!(engine.process(&report(LS, 0, 0, 2167, 2047), 0.2).wheel, 0);
    // Positive control: the same tilts scroll once the stick is released.
    assert_eq!(engine.process(&sticks(2047, 2167), 0.3).wheel, -10);
    assert_eq!(engine.process(&sticks(2167, 2047), 0.4).wheel, -10);
    // Clicking one stick does not silence the other.
    assert_eq!(engine.process(&report(RS, 0, 0, 2167, 2167), 0.5).wheel, -10);
}

#[test]
fn scroll_disabled() {
    let mut engine =
        InputEngine::new(EngineSettings { scroll_enabled: false, ..EngineSettings::default() });
    engine.process(&sticks(2047, 2047), 0.0);
    assert_eq!(engine.process(&sticks(2047, 2647), 0.1).wheel, 0);
    assert_eq!(engine.process(&sticks(2647, 2047), 0.2).wheel, 0);

    // Positive control: the same deflections scroll with scrolling on.
    let mut enabled = InputEngine::default();
    enabled.process(&sticks(2047, 2047), 0.0);
    assert_eq!(enabled.process(&sticks(2047, 2647), 0.1).wheel, -50);
    assert_eq!(enabled.process(&sticks(2647, 2047), 0.2).wheel, -50);
}

#[test]
fn apply_settings() {
    let mut engine = InputEngine::default();
    engine.process(&report(RS, 1000, 0, 2047, 2100), 0.0);

    // Remap RS from Return to Space while it is held: Return must be released.
    let released = engine.apply_settings(EngineSettings {
        key_mappings: vec![ButtonKeyMapping { button_mask: RS, key_code: 49 }],
        pointer_speed: 2.0,
        ..EngineSettings::default()
    });
    assert_eq!(released.len(), 1);
    assert!(released[0].key_code == 36 && !released[0].is_down);

    // Still held after the change: a fresh press under the new mapping.
    let out = engine.process(&report(RS, 1050, 0, 2047, 2100), 0.1);
    assert_eq!(out.keys.len(), 1);
    assert!(out.keys[0].key_code == 49 && out.keys[0].is_down);
    // Pointer baseline survives (no jump) and the new speed applies: 50 * 2 / 5.
    assert_eq!(out.dx, 20.0);
    // Scroll centre survives: same stick position, no scroll.
    assert_eq!(out.wheel, 0);
}
