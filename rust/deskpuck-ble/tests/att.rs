use deskpuck_ble::att::{Client, Event, Handles, MAX_MTU, REQUEST_TIMEOUT, uuid_le};
use deskpuck_ble::receiver::{NOTIFY_CHARACTERISTIC, SERVICE, WRITE_CHARACTERISTIC};

#[derive(Clone, Copy)]
enum Attr {
    Service([u8; 16]),
    ShortService(u16),
    Characteristic { props: u8, value: u16, uuid: [u8; 16] },
    Value,
    Descriptor(u16),
}

struct FakeJoyCon {
    attrs: Vec<(u16, Attr)>,
    mtu: u16,
    per_response: usize,
    requests: Vec<Vec<u8>>,
    cccd_writes: Vec<(u16, u16)>,
}

fn le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

fn get16(pdu: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([pdu[at], pdu[at + 1]])
}

fn other_uuid(n: u8) -> [u8; 16] {
    [n; 16]
}

fn uuid(s: &str) -> [u8; 16] {
    uuid_le(s).unwrap()
}

fn client() -> Client {
    Client::new(uuid(SERVICE), uuid(NOTIFY_CHARACTERISTIC), uuid(WRITE_CHARACTERISTIC))
}

impl FakeJoyCon {
    /// Layout a real Joy-Con 2 (R) reported: service 0x0008-0x002A, notify value 0x000A
    /// with descriptor 0x000B, write value 0x0014.
    fn real() -> Self {
        Self::at(0x0008, 0x002A)
    }

    fn at(start: u16, end: u16) -> Self {
        let o = start - 0x0008;
        let mut attrs = vec![
            (0x0001, Attr::Service(other_uuid(1))),
            (0x0002, Attr::Characteristic { props: 0x02, value: 0x0003, uuid: other_uuid(2) }),
            (0x0003, Attr::Value),
            (0x0004, Attr::Characteristic { props: 0x20, value: 0x0005, uuid: other_uuid(3) }),
            (0x0005, Attr::Value),
            (0x0006, Attr::Descriptor(0x2902)),
            (0x0007, Attr::Value),
        ];
        let service = [
            (0x0008, Attr::Service(uuid(SERVICE))),
            (
                0x0009,
                Attr::Characteristic {
                    props: 0x12,
                    value: 0x000A,
                    uuid: uuid(NOTIFY_CHARACTERISTIC),
                },
            ),
            (0x000A, Attr::Value),
            (0x000B, Attr::Descriptor(0x2902)),
            (0x000C, Attr::Descriptor(0x2901)),
            (0x000D, Attr::Characteristic { props: 0x12, value: 0x000E, uuid: other_uuid(4) }),
            (0x000E, Attr::Value),
            (0x000F, Attr::Descriptor(0x2902)),
            (
                0x0013,
                Attr::Characteristic {
                    props: 0x04,
                    value: 0x0014,
                    uuid: uuid(WRITE_CHARACTERISTIC),
                },
            ),
            (0x0014, Attr::Value),
            (0x0020, Attr::Characteristic { props: 0x0A, value: 0x0021, uuid: other_uuid(5) }),
            (0x0021, Attr::Value),
            (end - o, Attr::Value),
        ];
        attrs.extend(service.into_iter().map(|(h, a)| {
            let a = match a {
                Attr::Characteristic { props, value, uuid } => {
                    Attr::Characteristic { props, value: value + o, uuid }
                }
                a => a,
            };
            (h + o, a)
        }));
        attrs.push((end + 1, Attr::ShortService(0x180F)));
        attrs.push((
            end + 2,
            Attr::Characteristic { props: 0x02, value: end + 3, uuid: other_uuid(6) },
        ));
        attrs.push((end + 3, Attr::Value));
        Self {
            attrs,
            mtu: 512,
            per_response: usize::MAX,
            requests: Vec::new(),
            cccd_writes: Vec::new(),
        }
    }

    fn paged(mut self, per_response: usize) -> Self {
        self.per_response = per_response;
        self
    }

    fn group_end(&self, start: u16) -> u16 {
        self.attrs
            .iter()
            .find(|(h, a)| *h > start && matches!(a, Attr::Service(_) | Attr::ShortService(_)))
            .map_or(0xFFFF, |(h, _)| h - 1)
    }

    fn not_found(opcode: u8, handle: u16) -> Vec<u8> {
        let mut pdu = vec![0x01, opcode];
        pdu.extend(le(handle));
        pdu.push(0x0A);
        pdu
    }

    fn list(
        &self,
        opcode: u8,
        entries: Vec<Vec<u8>>,
        header: impl Fn(usize) -> Vec<u8>,
    ) -> Vec<u8> {
        let Some(width) = entries.first().map(Vec::len) else { unreachable!() };
        let room = (usize::from(self.mtu) - 2) / width;
        let mut pdu = vec![opcode];
        pdu.extend(header(width));
        for entry in
            entries.into_iter().take_while(|e| e.len() == width).take(room.min(self.per_response))
        {
            pdu.extend(entry);
        }
        pdu
    }

    fn answer(&mut self, request: &[u8]) -> Option<Vec<u8>> {
        self.requests.push(request.to_vec());
        match request[0] {
            0x02 => Some([vec![0x03], le(self.mtu).to_vec()].concat()),
            0x10 => {
                let (from, to) = (get16(request, 1), get16(request, 3));
                let entries: Vec<Vec<u8>> = self
                    .attrs
                    .iter()
                    .filter(|(h, _)| (from..=to).contains(h))
                    .filter_map(|&(h, a)| {
                        let uuid = match a {
                            Attr::Service(u) => u.to_vec(),
                            Attr::ShortService(u) => le(u).to_vec(),
                            _ => return None,
                        };
                        Some([le(h).to_vec(), le(self.group_end(h)).to_vec(), uuid].concat())
                    })
                    .collect();
                if entries.is_empty() {
                    return Some(Self::not_found(0x10, from));
                }
                Some(self.list(0x11, entries, |w| vec![w as u8]))
            }
            0x08 => {
                let (from, to) = (get16(request, 1), get16(request, 3));
                assert_eq!(get16(request, 5), 0x2803, "only characteristic declarations are read");
                let entries: Vec<Vec<u8>> = self
                    .attrs
                    .iter()
                    .filter(|(h, _)| (from..=to).contains(h))
                    .filter_map(|&(h, a)| match a {
                        Attr::Characteristic { props, value, uuid } => Some(
                            [le(h).to_vec(), vec![props], le(value).to_vec(), uuid.to_vec()]
                                .concat(),
                        ),
                        _ => None,
                    })
                    .collect();
                if entries.is_empty() {
                    return Some(Self::not_found(0x08, from));
                }
                Some(self.list(0x09, entries, |w| vec![w as u8]))
            }
            0x04 => {
                let (from, to) = (get16(request, 1), get16(request, 3));
                let entries: Vec<Vec<u8>> = self
                    .attrs
                    .iter()
                    .filter(|(h, _)| (from..=to).contains(h))
                    .map(|&(h, a)| {
                        let kind = match a {
                            Attr::Descriptor(k) => k,
                            Attr::Value => 0x2A00,
                            Attr::Characteristic { .. } => 0x2803,
                            Attr::Service(_) | Attr::ShortService(_) => 0x2800,
                        };
                        [le(h).to_vec(), le(kind).to_vec()].concat()
                    })
                    .collect();
                if entries.is_empty() {
                    return Some(Self::not_found(0x04, from));
                }
                Some(self.list(0x05, entries, |_| vec![1]))
            }
            0x12 => {
                self.cccd_writes.push((get16(request, 1), get16(request, 3)));
                Some(vec![0x13])
            }
            0x52 => None,
            other => panic!("unexpected request 0x{other:02X}"),
        }
    }
}

fn exchange(client: &mut Client, server: &mut FakeJoyCon, now: f64) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..1000 {
        let out = client.take_outgoing();
        if out.is_empty() {
            return events;
        }
        for pdu in out {
            if let Some(answer) = server.answer(&pdu) {
                events.extend(client.receive(&answer, now));
            }
        }
    }
    panic!("the client never went quiet");
}

fn discovered(events: &[Event]) -> Vec<(bool, bool)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Discovered { write, notify } => Some((*write, *notify)),
            _ => None,
        })
        .collect()
}

const REAL: Handles = Handles { write: Some(0x0014), notify: Some(0x000A), cccd: Some(0x000B) };

#[test]
fn uuids_go_on_the_wire_little_endian() {
    let want = [
        0xD0, 0x7F, 0xDF, 0x09, 0x8F, 0x11, 0x8F, 0x82, 0xAD, 0x49, 0xFE, 0x89, 0xBE, 0xE9, 0x7D,
        0xAB,
    ];
    assert_eq!(uuid_le(SERVICE), Some(want));
    assert_eq!(uuid_le("not a uuid"), None);
}

#[test]
fn finds_the_handles_a_real_joycon_reported() {
    for per_response in [usize::MAX, 1, 2] {
        let mut server = FakeJoyCon::real().paged(per_response);
        let mut client = client();
        client.discover(0.0);
        let events = exchange(&mut client, &mut server, 0.0);
        assert_eq!(discovered(&events), [(true, true)], "per_response {per_response}");
        assert_eq!(client.handles(), REAL, "per_response {per_response}");
        assert_eq!(client.mtu(), 512);
    }
}

#[test]
fn asks_for_the_largest_mtu_first_and_only_reads_inside_the_service() {
    let mut server = FakeJoyCon::real().paged(1);
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    assert_eq!(server.requests[0], [vec![0x02], MAX_MTU.to_le_bytes().to_vec()].concat());
    for request in &server.requests[1..] {
        let (from, to) = (get16(request, 1), get16(request, 3));
        match request[0] {
            0x10 => assert_eq!(to, 0xFFFF),
            0x08 => assert!(from >= 0x0008 && to == 0x002A, "{request:02X?}"),
            // The notify value's descriptors end before the next declaration.
            0x04 => assert!(from >= 0x000B && to == 0x000C, "{request:02X?}"),
            other => panic!("unexpected request 0x{other:02X}"),
        }
    }
}

#[test]
fn finds_the_characteristics_by_uuid_wherever_they_are() {
    let mut server = FakeJoyCon::at(0x0040, 0x0070).paged(2);
    let mut client = client();
    client.discover(0.0);
    let events = exchange(&mut client, &mut server, 0.0);
    assert_eq!(discovered(&events), [(true, true)]);
    assert_eq!(
        client.handles(),
        Handles { write: Some(0x004C), notify: Some(0x0042), cccd: Some(0x0043) }
    );
}

#[test]
fn reports_nothing_found_without_the_service() {
    let mut server = FakeJoyCon::real();
    server.attrs.retain(|(h, _)| !(0x0008..=0x002A).contains(h));
    let mut client = client();
    client.discover(0.0);
    let events = exchange(&mut client, &mut server, 0.0);
    assert_eq!(discovered(&events), [(false, false)]);
    assert!(server.requests.iter().all(|r| r[0] == 0x02 || r[0] == 0x10));
}

#[test]
fn a_notify_characteristic_without_its_descriptor_cannot_stream() {
    let mut server = FakeJoyCon::real();
    server.attrs.retain(|(h, _)| *h != 0x000B);
    let mut client = client();
    client.discover(0.0);
    let events = exchange(&mut client, &mut server, 0.0);
    assert_eq!(discovered(&events), [(true, false)]);
    // The 0x2902 at 0x000F belongs to the next characteristic.
    assert_eq!(client.handles().cccd, None);
}

#[test]
fn subscribing_writes_one_to_the_descriptor_and_commands_go_without_response() {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    assert!(client.subscribe(1.0).is_empty());
    let events = exchange(&mut client, &mut server, 1.0);
    assert_eq!(server.cccd_writes, [(0x000B, 0x0001)]);
    assert!(events.contains(&Event::Log("notifications enabled".into())));

    let command = [0x0C, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00];
    assert!(client.write(&command).is_empty());
    let sent = client.take_outgoing();
    assert_eq!(sent, [[vec![0x52, 0x14, 0x00], command.to_vec()].concat()]);
}

#[test]
fn only_the_notify_value_becomes_a_report() {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    client.subscribe(0.5);
    exchange(&mut client, &mut server, 0.5);
    let report: Vec<u8> = (0..63).collect();
    let notification = [vec![0x1B, 0x0A, 0x00], report.clone()].concat();
    assert_eq!(client.receive(&notification, 1.0), [Event::Notification(report.clone())]);
    assert!(client.receive(&[0x1B, 0x0E, 0x00, 1, 2, 3], 1.0).is_empty());
    assert!(client.receive(&[0x1B, 0x0A], 1.0).is_empty());

    // An indication is confirmed whether or not it is ours.
    let indication = [vec![0x1D, 0x0A, 0x00], report.clone()].concat();
    assert_eq!(client.receive(&indication, 1.0), [Event::Notification(report)]);
    assert!(client.receive(&[0x1D, 0x0E, 0x00, 9], 1.0).is_empty());
    assert_eq!(client.take_outgoing(), [vec![0x1E], vec![0x1E]]);
}

#[test]
fn nothing_is_reported_before_discovery_finds_the_handle() {
    let mut client = client();
    assert!(client.receive(&[0x1B, 0x0A, 0x00, 1, 2, 3], 0.0).is_empty());
    assert_eq!(client.subscribe(0.0).len(), 1, "logged");
    assert_eq!(client.write(&[1]).len(), 1, "logged");
    assert!(client.take_outgoing().is_empty());
}

#[test]
fn one_request_at_a_time() {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    client.subscribe(1.0);
    client.subscribe(1.0);
    assert_eq!(client.take_outgoing().len(), 1, "the second waits for the first");
    let events = client.receive(&[0x13], 1.1);
    assert_eq!(events, [Event::Log("notifications enabled".into())]);
    assert_eq!(client.take_outgoing(), [vec![0x12, 0x0B, 0x00, 0x01, 0x00]]);
    client.receive(&[0x13], 1.2);
    assert!(client.take_outgoing().is_empty());
    assert_eq!(client.deadline(), None);
}

#[test]
fn a_response_to_some_other_request_is_ignored() {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    let mtu_request = client.take_outgoing();
    // A stray write response and an error for another opcode change nothing.
    assert!(client.receive(&[0x13], 0.0).is_empty());
    assert!(client.receive(&[0x01, 0x10, 0x01, 0x00, 0x0A], 0.0).is_empty());
    assert!(client.take_outgoing().is_empty());
    assert_eq!(client.deadline(), Some(REQUEST_TIMEOUT));
    for pdu in mtu_request {
        let answer = server.answer(&pdu).unwrap();
        client.receive(&answer, 0.0);
    }
    let events = exchange(&mut client, &mut server, 0.0);
    assert_eq!(discovered(&events), [(true, true)]);
}

#[test]
fn an_unanswered_request_fails_the_link_after_the_timeout() {
    let mut client = client();
    client.discover(10.0);
    assert_eq!(client.take_outgoing().len(), 1);
    client.tick(10.0 + REQUEST_TIMEOUT - 0.01);
    assert_eq!(client.failed(), None);
    client.tick(10.0 + REQUEST_TIMEOUT);
    assert_eq!(client.failed(), Some("ATT request 0x02 timed out"));
    // ATT forbids further requests on a link that timed out.
    client.receive(&[0x03, 0x00, 0x02], 20.0);
    assert!(client.take_outgoing().is_empty());
}

#[test]
fn requests_from_the_joycon_are_answered() {
    let mut client = client();
    client.receive(&[0x02, 0x00, 0x01], 0.0);
    client.receive(&[0x0A, 0x03, 0x00], 0.0);
    client.receive(&[0x52, 0x03, 0x00, 1], 0.0);
    assert_eq!(
        client.take_outgoing(),
        [vec![0x03, 0x05, 0x02], vec![0x01, 0x0A, 0x00, 0x00, 0x06]],
        "MTU answered, read refused, command ignored"
    );
    assert_eq!(client.mtu(), 256);
}

#[test]
fn a_write_longer_than_the_mtu_allows_is_refused() {
    let mut server = FakeJoyCon::real();
    server.mtu = 23;
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    assert_eq!(client.mtu(), 23);
    assert!(client.write(&[0; 20]).is_empty());
    assert_eq!(client.write(&[0; 21]).len(), 1);
    assert_eq!(client.take_outgoing().len(), 1);
}

fn discovery_survives(bad: &[u8], at_opcode: u8) {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    let mut events = Vec::new();
    for _ in 0..200 {
        let out = client.take_outgoing();
        if out.is_empty() {
            break;
        }
        for pdu in out {
            let answer =
                if pdu[0] == at_opcode { bad.to_vec() } else { server.answer(&pdu).unwrap() };
            events.extend(client.receive(&answer, 0.0));
        }
    }
    assert!(client.take_outgoing().is_empty(), "still asking after 200 rounds: {bad:02X?}");
    let found = discovered(&events);
    assert_eq!(found.len(), 1, "{bad:02X?}");
    assert_ne!(found[0], (true, true), "{bad:02X?}");
}

#[test]
fn malformed_responses_end_discovery_without_looping() {
    let svc = uuid(SERVICE).to_vec();
    let service_entry =
        |start: u16, end: u16| [le(start).to_vec(), le(end).to_vec(), svc.clone()].concat();
    let services = [
        vec![0x11],
        vec![0x11, 0],
        vec![0x11, 6],
        vec![0x11, 6, 1, 0, 2, 0, 0x00],
        vec![0x11, 20, 1],
        [vec![0x11, 20], service_entry(0x0009, 0x0008)].concat(),
        // A service list that never moves forward.
        [vec![0x11, 6], le(1).to_vec(), le(1).to_vec(), le(0x1800).to_vec()].concat(),
    ];
    for bad in services {
        discovery_survives(&bad, 0x10);
    }
    let characteristics = [
        vec![0x09],
        vec![0x09, 0],
        vec![0x09, 7, 0x09, 0x00, 0x12],
        // Declarations outside the service, or values before their declaration.
        [vec![0x09, 21, 0x01, 0x00, 0x12, 0x02, 0x00], uuid(NOTIFY_CHARACTERISTIC).to_vec()]
            .concat(),
        [vec![0x09, 21, 0x09, 0x00, 0x12, 0x09, 0x00], uuid(NOTIFY_CHARACTERISTIC).to_vec()]
            .concat(),
        [vec![0x09, 21, 0x09, 0x00, 0x12, 0xFF, 0xFF], uuid(NOTIFY_CHARACTERISTIC).to_vec()]
            .concat(),
    ];
    for bad in characteristics {
        discovery_survives(&bad, 0x08);
    }
    let descriptors = [
        vec![0x05],
        vec![0x05, 3, 0x0B, 0x00, 0x02, 0x29],
        vec![0x05, 1, 0x0B, 0x00, 0x02],
        vec![0x05, 1, 0x01, 0x00, 0x02, 0x29],
        vec![0x05, 1],
    ];
    for bad in descriptors {
        discovery_survives(&bad, 0x04);
    }
}

#[test]
fn random_bytes_never_panic_or_loop() {
    // A fixed linear congruential sequence, so a failure reproduces.
    let mut seed: u32 = 0x2066_057E;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 24) as u8
    };
    for _ in 0..2000 {
        let mut client = client();
        client.discover(0.0);
        for round in 0..50 {
            let out = client.take_outgoing();
            if out.is_empty() && round > 0 {
                break;
            }
            let len = usize::from(next() % 48);
            let mut pdu: Vec<u8> = (0..len).map(|_| next()).collect();
            if let (Some(first), Some(request)) = (pdu.first_mut(), out.first()) {
                // Mostly well-typed answers, so the parsers past the opcode are reached.
                if next() % 4 != 0 {
                    *first = request[0] + 1;
                }
            }
            client.receive(&pdu, f64::from(round));
            client.tick(f64::from(round));
            let _ = client.write(&pdu);
        }
    }
}

#[test]
fn a_value_handle_must_follow_its_declaration() {
    let notify = uuid(NOTIFY_CHARACTERISTIC).to_vec();
    for value in [0x0008u16, 0x0009] {
        let mut server = FakeJoyCon::real();
        let mut client = client();
        client.discover(0.0);
        for pdu in client.take_outgoing() {
            client.receive(&server.answer(&pdu).unwrap(), 0.0);
        }
        for pdu in client.take_outgoing() {
            client.receive(&server.answer(&pdu).unwrap(), 0.0);
        }
        assert_eq!(client.take_outgoing()[0][0], 0x08, "control: characteristics are next");
        let bad = [vec![0x09, 21, 0x09, 0x00, 0x12], le(value).to_vec(), notify.clone()].concat();
        let events = client.receive(&bad, 0.0);
        assert_eq!(discovered(&events), [(false, false)], "value 0x{value:04X}");
        assert_eq!(client.handles().notify, None, "value 0x{value:04X}");
    }
}

#[test]
fn nothing_is_reported_until_notifications_are_enabled() {
    let mut server = FakeJoyCon::real();
    let mut client = client();
    client.discover(0.0);
    exchange(&mut client, &mut server, 0.0);
    let notification = [0x1B, 0x0A, 0x00, 1, 2, 3];
    assert!(client.receive(&notification, 1.0).is_empty());
    assert!(client.receive(&[0x1D, 0x0A, 0x00, 1, 2, 3], 1.0).is_empty());
    assert_eq!(client.take_outgoing(), [vec![0x1E]], "an indication is still confirmed");
    client.subscribe(1.5);
    assert_eq!(client.receive(&notification, 2.0), [Event::Notification(vec![1, 2, 3])]);
}

#[test]
fn a_device_without_the_descriptor_never_streams() {
    let mut server = FakeJoyCon::real();
    server.attrs.retain(|(h, _)| !matches!(*h, 0x000B | 0x0013 | 0x0014));
    let mut client = client();
    client.discover(0.0);
    let events = exchange(&mut client, &mut server, 0.0);
    assert_eq!(discovered(&events), [(false, false)]);
    assert_eq!(client.handles().notify, Some(0x000A), "control: the value handle is known");
    assert_eq!(client.subscribe(1.0).len(), 1, "refused and logged");
    assert!(client.receive(&[0x1B, 0x0A, 0x00, 1, 2, 3], 2.0).is_empty());
    assert!(client.receive(&[0x1D, 0x0A, 0x00, 1, 2, 3], 2.0).is_empty());
}
