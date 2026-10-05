use deskpuck_ble::controller::{Hooks, Hub, LinkStatus};
use deskpuck_ble::receiver::{Input, MANUFACTURER_ID, Output};
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::packet::{Report, encode_report};
use deskpuck_inject::{InputEvent, RecordingSink};
use std::sync::{Arc, Mutex};

const JOYCON: u32 = 7;
const RS: u32 = 0x0004_0000;

type Seen<T> = Arc<Mutex<Vec<T>>>;

struct Recorder {
    statuses: Seen<(LinkStatus, Option<String>)>,
    reports: Seen<(u32, Option<String>, u128)>,
    errors: Seen<String>,
}

fn hub() -> (Hub<u32, RecordingSink>, Recorder) {
    let rec =
        Recorder { statuses: Arc::default(), reports: Arc::default(), errors: Arc::default() };
    let (s, r, e) = (rec.statuses.clone(), rec.reports.clone(), rec.errors.clone());
    let hooks = Hooks {
        status: Box::new(move |status, name| {
            s.lock().unwrap().push((status, name.map(str::to_owned)))
        }),
        report: Some(Box::new(move |report, _, name, ms| {
            r.lock().unwrap().push((report.packet_id, name.map(str::to_owned), ms))
        })),
        log: None,
        error: Box::new(move |m| e.lock().unwrap().push(m.to_owned())),
    };
    (Hub::new(EngineSettings::default(), RecordingSink::default(), hooks), rec)
}

fn discovered() -> Input<u32> {
    Input::Discovered {
        id: JOYCON,
        name: Some("Joy-Con 2 (R)".into()),
        manufacturer_ids: vec![MANUFACTURER_ID],
    }
}

fn notification(packet_id: u32, buttons: u32) -> Input<u32> {
    let report =
        Report { packet_id, buttons, left_stick_y: 2047, right_stick_y: 2047, ..Report::default() };
    Input::Notification { id: JOYCON, data: encode_report(&report) }
}

/// Powered on, connected at t=1, characteristics found.
fn connected() -> (Hub<u32, RecordingSink>, Recorder) {
    let (mut hub, rec) = hub();
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    hub.input(discovered(), 0.5);
    hub.input(Input::Connected(JOYCON), 1.0);
    hub.input(Input::CharacteristicsFound { id: JOYCON, write: true, notify: true }, 1.0);
    (hub, rec)
}

fn key(down: bool) -> InputEvent {
    InputEvent::Key { key_code: 36, down, repeat: false }
}

#[test]
fn statuses_go_to_the_hook_and_commands_come_back() {
    let (mut hub, rec) = hub();
    assert!(hub.start(0.0).is_empty(), "no scan before Bluetooth is on");
    assert_eq!(hub.input(Input::AdapterPoweredOn, 0.0), [Output::StartScan]);
    assert_eq!(hub.input(discovered(), 0.5), [Output::Connect(JOYCON)]);
    assert_eq!(hub.input(Input::Connected(JOYCON), 1.0), [Output::DiscoverServices(JOYCON)]);
    let name = Some("Joy-Con 2 (R)".to_owned());
    assert_eq!(
        *rec.statuses.lock().unwrap(),
        [
            (LinkStatus::Searching, None),
            (LinkStatus::Connecting, name.clone()),
            (LinkStatus::Connected, name)
        ]
    );
}

#[test]
fn reports_reach_the_hook_and_the_pointer() {
    let (mut hub, rec) = connected();
    assert!(hub.input(notification(5, RS), 3.5).is_empty());
    assert_eq!(*rec.reports.lock().unwrap(), [(5, Some("Joy-Con 2 (R)".to_owned()), 2500)]);
    assert_eq!(hub.session().sink().events, [key(true)]);
}

#[test]
fn pause_stops_scanning_and_input_but_not_the_readout() {
    let (mut hub, rec) = connected();
    hub.input(notification(1, RS), 2.0);
    assert_eq!(hub.set_paused(true, 2.1), [Output::StopScan]);
    hub.input(notification(2, RS), 2.2);
    // The monitor still sees reports while paused; the pointer and keys do not.
    assert_eq!(rec.reports.lock().unwrap().len(), 2);
    assert_eq!(hub.session().sink().events, [key(true), key(false)]);
    // Connected, so resuming does not need a scan.
    assert!(hub.set_paused(false, 3.0).is_empty());
    assert!(rec.errors.lock().unwrap().is_empty());
}

#[test]
fn resume_while_searching_scans_again() {
    let (mut hub, _) = hub();
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    assert_eq!(hub.set_paused(true, 1.0), [Output::StopScan]);
    // A Joy-Con seen while paused is not connected.
    assert!(hub.input(discovered(), 1.5).is_empty());
    assert_eq!(hub.set_paused(false, 2.0), [Output::StartScan]);
}

#[test]
fn disconnect_shutdown_and_external_statuses() {
    let (mut hub, rec) = connected();
    hub.input(notification(1, RS), 2.0);
    hub.input(Input::Disconnected(JOYCON), 3.0);
    assert_eq!(hub.session().sink().events, [key(true), key(false)]);
    assert_eq!(rec.statuses.lock().unwrap().last(), Some(&(LinkStatus::Searching, None)));

    let (mut hub, rec) = connected();
    hub.input(notification(1, RS), 2.0);
    hub.shutdown();
    assert_eq!(hub.session().sink().events, [key(true), key(false)]);
    assert_eq!(hub.linked(), Some(&JOYCON), "the caller disconnects the linked Joy-Con");
    hub.report_status(LinkStatus::BluetoothUnauthorized);
    assert_eq!(
        rec.statuses.lock().unwrap().last(),
        Some(&(LinkStatus::BluetoothUnauthorized, None))
    );
}

#[test]
fn settings_apply_live() {
    let (mut hub, _) = connected();
    hub.input(notification(1, RS), 2.0);
    let mut settings = EngineSettings::default();
    settings.key_mappings.retain(|m| m.button_mask != RS);
    hub.apply_settings(settings);
    hub.input(notification(2, RS), 2.1);
    // Return is released by the change and not pressed again under the new mapping.
    assert_eq!(hub.session().sink().events, [key(true), key(false)]);
}
