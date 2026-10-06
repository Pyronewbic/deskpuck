use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::{ButtonKeyMapping, MoveKind};
use deskpuck_inject::{InjectError, InputEvent, RecordingSink, Sink};
use deskpuck_replay::{Frame, MAX_REPORT_BYTES, Summary, TICK, demo, read_capture, run};
use std::collections::BTreeMap;

const RETURN: u16 = 36;
const ARROWS: [u16; 4] = [123, 124, 125, 126];

fn replay(frames: &[Frame], settings: EngineSettings) -> (Summary, Vec<InputEvent>) {
    let mut sink = RecordingSink::default();
    let summary =
        run(frames, settings, &mut sink, &mut |_| {}).expect("recording sink never fails");
    (summary, sink.events)
}

fn moves(events: &[InputEvent]) -> impl Iterator<Item = (f64, f64, MoveKind)> + '_ {
    events.iter().filter_map(|e| match *e {
        InputEvent::Move { dx, dy, kind } => Some((dx, dy, kind)),
        _ => None,
    })
}

#[test]
fn demo_never_clicks_or_presses_return() {
    let (summary, events) = replay(&demo(), EngineSettings::default());
    // Positive control: the demo does post plenty of input, including keys.
    assert!(summary.events() > 100, "{summary:?}");
    assert!(summary.keys > 0);

    assert_eq!(summary.buttons, 0);
    assert!(events.iter().all(|e| !matches!(e, InputEvent::Button { .. })));
    assert!(events.iter().all(|e| !matches!(e, InputEvent::Key { key_code: RETURN, .. })));
    assert!(moves(&events).all(|(_, _, kind)| kind == MoveKind::Moved));
}

#[test]
fn demo_draws_a_closed_square_then_pushes_down() {
    let (_, events) = replay(&demo(), EngineSettings::default());
    let (mut x, mut y, mut max_x, mut max_y) = (0.0, 0.0, 0.0f64, 0.0f64);
    for (dx, dy, _) in moves(&events) {
        x += dx;
        y += dy;
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    // 40 ticks of 25 counts at 5 counts per point: 200-point sides, back to the start.
    assert_eq!(max_x, 200.0);
    assert_eq!(x, 0.0);
    // The square closes at y=0, then the edge push travels 1200 down and 300 back up.
    assert_eq!(max_y, 1200.0);
    assert_eq!(y, 900.0);
}

#[test]
fn demo_scrolls_down_then_up_by_the_same_amount() {
    let (_, events) = replay(&demo(), EngineSettings::default());
    let scrolls: Vec<i32> = events
        .iter()
        .filter_map(|e| match *e {
            InputEvent::Scroll { up } => Some(up),
            _ => None,
        })
        .collect();
    let first_up = scrolls.iter().position(|&up| up > 0).expect("scrolls up");
    assert!(
        first_up > 0 && scrolls[..first_up].iter().all(|&up| up < 0),
        "down first: {scrolls:?}"
    );
    assert!(scrolls[first_up..].iter().all(|&up| up > 0));
    assert_eq!(scrolls.iter().sum::<i32>(), 0);
}

#[test]
fn demo_taps_each_arrow_and_releases_everything() {
    let (_, events) = replay(&demo(), EngineSettings::default());
    let mut held: BTreeMap<u16, bool> = BTreeMap::new();
    let mut presses: BTreeMap<u16, usize> = BTreeMap::new();
    let mut repeats = 0;
    for e in &events {
        if let InputEvent::Key { key_code, down, repeat } = *e {
            assert!(ARROWS.contains(&key_code), "unexpected key {key_code}");
            if repeat {
                assert!(held.get(&key_code).copied().unwrap_or(false), "repeat without a press");
                repeats += 1;
            } else {
                held.insert(key_code, down);
                *presses.entry(key_code).or_default() += usize::from(down);
            }
        }
    }
    for arrow in ARROWS {
        assert!(presses.get(&arrow).copied().unwrap_or(0) >= 1, "arrow {arrow} never pressed");
    }
    // Right is held for 0.9 s: repeats begin after 0.4 s, then every 0.06 s.
    assert!((7..=10).contains(&repeats), "{repeats} repeats");
    assert!(held.values().all(|down| !down), "keys left down: {held:?}");
}

#[test]
fn demo_is_deterministic() {
    assert_eq!(demo(), demo());
    assert_eq!(
        replay(&demo(), EngineSettings::default()),
        replay(&demo(), EngineSettings::default())
    );
}

#[test]
fn demo_follows_the_config_mapping() {
    // Remapped A (Right arrow) to Space: the held-A segment now types spaces.
    let mut settings = EngineSettings::default();
    settings.key_mappings.retain(|m| m.button_mask != 0x0000_0800);
    settings.key_mappings.push(ButtonKeyMapping::key(0x0000_0800, 49));
    let (_, events) = replay(&demo(), settings);
    assert!(events.iter().any(|e| matches!(e, InputEvent::Key { key_code: 49, .. })));
    assert!(events.iter().all(|e| !matches!(e, InputEvent::Key { key_code: 124, .. })));
}

#[test]
fn resting_capture_produces_no_input() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/joycon2_r_capture.txt"
    ))
    .expect("fixture readable");
    let frames = read_capture(&text).expect("fixture parses");
    // Positive control: every captured report was read and parsed.
    assert_eq!(frames.len(), 18);
    let (summary, events) = replay(&frames, EngineSettings::default());
    assert_eq!((summary.reports, summary.skipped), (18, 0));
    // The right stick jitters between 2099 and 2100 at rest; that must not scroll.
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn capture_parsing() {
    let frames = read_capture("# comment\n\n0102 | ignored\n  ab  \n").expect("valid");
    assert_eq!(
        frames,
        [Frame { at: 0.0, bytes: vec![1, 2] }, Frame { at: TICK, bytes: vec![0xAB] }]
    );

    let err = read_capture("00\n012\n").expect_err("odd length");
    assert!(err.starts_with("line 2:") && err.contains("even number of hex digits"), "{err}");
    assert!(read_capture("zz\n").is_err());
    assert!(read_capture("é0\n").is_err(), "multi-byte characters are not hex");
    assert!(read_capture(&"00".repeat(MAX_REPORT_BYTES + 1)).is_err());
    assert!(read_capture(&"00".repeat(MAX_REPORT_BYTES)).is_ok());
}

#[test]
fn short_reports_are_skipped_not_fatal() {
    let mut frames = demo();
    frames.insert(5, Frame { at: frames[5].at, bytes: vec![0; 10] });
    let (summary, _) = replay(&frames, EngineSettings::default());
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.reports, frames.len());
}

/// Fails on the first auto-repeat, while that key is still held, then records
/// whatever `run` posts afterwards.
#[derive(Default)]
struct FailOnFirstRepeat {
    failed: bool,
    after_failure: Vec<InputEvent>,
}

impl Sink for FailOnFirstRepeat {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        if self.failed {
            self.after_failure.push(*event);
        } else if matches!(event, InputEvent::Key { repeat: true, .. }) {
            self.failed = true;
            return Err(InjectError::Failed("sink broke".into()));
        }
        Ok(())
    }
}

#[test]
fn sink_error_still_releases_held_keys() {
    let mut sink = FailOnFirstRepeat::default();
    let result = run(&demo(), EngineSettings::default(), &mut sink, &mut |_| {});
    assert!(matches!(result, Err(InjectError::Failed(_))), "{result:?}");
    // The failure stopped the replay with Right held; the only thing posted
    // afterwards is its release.
    assert_eq!(sink.after_failure, [InputEvent::Key { key_code: 124, down: false, repeat: false }]);
}

#[test]
fn wait_hook_sees_every_frame_in_order() {
    let frames = demo();
    let mut seen = Vec::new();
    run(&frames, EngineSettings::default(), &mut RecordingSink::default(), &mut |t| seen.push(t))
        .expect("runs");
    assert_eq!(seen.len(), frames.len());
    assert!(seen.windows(2).all(|w| w[1] > w[0]));
    assert!((seen.last().copied().unwrap_or(0.0) - (frames.len() - 1) as f64 * TICK).abs() < 1e-9);
}
