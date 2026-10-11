mod common;

use common::capture::*;
use deskpuck_ble::vendor::*;

#[test]
fn the_challenge_answer_matches_both_captures() {
    let b2 = |t: &Transcript| block(&t.exchange[2].1[27..]);
    for t in [&MAC, &ALLY] {
        let want = b2(t);
        assert_eq!(expected_answer(&block(t.a1), &block(B1), &block(t.a2)), want, "{}", t.host);
        // Control: the key without its byte reversal gives something else.
        let mut a1 = block(t.a1);
        a1.reverse();
        assert_ne!(expected_answer(&a1, &block(B1), &block(t.a2)), want);
    }
}

#[test]
fn replays_both_captured_pairings_byte_for_byte() {
    for t in [&MAC, &ALLY] {
        let mut pairing = Pairing::new(secrets(t));
        assert_eq!(pairing.first(), bytes(t.exchange[0].0), "{}", t.host);
        for i in 0..3 {
            assert_eq!(
                pairing.receive(&answer(t, i)),
                Progress::Send(bytes(t.exchange[i + 1].0)),
                "{} step {i}",
                t.host
            );
        }
        assert_eq!(pairing.receive(&answer(t, 3)), Progress::Done);
        assert_eq!(pairing.receive(&answer(t, 3)), Progress::Ignored, "nothing after done");
    }
}

#[test]
fn a_wrong_answer_stops_before_the_commit() {
    let mut pairing = Pairing::new(secrets(&MAC));
    pairing.receive(&answer(&MAC, 0));
    pairing.receive(&answer(&MAC, 1));
    // The Ally's answer is right for its own blocks, not for these.
    let progress = pairing.receive(&answer(&ALLY, 2));
    assert!(
        matches!(progress, Progress::Failed(ref why) if why.contains("answer")),
        "{progress:?}"
    );
    assert_eq!(
        pairing.receive(&answer(&MAC, 2)),
        Progress::Ignored,
        "a failed pairing stays failed"
    );
}

#[test]
fn replies_to_another_step_or_command_are_ignored() {
    let mut pairing = Pairing::new(secrets(&MAC));
    for other in [
        answer(&MAC, 1),
        answer(&MAC, 3),
        bytes("09 01 01 07 10 78 00 00 01"),
        bytes("15 01 01"),
        vec![],
    ] {
        assert_eq!(pairing.receive(&other), Progress::Ignored, "{other:02x?}");
    }
    assert!(matches!(pairing.receive(&answer(&MAC, 0)), Progress::Send(_)), "control");
}

type Edit = dyn Fn(&mut Vec<u8>);

#[test]
fn refusals_and_short_replies_fail_the_step() {
    let refused = |step: usize, edit: &Edit| {
        let mut pairing = Pairing::new(secrets(&MAC));
        for i in 0..step {
            pairing.receive(&answer(&MAC, i));
        }
        let mut reply = answer(&MAC, step);
        edit(&mut reply);
        pairing.receive(&reply)
    };
    let cases: [(usize, &Edit); 6] = [
        (0, &|r| r[1] = 0x00),
        (0, &|r| r[8] = 0x00),
        (0, &|r| r.truncate(8)),
        (1, &|r| r.truncate(24)),
        (2, &|r| r.truncate(9)),
        (3, &|r| r[8] = 0x02),
    ];
    for (step, edit) in cases {
        assert!(matches!(refused(step, edit), Progress::Failed(_)), "step {step}");
    }
    assert!(matches!(refused(0, &|_| {}), Progress::Send(_)), "control: unedited succeeds");
}

#[test]
fn secrets_never_reach_debug_output() {
    let shown = format!("{:?}", secrets(&MAC));
    assert!(shown.contains("HostAddress"), "control: the struct was shown: {shown}");
    assert!(!shown.contains(&format!("{:?}", block(MAC.a1))), "{shown}");
    assert!(!shown.contains(&format!("{:?}", block(MAC.a2))), "{shown}");
}
