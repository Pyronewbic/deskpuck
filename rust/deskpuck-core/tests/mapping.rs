use deskpuck_core::mapping::*;
use std::cell::Cell;

// Two displays: main 1920x1080 at origin, second 1280x1024 to its right, offset 200px down.
const MAIN: Rect = Rect { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 };
const SIDE: Rect = Rect { x: 1920.0, y: 200.0, width: 1280.0, height: 1024.0 };

fn display_at(p: Point) -> Option<Rect> {
    [MAIN, SIDE].into_iter().find(|d| d.contains(p))
}

fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn clamp_to_displays_cases() {
    // Control: a move within one display is unchanged.
    assert_eq!(
        clamp_to_displays(pt(600.0, 500.0), pt(500.0, 500.0), display_at, MAIN),
        pt(600.0, 500.0)
    );

    // Crossing onto the second display is allowed, and the lookup actually ran.
    let lookups = Cell::new(0);
    let counting = |p| {
        lookups.set(lookups.get() + 1);
        display_at(p)
    };
    assert_eq!(
        clamp_to_displays(pt(2000.0, 500.0), pt(1900.0, 500.0), counting, MAIN),
        pt(2000.0, 500.0)
    );
    assert!(lookups.get() > 0);

    // Pushing past the bottom of the main display stops on its last pixel row (Dock trigger), on-screen.
    let bottom = clamp_to_displays(pt(500.0, 1200.0), pt(500.0, 1079.0), display_at, MAIN);
    assert_eq!(bottom, pt(500.0, 1079.0));
    assert!(display_at(bottom).is_some());

    // Off all displays from the second display clamps to the second display, not main.
    assert_eq!(
        clamp_to_displays(pt(3500.0, 100.0), pt(3000.0, 300.0), display_at, MAIN),
        pt(3199.0, 200.0)
    );

    // Gap above the offset second display: clamps back onto the display we came from.
    assert_eq!(
        clamp_to_displays(pt(1950.0, 150.0), pt(1950.0, 250.0), display_at, MAIN),
        pt(1950.0, 200.0)
    );

    // Origin off every display (e.g. display unplugged) falls back to the given bounds.
    assert_eq!(
        clamp_to_displays(pt(-50.0, 5000.0), pt(-40.0, 4000.0), display_at, MAIN),
        pt(0.0, 1079.0)
    );
}

#[test]
fn move_kind_for_held_buttons() {
    assert_eq!(mouse_move_kind(0), MoveKind::Moved);
    assert_eq!(mouse_move_kind(1), MoveKind::Dragged(MouseButton::Left));
    assert_eq!(mouse_move_kind(2), MoveKind::Dragged(MouseButton::Right));
    assert_eq!(mouse_move_kind(4), MoveKind::Dragged(MouseButton::Middle));
    // Left wins when several buttons are held.
    assert_eq!(mouse_move_kind(1 | 2 | 4), MoveKind::Dragged(MouseButton::Left));
}

const A: u32 = 0x0000_0800;
const X: u32 = 0x0000_0200;
const UNMAPPED: u32 = 0x0010_0000; // HOME

fn test_mappings() -> Vec<ButtonKeyMapping> {
    vec![
        ButtonKeyMapping { button_mask: A, key_code: 124, modifiers: Modifiers::NONE },
        ButtonKeyMapping { button_mask: X, key_code: 126, modifiers: Modifiers::NONE },
    ]
}

fn ev(key_code: KeyCode, is_down: bool, is_repeat: bool) -> KeyEvent {
    KeyEvent { key_code, is_down, is_repeat }
}

#[test]
fn key_press_and_release() {
    // Control: a tap sends exactly one down then one up.
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    assert_eq!(keys.update(A, 0.0), [ev(124, true, false)]);
    assert_eq!(keys.update(0, 0.1), [ev(124, false, false)]);
    assert!(keys.update(0, 0.2).is_empty());
}

#[test]
fn key_repeat() {
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    assert_eq!(keys.update(A, 0.0).len(), 1);

    // Held, before the delay: nothing.
    assert!(keys.update(A, 0.39).is_empty());

    // At the delay: one repeat, then one per interval.
    assert_eq!(keys.update(A, 0.40), [ev(124, true, true)]);
    assert!(keys.update(A, 0.45).is_empty());
    assert_eq!(keys.update(A, 0.46), [ev(124, true, true)]);

    // A long packet gap yields one repeat, not a burst, and the schedule restarts
    // from the gap rather than firing on every packet until it catches up.
    assert_eq!(keys.update(A, 5.0).len(), 1);
    assert!(keys.update(A, 5.03).is_empty());

    // Release sends a plain key up and stops repeating; the repeat just above
    // proves the same repeater would otherwise have fired.
    assert_eq!(keys.update(0, 5.1), [ev(124, false, false)]);
    assert!(keys.update(0, 10.0).is_empty());
}

#[test]
fn independent_keys() {
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    assert_eq!(keys.update(A, 0.0).len(), 1);
    assert_eq!(keys.update(A | X, 0.2), [ev(126, true, false)]);

    // At 0.4 only A has been held long enough to repeat.
    assert_eq!(keys.update(A | X, 0.4), [ev(124, true, true)]);
    // Just past X's own delay (0.2 + 0.4; not exactly 0.6 in floating point) both repeat.
    assert_eq!(keys.update(A | X, 0.61), [ev(124, true, true), ev(126, true, true)]);
}

#[test]
fn unmapped_buttons_ignored() {
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    assert!(keys.update(UNMAPPED, 0.0).is_empty());
    assert!(keys.update(UNMAPPED, 1.0).is_empty());
    // Positive control: a mapped button on the same repeater still produces events.
    assert_eq!(keys.update(UNMAPPED | A, 1.1).len(), 1);
}

fn repeats_while_held(keys: &mut KeyRepeater) -> usize {
    let mut repeats = 0;
    let mut t = 0.03;
    while t < 3.0 {
        repeats += keys.update(A, t).len();
        t += 0.03;
    }
    repeats
}

#[test]
fn repeat_disable_sentinels() {
    let disabled = [
        (0.4, 0.0),      // interval 0 must disable, not repeat every packet
        (0.4, -1.0),     // negative interval
        (-1.0, 0.06),    // negative delay
        (0.4, f64::NAN), // malformed interval
        (f64::NAN, 0.06),
    ];
    for (delay, interval) in disabled {
        let mut keys = KeyRepeater::new(test_mappings(), delay, interval);
        assert_eq!(keys.update(A, 0.0).len(), 1);
        assert_eq!(repeats_while_held(&mut keys), 0, "delay {delay} interval {interval}");
        // Press and release still work with repeat off.
        assert_eq!(keys.update(0, 3.0), [ev(124, false, false)]);
    }

    // Positive control: the same loop with valid knobs does repeat.
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    keys.update(A, 0.0);
    assert!(repeats_while_held(&mut keys) > 0);

    // Delay 0 is valid: repeat starts on the next packet.
    let mut immediate = KeyRepeater::new(test_mappings(), 0.0, 0.06);
    immediate.update(A, 0.0);
    assert_eq!(immediate.update(A, 0.03).len(), 1);
}

#[test]
fn default_mappings() {
    let mappings = default_button_key_mappings();
    let key_for = |mask| mappings.iter().find(|m| m.button_mask == mask).map(|m| m.key_code);
    assert_eq!(key_for(0x0004_0000), Some(36)); // RS -> Return
    assert_eq!(key_for(0x0000_0200), Some(126)); // X -> Up
    assert_eq!(key_for(0x0000_0400), Some(125)); // B -> Down
    assert_eq!(key_for(0x0000_0100), Some(123)); // Y -> Left
    assert_eq!(key_for(0x0000_0800), Some(124)); // A -> Right
    // R and ZR stay mouse buttons.
    assert_eq!(key_for(0x0000_4000), None);
    assert_eq!(key_for(0x0000_8000), None);
}

#[test]
fn release_all() {
    let mut keys = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    keys.update(A | X, 0.0);
    assert_eq!(keys.release_all(), [ev(124, false, false), ev(126, false, false)]);

    // Released keys stay released, and a second release has nothing left to send.
    assert!(keys.release_all().is_empty());

    // Still holding A on reconnect counts as a fresh press.
    assert_eq!(keys.update(A, 5.0), [ev(124, true, false)]);

    // Nothing held: nothing to release.
    assert!(KeyRepeater::new(test_mappings(), 0.4, 0.06).release_all().is_empty());
}

const C: KeyCode = 8;
const V: KeyCode = 9;
const CONTROL: KeyCode = 59;

fn shortcut(button_mask: u32, key_code: KeyCode, modifiers: Modifiers) -> ButtonKeyMapping {
    ButtonKeyMapping { button_mask, key_code, modifiers }
}

#[test]
fn shortcut_presses_modifiers_around_the_key_and_never_repeats() {
    let mut keys = KeyRepeater::new(vec![shortcut(A, C, Modifiers::CONTROL)], 0.4, 0.06);
    assert_eq!(keys.update(A, 0.0), [ev(CONTROL, true, false), ev(C, true, false)]);
    // Held well past the repeat delay: a shortcut fires once.
    for t in [0.5, 1.0, 5.0] {
        assert!(keys.update(A, t).is_empty(), "repeated at {t}");
    }
    assert_eq!(keys.update(0, 5.1), [ev(C, false, false), ev(CONTROL, false, false)]);
    // Positive control: a plain key under the same timing does repeat.
    let mut plain = KeyRepeater::new(test_mappings(), 0.4, 0.06);
    plain.update(A, 0.0);
    assert_eq!(plain.update(A, 0.5), [ev(124, true, true)]);
}

#[test]
fn modifiers_go_down_in_a_fixed_order_and_up_in_reverse() {
    let all =
        Modifiers::COMMAND.with(Modifiers::SHIFT).with(Modifiers::OPTION).with(Modifiers::CONTROL);
    let mut keys = KeyRepeater::new(vec![shortcut(A, C, all)], 0.4, 0.06);
    assert_eq!(
        keys.update(A, 0.0),
        [
            ev(59, true, false),
            ev(58, true, false),
            ev(56, true, false),
            ev(55, true, false),
            ev(C, true, false)
        ]
    );
    assert_eq!(
        keys.update(0, 0.1),
        [
            ev(C, false, false),
            ev(55, false, false),
            ev(56, false, false),
            ev(58, false, false),
            ev(59, false, false)
        ]
    );
    assert_eq!(all.names(), ["control", "option", "shift", "command"]);
}

#[test]
fn shortcuts_sharing_a_modifier_hold_it_until_the_last_is_released() {
    let mappings = vec![shortcut(A, C, Modifiers::CONTROL), shortcut(X, V, Modifiers::CONTROL)];
    let mut keys = KeyRepeater::new(mappings, 0.4, 0.06);
    assert_eq!(keys.update(A, 0.0), [ev(CONTROL, true, false), ev(C, true, false)]);
    assert_eq!(keys.update(A | X, 0.1), [ev(V, true, false)]);
    assert_eq!(keys.update(X, 0.2), [ev(C, false, false)], "Control still held for V");
    assert_eq!(keys.update(0, 0.3), [ev(V, false, false), ev(CONTROL, false, false)]);
    // Pressing again starts from zero: Control goes down once more.
    assert_eq!(keys.update(A, 0.4), [ev(CONTROL, true, false), ev(C, true, false)]);
}

#[test]
fn release_all_lets_go_of_shortcut_modifiers() {
    let mappings = vec![shortcut(A, C, Modifiers::CONTROL), shortcut(X, V, Modifiers::CONTROL)];
    let mut keys = KeyRepeater::new(mappings, 0.4, 0.06);
    keys.update(A | X, 0.0);
    assert_eq!(
        keys.release_all(),
        [ev(C, false, false), ev(V, false, false), ev(CONTROL, false, false)]
    );
    assert!(keys.release_all().is_empty());
    assert!(keys.update(0, 0.1).is_empty(), "nothing left to release");
}
