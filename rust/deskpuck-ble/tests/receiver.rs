mod common;

use common::capture::{self, ALLY, MAC};
use deskpuck_ble::receiver::*;
use deskpuck_core::packet::{REPORT_MIN_SIZE, Report, encode_report};
use deskpuck_core::pairing::HostAddress;

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

fn paired() -> R {
    let mut r = R::default();
    r.set_paired(Some(JOYCON.to_string()));
    r
}

fn ready(suspended: bool) -> R {
    let mut r = paired();
    r.handle(Input::AdapterPoweredOn, 0.0);
    r.start();
    r.set_suspended(suspended);
    r
}

fn linked() -> R {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.5);
    r.handle(Input::Connected(JOYCON), 1.0);
    r.handle(
        Input::CharacteristicsFound { id: JOYCON, write: true, notify: true, response: false },
        2.0,
    );
    r
}

fn report_bytes(packet_id: u32) -> Vec<u8> {
    encode_report(&Report { packet_id, ..Report::default() })
}

#[test]
fn start_scans_only_once_bluetooth_is_on() {
    let mut r = paired();
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

        let mut r = paired();
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
    assert_eq!(scans(&r.set_suspended(false)), 1);

    let mut r = linked();
    r.set_suspended(true);
    assert_eq!(scans(&r.set_suspended(false)), 0);

    let mut idle = paired();
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
    let out = r.handle(
        Input::CharacteristicsFound { id: JOYCON, write: true, notify: true, response: false },
        2.0,
    );
    assert_eq!(out, [Output::Subscribe(JOYCON)]);

    assert!(r.tick(2.0 + INIT_DELAY - 0.01).is_empty());
    let first = r.tick(2.0 + INIT_DELAY);
    assert_eq!(first, [Output::Write { id: JOYCON, data: INIT_COMMANDS[0].to_vec() }]);
    let second = r.tick(2.0 + INIT_DELAY + INIT_SPACING);
    assert_eq!(second, [Output::Write { id: JOYCON, data: INIT_COMMANDS[1].to_vec() }]);
    let third = r.tick(2.0 + INIT_DELAY + 2.0 * INIT_SPACING);
    assert_eq!(third, [Output::Write { id: JOYCON, data: PLAYER_ONE_LED.to_vec() }]);
    assert_eq!(r.tick(2.0 + RESUBSCRIBE_DELAY), [Output::Subscribe(JOYCON)]);
    assert!(r.tick(10.0).is_empty());
}

#[test]
fn missing_characteristics_send_nothing() {
    for (write, notify) in [(false, true), (true, false), (false, false)] {
        let mut r = ready(false);
        r.handle(discovered(JOYCON), 0.0);
        r.handle(Input::Connected(JOYCON), 1.0);
        assert!(
            r.handle(
                Input::CharacteristicsFound { id: JOYCON, write, notify, response: false },
                2.0
            )
            .is_empty()
        );
        assert!(r.tick(5.0).is_empty(), "write={write} notify={notify}");
    }
}

#[test]
fn an_unconfirmed_link_injects_nothing_and_times_out() {
    let reports_until_timeout = |found: Option<(bool, bool)>| {
        // A pairing window accepts any device; both characteristics confirm it.
        let mut r = unpaired();
        r.start_pairing(0.0, None);
        r.handle(discovered(OTHER), 0.5);
        r.handle(Input::Connected(OTHER), 1.0);
        if let Some((write, notify)) = found {
            r.handle(
                Input::CharacteristicsFound { id: OTHER, write, notify, response: false },
                2.0,
            );
        }
        let reports = (3..30)
            .flat_map(|t| {
                r.handle(Input::Notification { id: OTHER, data: report_bytes(t) }, t as f64)
            })
            .filter(|o| matches!(o, Output::Report { .. }))
            .count();
        let dropped = has(&r.tick(2.0 + DATA_TIMEOUT), &Output::Disconnect(OTHER));
        (reports, dropped, r.paired().is_some())
    };
    for found in [None, Some((false, true)), Some((true, false)), Some((false, false))] {
        assert_eq!(reports_until_timeout(found), (0, true, false), "{found:?}");
    }
    assert_eq!(reports_until_timeout(Some((true, true))), (27, false, true), "control");
}

#[test]
fn init_commands_are_the_ones_the_joycon_accepts() {
    // As sent to real L and R Joy-Con 2s that then streamed reports (deskpuck-cli --verbose).
    let accepted: [&[u8]; 2] = [
        &[0x0C, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
        &[0x0C, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
    ];
    assert_eq!(INIT_COMMANDS[..2], accepted);
    // As a Switch 2 sends it (ndeadly/switch2_controller_research,
    // bluetooth_interface.md, "Set player LEDs"); last, after the controls work.
    let player_one: &[u8] = &[
        0x09, 0x91, 0x01, 0x07, 0x00, 0x08, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    assert_eq!(INIT_COMMANDS[2], player_one);
    assert_eq!(INIT_COMMANDS.len(), 3);
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

#[test]
fn quick_reconnect_cancels_the_pending_rescan() {
    let mut r = linked();
    r.handle(Input::Disconnected(JOYCON), 10.0);
    r.handle(discovered(JOYCON), 11.0);
    r.handle(Input::Connected(JOYCON), 11.5);
    assert_eq!(scans(&r.tick(10.0 + RESCAN_AFTER_DISCONNECT)), 0);

    // Positive control: without the reconnect, the same tick rescans.
    let mut r = linked();
    r.handle(Input::Disconnected(JOYCON), 10.0);
    assert_eq!(scans(&r.tick(10.0 + RESCAN_AFTER_DISCONNECT)), 1);
}

#[test]
fn control_characters_are_removed_from_device_names() {
    for (advertised, shown) in [
        ("\x1b]0;evil\x07Joy-Con\r\n", Some("]0;evilJoy-Con")),
        ("\x1b\x07\0", None),
        ("Joy-Con 2 (R)", Some("Joy-Con 2 (R)")),
    ] {
        let mut r = ready(false);
        let out = r.handle(
            Input::Discovered {
                id: JOYCON,
                name: Some(advertised.into()),
                manufacturer_ids: vec![MANUFACTURER_ID],
            },
            0.5,
        );
        let want = Output::Status { status: Status::Connecting, name: shown.map(Into::into) };
        assert!(has(&out, &want), "{advertised:?}: {out:?}");
        r.handle(Input::Connected(JOYCON), 1.0);
        r.handle(
            Input::CharacteristicsFound { id: JOYCON, write: true, notify: true, response: false },
            2.0,
        );
        assert_eq!(r.linked().map(|(_, n)| n.map(str::to_owned)), Some(shown.map(Into::into)));
    }
}

fn status(out: &[Output<u32>]) -> Option<Status> {
    out.iter().rev().find_map(|o| match o {
        Output::Status { status, .. } => Some(*status),
        _ => None,
    })
}

fn paired_output(out: &[Output<u32>]) -> Option<u32> {
    out.iter().find_map(|o| match o {
        Output::Paired { id, .. } => Some(*id),
        _ => None,
    })
}

fn unpaired() -> R {
    let mut r = R::default();
    r.start();
    r.handle(Input::AdapterPoweredOn, 0.0);
    r
}

#[test]
fn unpaired_neither_scans_nor_connects() {
    let mut r = R::default();
    r.start();
    let out = r.handle(Input::AdapterPoweredOn, 0.0);
    assert_eq!((status(&out), scans(&out)), (Some(Status::NotPaired), 0));
    assert!(r.handle(discovered(JOYCON), 1.0).is_empty());
    // Positive control: the same discovery connects once JOYCON is paired.
    assert!(has(&ready(false).handle(discovered(JOYCON), 1.0), &Output::Connect(JOYCON)));
}

#[test]
fn outside_a_window_only_the_paired_joycon_connects() {
    let mut r = ready(false);
    assert!(r.handle(discovered(OTHER), 1.0).is_empty());
    assert!(has(&r.handle(discovered(JOYCON), 1.0), &Output::Connect(JOYCON)));
}

#[test]
fn pairing_window_pairs_the_first_joycon_and_remembers_it() {
    let mut r = unpaired();
    let out = r.start_pairing(10.0, None);
    assert_eq!((status(&out), scans(&out)), (Some(Status::Pairing), 1));
    assert!(r.is_pairing());

    assert!(has(&r.handle(discovered(OTHER), 20.0), &Output::Connect(OTHER)));
    r.handle(Input::Connected(OTHER), 21.0);
    assert_eq!(r.paired(), None);
    let out = r.handle(
        Input::CharacteristicsFound { id: OTHER, write: true, notify: true, response: false },
        22.0,
    );
    assert_eq!(paired_output(&out), Some(OTHER));
    assert!(has(&out, &Output::Subscribe(OTHER)));
    assert_eq!((r.paired(), r.is_pairing()), (Some("8"), false));

    let out = r.handle(Input::Disconnected(OTHER), 30.0);
    assert_eq!(status(&out), Some(Status::Searching));
    assert_eq!(scans(&r.tick(30.0 + RESCAN_AFTER_DISCONNECT)), 1);
    assert!(r.handle(discovered(JOYCON), 34.0).is_empty(), "the old id is no longer paired");
    assert!(has(&r.handle(discovered(OTHER), 34.0), &Output::Connect(OTHER)));
    r.handle(Input::Connected(OTHER), 35.0);
    let out = r.handle(
        Input::CharacteristicsFound { id: OTHER, write: true, notify: true, response: false },
        36.0,
    );
    assert_eq!(paired_output(&out), None, "a reconnect is not a new pairing");
}

#[test]
fn a_link_without_the_characteristics_is_not_paired() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), 1.0);
    r.handle(Input::Connected(OTHER), 2.0);
    let out = r.handle(
        Input::CharacteristicsFound { id: OTHER, write: true, notify: false, response: false },
        3.0,
    );
    assert_eq!((paired_output(&out), r.paired()), (None, None));
    assert!(r.is_pairing(), "the window stays open for a real Joy-Con");
}

#[test]
fn pairing_window_expires() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    assert!(has(&r.handle(discovered(OTHER), PAIRING_WINDOW - 0.01), &Output::Connect(OTHER)));
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    assert!(r.tick(PAIRING_WINDOW - 0.01).is_empty());
    assert!(r.handle(discovered(OTHER), PAIRING_WINDOW).is_empty(), "window is half-open");
    let out = r.tick(PAIRING_WINDOW);
    assert_eq!(status(&out), Some(Status::NotPaired));
    assert!(has(&out, &Output::StopScan));
    assert!(!r.is_pairing());

    let mut r = ready(false);
    r.start_pairing(0.0, None);
    let out = r.tick(PAIRING_WINDOW);
    assert_eq!(status(&out), Some(Status::Searching));
    assert!(!has(&out, &Output::StopScan));
    assert!(r.handle(discovered(OTHER), PAIRING_WINDOW + 1.0).is_empty());
}

#[test]
fn a_joycon_accepted_in_the_window_may_finish_after_it() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), PAIRING_WINDOW - 1.0);
    let out = r.tick(PAIRING_WINDOW);
    assert_eq!(status(&out), None, "still connecting, so no idle status");
    r.handle(Input::Connected(OTHER), PAIRING_WINDOW + 1.0);
    let out = r.handle(
        Input::CharacteristicsFound { id: OTHER, write: true, notify: true, response: false },
        PAIRING_WINDOW + 2.0,
    );
    assert_eq!(paired_output(&out), Some(OTHER));
}

#[test]
fn starting_a_window_lets_go_of_the_linked_joycon() {
    let mut r = linked();
    let out = r.start_pairing(5.0, None);
    assert!(has(&out, &Output::Disconnect(JOYCON)));
    assert_eq!(status(&out), Some(Status::Pairing));
    assert_eq!(r.linked(), None);
    assert!(r.handle(Input::Disconnected(JOYCON), 5.5).is_empty());
    assert!(has(&r.handle(discovered(OTHER), 6.0), &Output::Connect(OTHER)));

    let mut r = ready(false);
    r.handle(discovered(JOYCON), 1.0);
    assert!(has(&r.start_pairing(2.0, None), &Output::Disconnect(JOYCON)));
    assert!(has(&r.handle(Input::Connected(JOYCON), 3.0), &Output::Disconnect(JOYCON)));
}

#[test]
fn cancel_pairing() {
    assert!(unpaired().cancel_pairing().is_empty(), "no window, nothing to cancel");

    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), 1.0);
    let out = r.cancel_pairing();
    assert!(has(&out, &Output::Disconnect(OTHER)));
    assert!(has(&out, &Output::StopScan));
    assert_eq!(status(&out), Some(Status::NotPaired));
    assert!(has(&r.handle(Input::Connected(OTHER), 2.0), &Output::Disconnect(OTHER)));

    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), 1.0);
    r.handle(Input::Connected(OTHER), 2.0);
    assert!(has(&r.cancel_pairing(), &Output::Disconnect(OTHER)));
    assert_eq!(r.linked(), None);

    let mut r = ready(false);
    r.start_pairing(0.0, None);
    let out = r.cancel_pairing();
    assert_eq!((status(&out), scans(&out)), (Some(Status::Searching), 1));
    assert!(!has(&out, &Output::StopScan));
}

#[test]
fn suspend_blocks_pairing_too() {
    let mut r = unpaired();
    r.set_suspended(true);
    assert!(r.start_pairing(0.0, None).is_empty());
    assert!(!r.is_pairing());

    let mut r = unpaired();
    r.start_pairing(0.0, None);
    assert_eq!(r.set_suspended(true), [Output::StopScan]);
    assert!(r.handle(discovered(OTHER), 1.0).is_empty());
    assert_eq!(scans(&r.tick(1.0)), 0);
    // Positive control: resuming inside the window scans and connects again.
    assert_eq!(scans(&r.set_suspended(false)), 1);
    assert!(has(&r.handle(discovered(OTHER), 2.0), &Output::Connect(OTHER)));
}

#[test]
fn a_failed_connect_inside_the_window_keeps_pairing() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), 1.0);
    assert_eq!(status(&r.handle(Input::ConnectFailed(OTHER), 2.0)), Some(Status::Pairing));
    assert_eq!(scans(&r.tick(2.0 + RESCAN_AFTER_FAILURE)), 1);
}

#[test]
fn side_comes_from_the_product_id_in_the_advertisement() {
    // Captured from real Joy-Con 2s (deskpuck-cli --verbose).
    let mut left = vec![0x01, 0x00, 0x03, 0x7E, 0x05, 0x67, 0x20, 0x00, 0x01];
    left.extend([0; 7]);
    left.extend([0x0F, 0, 0, 0, 0, 0, 0, 0]);
    let mut right = left.clone();
    right[5] = 0x66;
    assert_eq!(name_from_manufacturer_data(&left), Some("Joy-Con 2 (L)"));
    assert_eq!(name_from_manufacturer_data(&right), Some("Joy-Con 2 (R)"));

    let mut other_vendor = right.clone();
    other_vendor[3] = 0x7F;
    let mut other_product = right.clone();
    other_product[5] = 0x69;
    for (data, why) in [
        (&other_vendor[..], "not Nintendo"),
        (&other_product[..], "not a Joy-Con 2"),
        (&right[..6], "too short for the product id"),
        (&[][..], "empty"),
    ] {
        assert_eq!(name_from_manufacturer_data(data), None, "{why}");
    }
}

#[test]
fn a_joycon_held_by_another_program_shows_as_in_use() {
    let mut r = ready(false);
    assert!(r.wants_presence_check());
    assert!(r.handle(Input::SystemConnected(vec![OTHER]), 1.0).is_empty(), "not ours");
    let out = r.handle(Input::SystemConnected(vec![OTHER, JOYCON]), 2.0);
    assert_eq!(status(&out), Some(Status::InUseElsewhere));
    assert!(r.handle(Input::SystemConnected(vec![JOYCON]), 3.0).is_empty(), "no repeat");
    assert_eq!(status(&r.handle(Input::SystemConnected(vec![]), 4.0)), Some(Status::Searching));
    r.handle(Input::SystemConnected(vec![JOYCON]), 5.0);
    assert!(has(&r.handle(discovered(JOYCON), 6.0), &Output::Connect(JOYCON)));
    r.handle(Input::Connected(JOYCON), 7.0);
    // Connecting cleared the stale flag, so a later drop searches again.
    let out = r.handle(Input::Disconnected(JOYCON), 8.0);
    assert_eq!(status(&out), Some(Status::Searching));
}

#[test]
fn presence_is_only_checked_while_searching_for_the_paired_joycon() {
    let suspended = ready(true);
    let mut pairing = ready(false);
    pairing.start_pairing(0.0, None);
    let mut connecting = ready(false);
    connecting.handle(discovered(JOYCON), 0.0);
    let mut off = paired();
    off.start();
    let mut cases = [
        ("linked", linked()),
        ("suspended", suspended),
        ("pairing", pairing),
        ("unpaired", unpaired()),
    ];
    for (why, r) in cases.iter_mut() {
        assert!(!r.wants_presence_check(), "{why}");
        let out = r.handle(Input::SystemConnected(vec![JOYCON]), 1.0);
        assert_eq!(status(&out), None, "{why}");
    }
    assert!(!connecting.wants_presence_check() && !off.wants_presence_check());
    // Positive control: the same input on a searching receiver changes the status.
    assert!(ready(false).wants_presence_check());
}

#[test]
fn starting_a_window_clears_in_use() {
    let mut r = ready(false);
    r.handle(Input::SystemConnected(vec![JOYCON]), 1.0);
    assert_eq!(status(&r.start_pairing(2.0, None)), Some(Status::Pairing));
    assert_eq!(status(&r.cancel_pairing()), Some(Status::Searching));
}

#[test]
fn cancel_drops_a_joycon_still_finishing_after_the_window() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), PAIRING_WINDOW - 0.1);
    r.tick(PAIRING_WINDOW + 1.0);
    assert!(!r.is_pairing(), "the window has expired");
    let out = r.cancel_pairing();
    assert!(has(&out, &Output::Disconnect(OTHER)), "{out:?}");
    assert_eq!(status(&out), Some(Status::NotPaired));
    assert!(has(
        &r.handle(Input::Connected(OTHER), PAIRING_WINDOW + 2.0),
        &Output::Disconnect(OTHER)
    ));
    assert_eq!(r.paired(), None);
    // Positive control: with nothing in flight, a cancel after expiry does nothing.
    let mut idle = unpaired();
    idle.start_pairing(0.0, None);
    idle.tick(PAIRING_WINDOW);
    assert!(idle.cancel_pairing().is_empty());
}

#[test]
fn pausing_drops_an_unconfirmed_pairing() {
    for confirm_step in [false, true] {
        let mut r = unpaired();
        r.start_pairing(0.0, None);
        r.handle(discovered(OTHER), 1.0);
        if confirm_step {
            r.handle(Input::Connected(OTHER), 2.0);
        }
        let out = r.set_suspended(true);
        assert!(has(&out, &Output::Disconnect(OTHER)), "connected={confirm_step}: {out:?}");
        assert_eq!(status(&out), Some(Status::Pairing), "the window is paused, not closed");
        r.handle(Input::Connected(OTHER), 3.0);
        let late = r.handle(
            Input::CharacteristicsFound { id: OTHER, write: true, notify: true, response: false },
            4.0,
        );
        assert_eq!((paired_output(&late), r.paired()), (None, None), "connected={confirm_step}");
    }
    let mut r = linked();
    assert_eq!(r.set_suspended(true), [Output::StopScan]);
    assert!(r.linked().is_some());
}

#[test]
fn advertised_names_are_cleaned_and_capped() {
    let mut r = ready(false);
    let long = format!("Joy\u{202E}Con\u{200B}{}", "x".repeat(300));
    let out = r.handle(
        Input::Discovered { id: JOYCON, name: Some(long), manufacturer_ids: vec![MANUFACTURER_ID] },
        1.0,
    );
    let shown = out.iter().find_map(|o| match o {
        Output::Status { name, .. } => name.clone(),
        _ => None,
    });
    let shown = shown.expect("a name");
    assert!(shown.starts_with("JoyConx"), "{shown:?}");
    assert_eq!(shown.chars().count(), deskpuck_core::pairing::MAX_NAME_CHARS);
}

#[test]
fn scanning_stops_before_a_connect() {
    let mut r = ready(false);
    let out = r.handle(discovered(JOYCON), 0.0);
    let stop = out.iter().position(|o| *o == Output::StopScan).expect("the scan is stopped");
    let connect = out.iter().position(|o| *o == Output::Connect(JOYCON)).expect("a connect");
    assert!(stop < connect, "{out:?}");
}

#[test]
fn a_failed_connect_is_cancelled_so_the_link_cannot_linger() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.0);
    assert!(has(&r.handle(Input::ConnectFailed(JOYCON), 1.0), &Output::Disconnect(JOYCON)));
    assert!(r.handle(Input::ConnectFailed(OTHER), 2.0).is_empty());
}

#[test]
fn the_model_name_wins_over_an_address_shaped_name() {
    let right = [0x01, 0x00, 0x03, 0x7E, 0x05, 0x66, 0x20, 0x00];
    let bluez_alias = Some("E0-EF-BF-2A-2B-72".to_owned());
    assert_eq!(device_name(bluez_alias.clone(), Some(&right)).as_deref(), Some("Joy-Con 2 (R)"));
    let unknown = [0x01, 0x00, 0x03, 0x7E, 0x05, 0x69, 0x20, 0x00];
    assert_eq!(device_name(bluez_alias.clone(), Some(&unknown)), bluez_alias);
    assert_eq!(device_name(Some("Joy-Con 2 (R)".into()), None).as_deref(), Some("Joy-Con 2 (R)"));
    assert_eq!(device_name(None, None), None);
}

fn writes(out: &[Output<u32>]) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Output::Write { data, .. } => Some(data.clone()),
            _ => None,
        })
        .collect()
}

fn paired_host(out: &[Output<u32>]) -> Option<Option<HostAddress>> {
    out.iter().find_map(|o| match o {
        Output::Paired { host, .. } => Some(*host),
        _ => None,
    })
}

fn found(id: u32, response: bool) -> Input<u32> {
    Input::CharacteristicsFound { id, write: true, notify: true, response }
}

fn reply(t: &capture::Transcript, step: usize) -> Input<u32> {
    Input::Response { id: OTHER, data: capture::answer(t, step) }
}

/// A pairing window offering vendor pairing to MAC's host, with OTHER connected and its
/// characteristics, the reply one included, just found at 3.0.
fn vendor_pairing() -> (R, Vec<Output<u32>>) {
    let mut r = unpaired();
    r.start_pairing(0.0, Some(capture::secrets(&MAC)));
    r.handle(discovered(OTHER), 1.0);
    r.handle(Input::Connected(OTHER), 2.0);
    let out = r.handle(found(OTHER, true), 3.0);
    (r, out)
}

#[test]
fn vendor_pairing_replays_a_real_exchange_then_streams_as_before() {
    let (mut r, out) = vendor_pairing();
    assert_eq!(out, [Output::SubscribeResponse(OTHER)], "nothing is sent before replies are on");
    assert_eq!(r.paired(), None);

    let out = r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    assert_eq!(writes(&out), [capture::request(&MAC, 0)]);
    for step in 0..3 {
        let out = r.handle(reply(&MAC, step), 3.2 + step as f64 * 0.1);
        assert_eq!(writes(&out), [capture::request(&MAC, step + 1)], "step {step}");
        assert_eq!(paired_output(&out), None, "step {step}");
    }

    let out = r.handle(reply(&MAC, 3), 3.6);
    assert_eq!(paired_host(&out), Some(Some(HostAddress::parse(MAC.host).unwrap())));
    assert!(has(&out, &Output::Subscribe(OTHER)));
    assert_eq!((r.paired(), r.is_pairing()), (Some("8"), false));

    let init = r.tick(3.6 + INIT_DELAY + 2.0 * INIT_SPACING);
    assert_eq!(writes(&init), INIT_COMMANDS.map(<[u8]>::to_vec), "then the usual init");
    let report = Input::Notification { id: OTHER, data: report_bytes(1) };
    assert!(matches!(r.handle(report, 4.0)[..], [Output::Report { .. }]));
}

#[test]
fn input_is_ignored_while_vendor_pairing_runs() {
    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    let report = Input::Notification { id: OTHER, data: report_bytes(1) };
    assert!(r.handle(report, 3.2).is_empty());
}

#[test]
fn a_wrong_challenge_answer_never_commits_and_falls_back_to_sync_pairing() {
    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    r.handle(reply(&MAC, 0), 3.2);
    r.handle(reply(&MAC, 1), 3.3);
    let out = r.handle(reply(&ALLY, 2), 3.4);
    assert!(writes(&out).iter().all(|w| w[..4] != [0x15, 0x91, 0x01, 0x03]), "{out:?}");
    assert_eq!(paired_host(&out), Some(None), "paired, without the button-press reconnect");
    assert!(out.iter().any(|o| matches!(o, Output::Log(m) if m.contains("answer"))), "{out:?}");
    assert!(has(&out, &Output::Subscribe(OTHER)), "input still works");
    assert_eq!(r.paired(), Some("8"));
}

#[test]
fn a_refused_or_malformed_reply_falls_back() {
    for step in 0..4 {
        let (mut r, _) = vendor_pairing();
        r.handle(Input::ResponseSubscribed(OTHER), 3.1);
        for i in 0..step {
            r.handle(reply(&MAC, i), 3.2);
        }
        let mut refused = capture::answer(&MAC, step);
        refused[1] = 0x00;
        let out = r.handle(Input::Response { id: OTHER, data: refused }, 3.5);
        assert_eq!(paired_host(&out), Some(None), "step {step}");
    }
}

#[test]
fn replies_for_another_step_another_joycon_or_before_subscribing_are_ignored() {
    let (mut r, _) = vendor_pairing();
    assert!(r.handle(reply(&MAC, 0), 3.05).is_empty(), "replies are not on yet");
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    assert!(r.handle(Input::ResponseSubscribed(OTHER), 3.15).is_empty(), "sent once");
    assert!(r.handle(reply(&MAC, 2), 3.2).is_empty());
    let stranger = Input::Response { id: JOYCON, data: capture::answer(&MAC, 0) };
    assert!(r.handle(stranger, 3.2).is_empty());
    assert!(r.handle(Input::Response { id: OTHER, data: vec![] }, 3.2).is_empty());
    assert_eq!(writes(&r.handle(reply(&MAC, 0), 3.3)), [capture::request(&MAC, 1)], "control");
}

#[test]
fn a_silent_joycon_times_out_into_sync_pairing() {
    // No acknowledgement of the subscription at all.
    let (mut r, _) = vendor_pairing();
    assert!(r.tick(3.0 + VENDOR_STEP_TIMEOUT - 0.01).is_empty());
    let out = r.tick(3.0 + VENDOR_STEP_TIMEOUT);
    assert_eq!(paired_host(&out), Some(None));
    assert!(has(&out, &Output::Subscribe(OTHER)));

    // A step with no reply; each reply restarts the clock.
    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    r.handle(reply(&MAC, 0), 3.1 + VENDOR_STEP_TIMEOUT - 0.1);
    assert_eq!(paired_host(&r.tick(3.1 + VENDOR_STEP_TIMEOUT + 0.5)), None, "clock restarted");
    let out = r.tick(3.1 + 2.0 * VENDOR_STEP_TIMEOUT);
    assert_eq!(paired_host(&out), Some(None));
    assert!(out.iter().any(|o| matches!(o, Output::Log(m) if m.contains("no reply"))), "{out:?}");
}

#[test]
fn without_an_offer_or_a_reply_characteristic_pairing_is_as_before() {
    let mut r = unpaired();
    r.start_pairing(0.0, None);
    r.handle(discovered(OTHER), 1.0);
    r.handle(Input::Connected(OTHER), 2.0);
    let out = r.handle(found(OTHER, true), 3.0);
    assert_eq!(paired_host(&out), Some(None));
    assert!(!has(&out, &Output::SubscribeResponse(OTHER)));

    let mut r = unpaired();
    r.start_pairing(0.0, Some(capture::secrets(&MAC)));
    r.handle(discovered(OTHER), 1.0);
    r.handle(Input::Connected(OTHER), 2.0);
    let out = r.handle(found(OTHER, false), 3.0);
    assert_eq!(paired_host(&out), Some(None));
    assert!(out.iter().any(|o| matches!(o, Output::Log(_))), "says why: {out:?}");
}

#[test]
fn a_reconnect_never_vendor_pairs() {
    let mut r = ready(false);
    r.handle(discovered(JOYCON), 0.5);
    r.handle(Input::Connected(JOYCON), 1.0);
    let out = r.handle(found(JOYCON, true), 2.0);
    assert!(!has(&out, &Output::SubscribeResponse(JOYCON)));
    assert_eq!(paired_output(&out), None);
}

#[test]
fn an_offer_is_spent_by_one_pairing_and_dropped_with_the_window() {
    let (mut r, _) = vendor_pairing();
    r.tick(3.0 + VENDOR_STEP_TIMEOUT);
    // A second window without an offer must not reuse the first one's blocks.
    r.start_pairing(10.0, None);
    r.handle(discovered(OTHER), 11.0);
    r.handle(Input::Connected(OTHER), 12.0);
    assert!(!has(&r.handle(found(OTHER, true), 13.0), &Output::SubscribeResponse(OTHER)));

    let mut r = unpaired();
    r.start_pairing(0.0, Some(capture::secrets(&MAC)));
    r.cancel_pairing();
    r.start_pairing(1.0, None);
    r.handle(discovered(OTHER), 2.0);
    r.handle(Input::Connected(OTHER), 3.0);
    assert!(!has(&r.handle(found(OTHER, true), 4.0), &Output::SubscribeResponse(OTHER)));
}

#[test]
fn cancelling_pausing_or_losing_the_link_mid_pairing_pairs_nothing() {
    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    assert!(has(&r.cancel_pairing(), &Output::Disconnect(OTHER)));
    assert_eq!(paired_output(&r.handle(reply(&MAC, 0), 3.2)), None);
    assert_eq!(r.paired(), None);

    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    assert!(has(&r.set_suspended(true), &Output::Disconnect(OTHER)));
    assert_eq!(r.paired(), None);

    let (mut r, _) = vendor_pairing();
    r.handle(Input::ResponseSubscribed(OTHER), 3.1);
    r.handle(Input::Disconnected(OTHER), 3.2);
    assert_eq!(paired_output(&r.tick(3.0 + VENDOR_STEP_TIMEOUT)), None);
    assert_eq!(r.paired(), None);
}

#[test]
fn a_pairing_that_outlives_its_window_still_finishes() {
    let mut r = unpaired();
    r.start_pairing(0.0, Some(capture::secrets(&MAC)));
    r.handle(discovered(OTHER), PAIRING_WINDOW - 2.0);
    r.handle(Input::Connected(OTHER), PAIRING_WINDOW - 1.5);
    r.handle(found(OTHER, true), PAIRING_WINDOW - 1.0);
    r.handle(Input::ResponseSubscribed(OTHER), PAIRING_WINDOW - 0.9);
    r.tick(PAIRING_WINDOW);
    for step in 0..3 {
        r.handle(reply(&MAC, step), PAIRING_WINDOW + 0.1);
    }
    let out = r.handle(reply(&MAC, 3), PAIRING_WINDOW + 0.2);
    assert_eq!(paired_host(&out), Some(Some(HostAddress::parse(MAC.host).unwrap())));
}
