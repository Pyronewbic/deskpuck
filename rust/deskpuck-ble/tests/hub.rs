mod common;

use common::capture::{self, MAC};
use deskpuck_ble::controller::{Hooks, Hub, LinkStatus, PairingRequest};
use deskpuck_ble::receiver::{Input, MANUFACTURER_ID, Output};
use deskpuck_ble::vendor::{Addressee, expected_answer};
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::packet::{Report, encode_report};
use deskpuck_core::pairing::{HostAddress, PairedDevice};
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

fn hub() -> (Hub<u32, RecordingSink>, Recorder) {
    let dir = tempfile::tempdir().unwrap();
    save_pairing(&dir.path().join("pairing.json"), JOYCON);
    hub_in(dir)
}

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
        addressee: Addressee::Unknown,
    }
}

fn notification(packet_id: u32, buttons: u32) -> Input<u32> {
    let report =
        Report { packet_id, buttons, left_stick_y: 2047, right_stick_y: 2047, ..Report::default() };
    Input::Notification { id: JOYCON, data: encode_report(&report) }
}

fn connected() -> (Hub<u32, RecordingSink>, Recorder) {
    let (mut hub, rec) = hub();
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    hub.input(discovered(), 0.5);
    hub.input(Input::Connected(JOYCON), 1.0);
    hub.input(
        Input::CharacteristicsFound { id: JOYCON, write: true, notify: true, response: false },
        1.0,
    );
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
    assert!(hub.set_paused(false, 3.0).is_empty());
    assert!(rec.errors.lock().unwrap().is_empty());
}

#[test]
fn resume_while_searching_scans_again() {
    let (mut hub, _) = hub();
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    assert_eq!(hub.set_paused(true, 1.0), [Output::StopScan]);
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
    hub.start_pairing(1.0, PairingRequest::default());
    hub.input(
        Input::Discovered {
            id: NEW,
            name: Some("New\x07Pad".into()),
            manufacturer_ids: vec![MANUFACTURER_ID],
            addressee: Addressee::Unknown,
        },
        2.0,
    );
    hub.input(Input::Connected(NEW), 3.0);
    hub.input(
        Input::CharacteristicsFound { id: NEW, write: true, notify: true, response: false },
        4.0,
    );
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
    let new = Input::Discovered {
        id: NEW,
        name: None,
        manufacturer_ids: vec![MANUFACTURER_ID],
        addressee: Addressee::Unknown,
    };
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
    assert_eq!(hub.start_pairing(1.0, PairingRequest::default()), [Output::StartScan]);
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
    hub.set_paused(true, 3.0);
    assert_eq!(*rec.latched.lock().unwrap(), [Modifiers::SHIFT, Modifiers::NONE]);
    hub.shutdown();
    assert_eq!(rec.latched.lock().unwrap().len(), 2, "no change, no call");
}

fn written(out: &[Output<u32>]) -> Vec<u8> {
    let writes: Vec<&Vec<u8>> = out
        .iter()
        .filter_map(|o| match o {
            Output::Write { data, .. } => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(writes.len(), 1, "{out:?}");
    writes[0].clone()
}

fn mac_host() -> HostAddress {
    HostAddress::parse(MAC.host).unwrap()
}

/// Pairs NEW with button reconnect, answering as a real Joy-Con would for whatever blocks the
/// hub drew; returns the A1 it sent.
fn vendor_pair(hub: &mut Hub<u32, RecordingSink>) -> [u8; 16] {
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    hub.start_pairing(1.0, PairingRequest { button_reconnect: true, host: Some(mac_host()) });
    let new = Input::Discovered {
        id: NEW,
        name: None,
        manufacturer_ids: vec![MANUFACTURER_ID],
        addressee: Addressee::Unknown,
    };
    hub.input(new, 2.0);
    hub.input(Input::Connected(NEW), 3.0);
    let found = Input::CharacteristicsFound { id: NEW, write: true, notify: true, response: true };
    assert_eq!(hub.input(found, 4.0), [Output::SubscribeResponse(NEW)]);
    let reply = |data: Vec<u8>| Input::Response { id: NEW, data };

    let address = written(&hub.input(Input::ResponseSubscribed(NEW), 4.1));
    assert_eq!(address, capture::request(&MAC, 0), "the host address goes out as captured");
    let key = written(&hub.input(reply(capture::answer(&MAC, 0)), 4.2));
    assert_eq!(key[..9], capture::request(&MAC, 1)[..9]);
    let a1: [u8; 16] = key[9..25].try_into().unwrap();
    let challenge = written(&hub.input(reply(capture::answer(&MAC, 1)), 4.3));
    let a2: [u8; 16] = challenge[9..25].try_into().unwrap();
    let b2 = expected_answer(&a1, &capture::block(capture::B1), &a2);
    let answer = [&[0x15, 0x01, 0x01, 0x02, 0x10, 0x78, 0x00, 0x00, 0x01][..], &b2].concat();
    assert_eq!(written(&hub.input(reply(answer), 4.4)), capture::request(&MAC, 3));
    hub.input(reply(capture::answer(&MAC, 3)), 4.5);
    a1
}

#[test]
fn a_vendor_pairing_saves_the_host_and_draws_fresh_blocks_each_time() {
    let (mut hub, rec) = hub_in(tempfile::tempdir().unwrap());
    let first = vendor_pair(&mut hub);
    let saved = PairedDevice::load(&rec.pairing_file()).unwrap().expect("saved");
    assert_eq!(saved, PairedDevice::new("9", None).unwrap().with_host(Some(mac_host())));
    assert!(rec.errors.lock().unwrap().is_empty(), "{:?}", rec.errors.lock().unwrap());

    let (mut again, _) = hub_in(tempfile::tempdir().unwrap());
    let second = vendor_pair(&mut again);
    assert_ne!(first, second, "A1 is random, never fixed");
    assert_ne!(first, [0; 16]);
}

#[test]
fn button_reconnect_needs_the_setting_and_a_known_host() {
    let (mut hub, rec) = hub_in(tempfile::tempdir().unwrap());
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    hub.start_pairing(1.0, PairingRequest { button_reconnect: true, host: None });
    let errors = rec.errors.lock().unwrap().clone();
    assert!(errors.len() == 1 && errors[0].contains("hold SYNC"), "{errors:?}");

    for request in [
        PairingRequest { button_reconnect: true, host: None },
        PairingRequest { button_reconnect: false, host: Some(mac_host()) },
    ] {
        let (mut hub, rec) = hub_in(tempfile::tempdir().unwrap());
        hub.start(0.0);
        hub.input(Input::AdapterPoweredOn, 0.0);
        hub.start_pairing(1.0, request);
        let new = Input::Discovered {
            id: NEW,
            name: None,
            manufacturer_ids: vec![MANUFACTURER_ID],
            addressee: Addressee::Unknown,
        };
        hub.input(new, 2.0);
        hub.input(Input::Connected(NEW), 3.0);
        let found =
            Input::CharacteristicsFound { id: NEW, write: true, notify: true, response: true };
        let out = hub.input(found, 4.0);
        assert!(!out.contains(&Output::SubscribeResponse(NEW)), "{request:?}");
        let saved = PairedDevice::load(&rec.pairing_file()).unwrap().expect("plain pairing saved");
        assert_eq!(saved.host, None, "{request:?}");
    }
}

#[test]
fn the_saved_host_decides_which_reconnect_adverts_connect() {
    let dir = tempfile::tempdir().unwrap();
    let device = PairedDevice::new(&JOYCON.to_string(), None).unwrap().with_host(Some(mac_host()));
    device.save(&dir.path().join("pairing.json")).unwrap();
    let (mut hub, _) = hub_in(dir);
    hub.start(0.0);
    hub.input(Input::AdapterPoweredOn, 0.0);
    let advert = |addressee| Input::Discovered {
        id: JOYCON,
        name: None,
        manufacturer_ids: vec![MANUFACTURER_ID],
        addressee,
    };
    let other = HostAddress::parse(capture::ALLY.host).unwrap();
    assert!(hub.input(advert(Addressee::Host(other)), 1.0).is_empty());
    let out = hub.input(advert(Addressee::Host(mac_host())), 1.0);
    assert_eq!(out, [Output::StopScan, Output::Connect(JOYCON)]);
}
