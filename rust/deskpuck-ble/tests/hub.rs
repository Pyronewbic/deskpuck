use deskpuck_ble::controller::{Hooks, Hub, LinkStatus};
use deskpuck_ble::receiver::{Input, MANUFACTURER_ID, Output};
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::packet::{Report, encode_report};
use deskpuck_core::pairing::PairedDevice;
use deskpuck_inject::{InputEvent, RecordingSink};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const JOYCON: u32 = 7;
const RS: u32 = 0x0004_0000;

type Seen<T> = Arc<Mutex<Vec<T>>>;

struct Recorder {
    statuses: Seen<(LinkStatus, Option<String>)>,
    reports: Seen<(u32, Option<String>, u128)>,
    errors: Seen<String>,
    latched: Seen<Modifiers>,
    /// Holds the pairing file's directory for the test's lifetime.
    dir: tempfile::TempDir,
}

impl Recorder {
    fn pairing_file(&self) -> PathBuf {
        self.dir.path().join("pairing.json")
    }
}

fn save_pairing(path: &Path, id: u32) {
    PairedDevice::new(&id.to_string(), Some("Joy-Con 2 (R)")).unwrap().save(path).unwrap();
}

/// Paired with JOYCON.
fn hub() -> (Hub<u32, RecordingSink>, Recorder) {
    let dir = tempfile::tempdir().unwrap();
    save_pairing(&dir.path().join("pairing.json"), JOYCON);
    hub_in(dir)
}

/// Uses whatever pairing.json `dir` holds.
fn hub_in(dir: tempfile::TempDir) -> (Hub<u32, RecordingSink>, Recorder) {
    let rec = Recorder {
        statuses: Arc::default(),
        reports: Arc::default(),
        errors: Arc::default(),
        latched: Arc::default(),
        dir,
    };
    let (s, r, e, l) =
        (rec.statuses.clone(), rec.reports.clone(), rec.errors.clone(), rec.latched.clone());
    let hooks = Hooks {
        status: Box::new(move |status, name| {
            s.lock().unwrap().push((status, name.map(str::to_owned)))
        }),
        report: Some(Box::new(move |report, _, name, ms| {
            r.lock().unwrap().push((report.packet_id, name.map(str::to_owned), ms))
        })),
        log: None,
        error: Box::new(move |m| e.lock().unwrap().push(m.to_owned())),
        latched: Some(Box::new(move |m| l.lock().unwrap().push(m))),
    };
    let file = Some(rec.pairing_file());
    (Hub::new(EngineSettings::default(), RecordingSink::default(), hooks, file), rec)
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
    assert_eq!(hub.input(discovered(), 0.5), [Output::StopScan, Output::Connect(JOYCON)]);
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

const NEW: u32 = 9;

fn pair_new(hub: &mut Hub<u32, RecordingSink>) {
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    hub.start_pairing(1.0);
    hub.input(
        Input::Discovered {
            id: NEW,
            name: Some("New\x07Pad".into()),
            manufacturer_ids: vec![MANUFACTURER_ID],
        },
        2.0,
    );
    hub.input(Input::Connected(NEW), 3.0);
    hub.input(Input::CharacteristicsFound { id: NEW, write: true, notify: true }, 4.0);
}

#[test]
fn pairing_is_saved_and_survives_a_restart() {
    let (mut hub, rec) = hub_in(tempfile::tempdir().unwrap());
    assert_eq!(hub.paired(), None);
    pair_new(&mut hub);
    let statuses: Vec<LinkStatus> = rec.statuses.lock().unwrap().iter().map(|s| s.0).collect();
    assert_eq!(
        statuses,
        [LinkStatus::NotPaired, LinkStatus::Pairing, LinkStatus::Connecting, LinkStatus::Connected]
    );
    let saved = PairedDevice::load(&rec.pairing_file()).unwrap().expect("saved");
    assert_eq!(saved, PairedDevice::new("9", Some("NewPad")).unwrap());
    assert!(rec.errors.lock().unwrap().is_empty());

    // A new Hub on the same file connects to NEW and only NEW.
    let (mut again, rec2) = hub_in(rec.dir);
    assert_eq!(again.paired(), Some("9"));
    again.start(0.0);
    assert_eq!(again.input(Input::AdapterPoweredOn, 0.0), [Output::StartScan]);
    assert!(again.input(discovered(), 1.0).is_empty(), "the old Joy-Con is not paired");
    let new = Input::Discovered { id: NEW, name: None, manufacturer_ids: vec![MANUFACTURER_ID] };
    assert_eq!(again.input(new, 1.0), [Output::StopScan, Output::Connect(NEW)]);
    assert_eq!(rec2.statuses.lock().unwrap()[0], (LinkStatus::Searching, None));
}

#[test]
fn a_malformed_pairing_file_is_reported_and_connects_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pairing.json"), r#"{"version": 1, "id": "7", "x": 1}"#)
        .unwrap();
    let (mut hub, rec) = hub_in(dir);
    assert_eq!(hub.paired(), None);
    let errors = rec.errors.lock().unwrap().clone();
    assert!(errors.len() == 1 && errors[0].contains("pair the Joy-Con again"), "{errors:?}");
    hub.start(0.0);
    assert!(hub.input(Input::AdapterPoweredOn, 0.0).is_empty(), "no scan");
    assert!(hub.input(discovered(), 1.0).is_empty());
    assert_eq!(rec.statuses.lock().unwrap()[0], (LinkStatus::NotPaired, None));
}

#[test]
fn a_failed_save_is_reported_but_the_pairing_holds_until_quit() {
    let dir = tempfile::tempdir().unwrap();
    // A directory where the file should be: loading and saving both fail.
    std::fs::create_dir(dir.path().join("pairing.json")).unwrap();
    let (mut hub, rec) = hub_in(dir);
    pair_new(&mut hub);
    let errors = rec.errors.lock().unwrap().clone();
    assert!(errors.last().is_some_and(|e| e.contains("Could not save the pairing")), "{errors:?}");
    assert_eq!(hub.paired(), Some("9"));
}

#[test]
fn without_a_file_pairing_lasts_for_the_hub() {
    let hooks = Hooks {
        status: Box::new(|_, _| {}),
        report: None,
        log: None,
        error: Box::new(|m| panic!("unexpected error: {m}")),
        latched: None,
    };
    let mut hub = Hub::new(EngineSettings::default(), RecordingSink::default(), hooks, None);
    pair_new(&mut hub);
    assert_eq!(hub.paired(), Some("9"));
}

#[test]
fn cancel_pairing_goes_through_the_hub() {
    let (mut hub, rec) = hub_in(tempfile::tempdir().unwrap());
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    assert_eq!(hub.start_pairing(1.0), [Output::StartScan]);
    assert_eq!(hub.cancel_pairing(2.0), [Output::StopScan]);
    assert_eq!(rec.statuses.lock().unwrap().last(), Some(&(LinkStatus::NotPaired, None)));
}

#[test]
fn latch_changes_reach_the_hook_once_each() {
    let (mut hub, rec) = connected();
    hub.apply_settings(EngineSettings {
        key_mappings: vec![deskpuck_core::mapping::ButtonKeyMapping::modifier(
            RS,
            Modifiers::SHIFT,
            true,
        )],
        ..EngineSettings::default()
    });
    hub.input(notification(1, RS), 2.0);
    hub.input(notification(2, 0), 2.1);
    hub.input(notification(3, 0), 2.2);
    assert_eq!(*rec.latched.lock().unwrap(), [Modifiers::SHIFT]);
    // Pausing releases it, and the hook hears that too.
    hub.set_paused(true, 3.0);
    assert_eq!(*rec.latched.lock().unwrap(), [Modifiers::SHIFT, Modifiers::NONE]);
    hub.shutdown();
    assert_eq!(rec.latched.lock().unwrap().len(), 2, "no change, no call");
}
