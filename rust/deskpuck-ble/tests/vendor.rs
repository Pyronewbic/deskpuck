use deskpuck_ble::vendor::*;
use deskpuck_core::pairing::HostAddress;

fn bytes(hex: &str) -> Vec<u8> {
    hex.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
}

fn block(hex: &str) -> [u8; 16] {
    bytes(hex).try_into().unwrap()
}

/// One pairing as captured from a real Joy-Con 2 (R): what was sent, then what it answered.
struct Transcript {
    host: &'static str,
    a1: &'static str,
    a2: &'static str,
    exchange: [(&'static str, &'static str); 4],
}

const B1: &str = "5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10";

const MAC: Transcript = Transcript {
    host: "FC:B2:14:D1:5C:A1",
    a1: "ba 0c 9d f9 fe 94 23 5c 37 1b cc 80 d6 70 d3 fa",
    a2: "ce 08 c4 59 5f 25 19 b3 77 f6 97 60 4b c1 21 4a",
    exchange: [
        (
            "15 91 01 01 00 0e 00 00 00 02 a1 5c d1 14 b2 fc a0 5c d1 14 b2 fc",
            "15 01 01 01 10 78 00 00 01 04 01 72 2b 2a bf ef e0",
        ),
        (
            "15 91 01 04 00 11 00 00 00 ba 0c 9d f9 fe 94 23 5c 37 1b cc 80 d6 70 d3 fa",
            "15 01 01 04 10 78 00 00 01 5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10",
        ),
        (
            "15 91 01 02 00 11 00 00 00 ce 08 c4 59 5f 25 19 b3 77 f6 97 60 4b c1 21 4a",
            "15 01 01 02 10 78 00 00 01 23 3b 63 d7 3f 7d bd e1 54 61 4a c6 d1 01 78 fd",
        ),
        ("15 91 01 03 00 01 00 00 00", "15 01 01 03 10 78 00 00 01"),
    ],
};

const ALLY: Transcript = Transcript {
    host: "14:AC:60:2B:4C:EA",
    a1: "0e cb b2 1c 14 3d f8 6a 65 e7 b6 4f 24 43 e9 61",
    a2: "52 d7 f0 e9 c9 21 55 14 6e 41 11 6a 12 fb 1f 0a",
    exchange: [
        (
            "15 91 01 01 00 0e 00 00 00 02 ea 4c 2b 60 ac 14 e9 4c 2b 60 ac 14",
            "15 01 01 01 10 78 00 00 01 04 01 72 2b 2a bf ef e0",
        ),
        (
            "15 91 01 04 00 11 00 00 00 0e cb b2 1c 14 3d f8 6a 65 e7 b6 4f 24 43 e9 61",
            "15 01 01 04 10 78 00 00 01 5c f6 ee 79 2c df 05 e1 ba 2b 63 25 c4 1a 5f 10",
        ),
        (
            "15 91 01 02 00 11 00 00 00 52 d7 f0 e9 c9 21 55 14 6e 41 11 6a 12 fb 1f 0a",
            "15 01 01 02 10 78 00 00 01 c4 d3 8a 1a 2f 27 ab d2 59 52 e6 6a a4 e0 f8 c7",
        ),
        ("15 91 01 03 00 01 00 00 00", "15 01 01 03 10 78 00 00 01"),
    ],
};

fn secrets(t: &Transcript) -> Secrets {
    Secrets { host: HostAddress::parse(t.host).unwrap(), a1: block(t.a1), a2: block(t.a2) }
}

fn answer(t: &Transcript, step: usize) -> Vec<u8> {
    bytes(t.exchange[step].1)
}

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
