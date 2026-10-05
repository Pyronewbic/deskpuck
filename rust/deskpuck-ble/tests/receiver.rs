use deskpuck_ble::receiver::*;
use deskpuck_core::packet::{REPORT_MIN_SIZE, Report, encode_report};

type R = Receiver<u32>;

const JOYCON: u32 = 7;
const OTHER: u32 = 8;

fn discovered(id: u32) -> Input<u32> {
    Input::Discovered {
        id,
        name: Some("Joy-Con 2 (R)".into()),
        manufacturer_ids: vec![MANUFACTURER_ID],
    }
}

fn scans(out: &[Output<u32>]) -> usize {
    out.iter().filter(|o| **o == Output::StartScan).count()
}

fn has(out: &[Output<u32>], want: &Output<u32>) -> bool {
    out.contains(want)
}

/// Powered on and scanning, optionally suspended.
fn ready(suspended: bool) -> R {
    let mut r = R::default();
    r.handle(Input::AdapterPoweredOn, 0.0);
    r.start();
    r.set_suspended(suspended);
    r
}

/// Connected to JOYCON at t=1 with characteristics found at t=2.
fn linked() -> R {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.5);
    r.handle(Input::Connected(JOYCON), 1.0);
    r.handle(Input::CharacteristicsFound { id: JOYCON, write: true, notify: true }, 2.0);
    r
}

fn report_bytes(packet_id: u32) -> Vec<u8> {
    encode_report(&Report { packet_id, ..Report::default() })
}

#[test]
fn start_scans_only_once_bluetooth_is_on() {
    let mut r = R::default();
    assert_eq!(scans(&r.start()), 0);
    let out = r.handle(Input::AdapterPoweredOn, 0.0);
    assert_eq!(scans(&out), 1);
    assert!(has(&out, &Output::Status { status: Status::Searching, name: None }));
}

#[test]
fn suspended_blocks_every_scan_path() {
    // Each path runs on a running and a suspended receiver; the running one is
    // the positive control proving the path really scans.
    for suspended in [false, true] {
        let expect = usize::from(!suspended);

        let mut r = R::default();
        r.set_suspended(suspended);
        r.start();
        assert_eq!(
            scans(&r.handle(Input::AdapterPoweredOn, 0.0)),
            expect,
            "power on, suspended={suspended}"
        );

        let mut r = ready(false);
        r.handle(discovered(JOYCON), 0.0);
        r.set_suspended(suspended);
        r.handle(Input::ConnectFailed(JOYCON), 1.0);
        assert_eq!(
            scans(&r.tick(1.0 + RESCAN_AFTER_FAILURE)),
            expect,
            "failed connect, suspended={suspended}"
        );

        let mut r = linked();
        r.set_suspended(suspended);
        r.handle(Input::Disconnected(JOYCON), 5.0);
        assert_eq!(
            scans(&r.tick(5.0 + RESCAN_AFTER_DISCONNECT)),
            expect,
            "disconnect, suspended={suspended}"
        );

        let mut r = ready(false);
        r.handle(discovered(JOYCON), 0.0);
        r.set_suspended(suspended);
        r.tick(CONNECT_TIMEOUT);
        assert_eq!(
            scans(&r.tick(CONNECT_TIMEOUT + RESCAN_AFTER_FAILURE)),
            expect,
            "connect timeout, suspended={suspended}"
        );
    }
}

#[test]
fn discovery_while_suspended_does_not_connect() {
    let mut running = ready(false);
    assert!(has(&running.handle(discovered(JOYCON), 0.0), &Output::Connect(JOYCON)));
    let mut paused = ready(true);
    assert!(paused.handle(discovered(JOYCON), 0.0).is_empty());
}

#[test]
fn pause_and_resume() {
    let mut r = ready(false);
    assert_eq!(r.set_suspended(true), [Output::StopScan]);
    // Resuming with nothing connected scans again.
    assert_eq!(scans(&r.set_suspended(false)), 1);

    // Resuming while connected leaves the link alone.
    let mut r = linked();
    r.set_suspended(true);
    assert_eq!(scans(&r.set_suspended(false)), 0);

    // A receiver that never wanted a scan does not start one on resume.
    let mut idle = R::default();
    idle.handle(Input::AdapterPoweredOn, 0.0);
    idle.set_suspended(true);
    assert_eq!(scans(&idle.set_suspended(false)), 0);
}

#[test]
fn only_joycons_are_connected_one_at_a_time() {
    let mut r = ready(false);
    let not_nintendo = Input::Discovered { id: OTHER, name: None, manufacturer_ids: vec![0x004C] };
    assert!(r.handle(not_nintendo, 0.0).is_empty());
    assert!(
        r.handle(Input::Discovered { id: OTHER, name: None, manufacturer_ids: vec![] }, 0.0)
            .is_empty()
    );

    let out = r.handle(discovered(JOYCON), 0.0);
    assert!(has(&out, &Output::Connect(JOYCON)));
    assert!(has(
        &out,
        &Output::Status { status: Status::Connecting, name: Some("Joy-Con 2 (R)".into()) }
    ));
    // A second Joy-Con (or a repeat sighting) while connecting is ignored.
    assert!(r.handle(discovered(OTHER), 0.1).is_empty());
    assert!(r.handle(discovered(JOYCON), 0.2).is_empty());
}

#[test]
fn connect_then_discover_services() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.0);
    let out = r.handle(Input::Connected(JOYCON), 1.0);
    assert!(has(
        &out,
        &Output::Status { status: Status::Connected, name: Some("Joy-Con 2 (R)".into()) }
    ));
    assert!(has(&out, &Output::DiscoverServices(JOYCON)));
    assert_eq!(r.linked().map(|(id, _)| *id), Some(JOYCON));

    // A connection we did not ask for is dropped.
    let mut r = ready(false);
    assert_eq!(r.handle(Input::Connected(OTHER), 0.0), [Output::Disconnect(OTHER)]);
}

#[test]
fn connect_timeout_cancels_and_rescans() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.0);
    assert!(r.tick(CONNECT_TIMEOUT - 0.01).is_empty());
    let out = r.tick(CONNECT_TIMEOUT);
    assert!(has(&out, &Output::Disconnect(JOYCON)));
    assert!(has(&out, &Output::Status { status: Status::Searching, name: None }));
    assert_eq!(scans(&r.tick(CONNECT_TIMEOUT + RESCAN_AFTER_FAILURE - 0.01)), 0);
    assert_eq!(scans(&r.tick(CONNECT_TIMEOUT + RESCAN_AFTER_FAILURE)), 1);
    // The late connect after the timeout is let go, not adopted.
    assert_eq!(r.handle(Input::Connected(JOYCON), 70.0), [Output::Disconnect(JOYCON)]);
}

#[test]
fn rescan_delays_match_the_mac_app() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.0);
    r.handle(Input::ConnectFailed(JOYCON), 10.0);
    assert_eq!(scans(&r.tick(10.0 + RESCAN_AFTER_FAILURE - 0.01)), 0);
    assert_eq!(scans(&r.tick(10.0 + RESCAN_AFTER_FAILURE)), 1);

    let mut r = linked();
    let out = r.handle(Input::Disconnected(JOYCON), 20.0);
    assert!(has(&out, &Output::Status { status: Status::Searching, name: None }));
    assert!(r.linked().is_none());
    assert_eq!(scans(&r.tick(20.0 + RESCAN_AFTER_DISCONNECT - 0.01)), 0);
    assert_eq!(scans(&r.tick(20.0 + RESCAN_AFTER_DISCONNECT)), 1);
}

#[test]
fn init_sequence_timing() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.0);
    r.handle(Input::Connected(JOYCON), 1.0);
    let out = r.handle(Input::CharacteristicsFound { id: JOYCON, write: true, notify: true }, 2.0);
    assert_eq!(out, [Output::Subscribe(JOYCON)]);

    assert!(r.tick(2.0 + INIT_DELAY - 0.01).is_empty());
    let first = r.tick(2.0 + INIT_DELAY);
    assert_eq!(first, [Output::Write { id: JOYCON, data: INIT_COMMANDS[0].to_vec() }]);
    let second = r.tick(2.0 + INIT_DELAY + INIT_SPACING);
    assert_eq!(second, [Output::Write { id: JOYCON, data: INIT_COMMANDS[1].to_vec() }]);
    assert_eq!(r.tick(2.0 + RESUBSCRIBE_DELAY), [Output::Subscribe(JOYCON)]);
    // Each step happens once.
    assert!(r.tick(10.0).is_empty());
}

#[test]
fn missing_characteristics_send_nothing() {
    for (write, notify) in [(false, true), (true, false), (false, false)] {
        let mut r = ready(false);
        r.handle(discovered(JOYCON), 0.0);
        r.handle(Input::Connected(JOYCON), 1.0);
        assert!(
            r.handle(Input::CharacteristicsFound { id: JOYCON, write, notify }, 2.0).is_empty()
        );
        assert!(r.tick(5.0).is_empty(), "write={write} notify={notify}");
    }
}

#[test]
fn init_commands_match_the_cpp_receiver() {
    // Read the bytes from the shipping receiver so the two cannot drift apart.
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../Sources/DeskpuckCore/Joycon2BLEReceiver.mm"
    ))
    .expect("Joycon2BLEReceiver.mm readable");
    let start = source.find("- (void)sendInitializationCommandsOnce").expect("init method");
    let body = &source[start..start + source[start..].find("\n}").expect("method end")];
    let commands: Vec<Vec<u8>> = body
        .split("(uint8_t[]){")
        .skip(1)
        .map(|rest| {
            rest.split('}')
                .next()
                .unwrap_or_default()
                .split(',')
                .map(|b| {
                    u8::from_str_radix(b.trim().trim_start_matches("0x"), 16).expect("hex byte")
                })
                .collect()
        })
        .collect();
    assert_eq!(commands.len(), 2, "found {} commands", commands.len());
    assert_eq!(commands, INIT_COMMANDS.map(|c| c.to_vec()));
}

#[test]
fn reports_flow_and_short_ones_are_dropped() {
    let mut r = linked();
    let out = r.handle(Input::Notification { id: JOYCON, data: report_bytes(42) }, 3.0);
    assert!(matches!(&out[..], [Output::Report { report, .. }] if report.packet_id == 42));

    assert!(
        r.handle(Input::Notification { id: JOYCON, data: vec![0; REPORT_MIN_SIZE - 1] }, 3.1)
            .is_empty()
    );
    assert!(r.handle(Input::Notification { id: JOYCON, data: vec![] }, 3.2).is_empty());
    // Data from a peripheral we are not linked to is ignored.
    assert!(r.handle(Input::Notification { id: OTHER, data: report_bytes(1) }, 3.3).is_empty());
}

#[test]
fn data_timeout_disconnects_once() {
    let mut r = linked();
    assert!(!has(&r.tick(1.0 + DATA_TIMEOUT - 0.01), &Output::Disconnect(JOYCON)));
    assert!(has(&r.tick(1.0 + DATA_TIMEOUT), &Output::Disconnect(JOYCON)));
    assert!(
        !has(&r.tick(1.0 + DATA_TIMEOUT + 5.0), &Output::Disconnect(JOYCON)),
        "repeated disconnect"
    );
}

#[test]
fn reports_keep_the_link_alive_but_short_ones_do_not() {
    let mut r = linked();
    r.handle(Input::Notification { id: JOYCON, data: report_bytes(1) }, 25.0);
    // Positive control for the reset: no timeout at the original deadline.
    assert!(!has(&r.tick(1.0 + DATA_TIMEOUT), &Output::Disconnect(JOYCON)));
    // Only short reports since: they do not count, so it times out from 25 s.
    r.handle(Input::Notification { id: JOYCON, data: vec![0; 10] }, 50.0);
    assert!(has(&r.tick(25.0 + DATA_TIMEOUT), &Output::Disconnect(JOYCON)));
}

#[test]
fn bluetooth_off_and_back_on() {
    let mut r = ready(false);
    assert_eq!(
        r.handle(Input::AdapterPoweredOff, 1.0),
        [Output::Status { status: Status::BluetoothOff, name: None }]
    );
    assert_eq!(scans(&r.tick(100.0)), 0);
    assert_eq!(scans(&r.handle(Input::AdapterPoweredOn, 101.0)), 1);
}
