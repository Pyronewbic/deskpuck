//! A minimal ATT client with no I/O, for links where the OS's GATT layer
//! cannot be used: feed it received PDUs and the current time, send the PDUs
//! it queues. It finds the Joy-Con's characteristics by UUID (never by fixed
//! handles), enables notifications and writes commands. Everything it reads
//! comes from the radio, so every length and handle is checked.

use std::collections::VecDeque;

pub const ERROR_RESPONSE: u8 = 0x01;
pub const EXCHANGE_MTU_REQUEST: u8 = 0x02;
pub const EXCHANGE_MTU_RESPONSE: u8 = 0x03;
pub const FIND_INFORMATION_REQUEST: u8 = 0x04;
pub const FIND_INFORMATION_RESPONSE: u8 = 0x05;
pub const READ_BY_TYPE_REQUEST: u8 = 0x08;
pub const READ_BY_TYPE_RESPONSE: u8 = 0x09;
pub const READ_BY_GROUP_TYPE_REQUEST: u8 = 0x10;
pub const READ_BY_GROUP_TYPE_RESPONSE: u8 = 0x11;
pub const WRITE_REQUEST: u8 = 0x12;
pub const WRITE_RESPONSE: u8 = 0x13;
pub const HANDLE_VALUE_NOTIFICATION: u8 = 0x1B;
pub const HANDLE_VALUE_INDICATION: u8 = 0x1D;
pub const HANDLE_VALUE_CONFIRMATION: u8 = 0x1E;
pub const WRITE_COMMAND: u8 = 0x52;

const ATTRIBUTE_NOT_FOUND: u8 = 0x0A;
const REQUEST_NOT_SUPPORTED: u8 = 0x06;
const PRIMARY_SERVICE: u16 = 0x2800;
const CHARACTERISTIC: u16 = 0x2803;
const CLIENT_CHARACTERISTIC_CONFIGURATION: u16 = 0x2902;
/// The default before an MTU exchange, and the largest ATT allows.
pub const DEFAULT_MTU: u16 = 23;
pub const MAX_MTU: u16 = 517;
/// A server that does not answer a request within this many seconds is gone.
pub const REQUEST_TIMEOUT: f64 = 3.0;

/// Requests a server can send; a client that cannot serve them must refuse
/// each one, or the server may wait on it.
const SERVER_REQUESTS: [u8; 11] =
    [0x04, 0x06, 0x08, 0x0A, 0x0C, 0x0E, 0x10, 0x12, 0x16, 0x18, 0x20];

/// A UUID string in the little-endian byte order ATT uses on the wire.
pub fn uuid_le(uuid: &str) -> Option<[u8; 16]> {
    let mut bytes = *uuid::Uuid::parse_str(uuid).ok()?.as_bytes();
    bytes.reverse();
    Some(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Discovery ended; which characteristics were found.
    Discovered { write: bool, notify: bool },
    /// A value from the notify characteristic.
    Notification(Vec<u8>),
    /// Worth a line in the verbose log.
    Log(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Handles {
    pub write: Option<u16>,
    pub notify: Option<u16>,
    pub cccd: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Idle,
    Mtu,
    Services { from: u16 },
    Characteristics { from: u16, end: u16 },
    Descriptors { from: u16, end: u16 },
    Done,
}

#[derive(Clone, Copy, Debug)]
struct Declaration {
    handle: u16,
    value: u16,
}

pub struct Client {
    service: [u8; 16],
    notify_uuid: [u8; 16],
    write_uuid: [u8; 16],
    mtu: u16,
    step: Step,
    /// The request in flight: its opcode and when it times out. ATT allows one.
    waiting: Option<(u8, f64)>,
    queued: VecDeque<Vec<u8>>,
    outgoing: Vec<Vec<u8>>,
    handles: Handles,
    /// The characteristic declaration after the notify one, which bounds its descriptors.
    notify_declaration: Option<Declaration>,
    next_declaration: Option<u16>,
    failed: Option<String>,
    /// Values are passed on only once notifications were asked for, as a
    /// GATT stack would, so a device that never passed discovery sends nothing.
    subscribed: bool,
}

impl Client {
    pub fn new(service: [u8; 16], notify: [u8; 16], write: [u8; 16]) -> Self {
        Self {
            service,
            notify_uuid: notify,
            write_uuid: write,
            mtu: DEFAULT_MTU,
            step: Step::Idle,
            waiting: None,
            queued: VecDeque::new(),
            outgoing: Vec::new(),
            handles: Handles::default(),
            notify_declaration: None,
            next_declaration: None,
            failed: None,
            subscribed: false,
        }
    }

    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    pub fn handles(&self) -> Handles {
        self.handles
    }

    /// PDUs to send now, in order.
    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing)
    }

    /// When the request in flight times out, if one is.
    pub fn deadline(&self) -> Option<f64> {
        self.waiting.map(|(_, at)| at)
    }

    /// Why the link is unusable: a request timed out. No further request may
    /// be sent on it, so the caller closes the link.
    pub fn failed(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Exchanges the MTU, then finds the service and its characteristics.
    pub fn discover(&mut self, now: f64) {
        if self.step != Step::Idle {
            return;
        }
        self.step = Step::Mtu;
        let mut pdu = vec![EXCHANGE_MTU_REQUEST];
        pdu.extend(MAX_MTU.to_le_bytes());
        self.request(pdu, now);
    }

    /// Enables notifications on the notify characteristic.
    pub fn subscribe(&mut self, now: f64) -> Vec<Event> {
        let Some(cccd) = self.handles.cccd else {
            return vec![Event::Log("cannot enable notifications: no descriptor found".into())];
        };
        let mut pdu = vec![WRITE_REQUEST];
        pdu.extend(cccd.to_le_bytes());
        pdu.extend(1u16.to_le_bytes());
        self.request(pdu, now);
        self.subscribed = true;
        Vec::new()
    }

    /// Writes to the write characteristic without a response.
    pub fn write(&mut self, data: &[u8]) -> Vec<Event> {
        let Some(handle) = self.handles.write else {
            return vec![Event::Log("cannot write: no write characteristic found".into())];
        };
        if data.len() > usize::from(self.mtu) - 3 {
            return vec![Event::Log(format!(
                "cannot write {} bytes at MTU {}",
                data.len(),
                self.mtu
            ))];
        }
        let mut pdu = vec![WRITE_COMMAND];
        pdu.extend(handle.to_le_bytes());
        pdu.extend(data);
        self.outgoing.push(pdu);
        Vec::new()
    }

    /// Marks the link failed if the request in flight has timed out.
    pub fn tick(&mut self, now: f64) {
        if let Some((opcode, at)) = self.waiting
            && at <= now
            && self.failed.is_none()
        {
            self.failed = Some(format!("ATT request 0x{opcode:02X} timed out"));
            self.queued.clear();
        }
    }

    pub fn receive(&mut self, pdu: &[u8], now: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(&opcode) = pdu.first() else { return events };
        match opcode {
            HANDLE_VALUE_NOTIFICATION | HANDLE_VALUE_INDICATION => {
                if opcode == HANDLE_VALUE_INDICATION {
                    self.outgoing.push(vec![HANDLE_VALUE_CONFIRMATION]);
                }
                if self.subscribed && pdu.len() >= 3 && Some(le16(pdu, 1)) == self.handles.notify {
                    events.push(Event::Notification(pdu[3..].to_vec()));
                }
            }
            EXCHANGE_MTU_REQUEST => {
                if pdu.len() >= 3 {
                    self.set_mtu(le16(pdu, 1));
                }
                let mut response = vec![EXCHANGE_MTU_RESPONSE];
                response.extend(MAX_MTU.to_le_bytes());
                self.outgoing.push(response);
            }
            _ if SERVER_REQUESTS.contains(&opcode) => {
                self.outgoing.push(vec![ERROR_RESPONSE, opcode, 0, 0, REQUEST_NOT_SUPPORTED]);
            }
            _ => self.response(pdu, now, &mut events),
        }
        events
    }

    fn request(&mut self, pdu: Vec<u8>, now: f64) {
        if self.failed.is_some() {
            return;
        }
        if self.waiting.is_some() {
            self.queued.push_back(pdu);
        } else {
            self.waiting = Some((pdu[0], now + REQUEST_TIMEOUT));
            self.outgoing.push(pdu);
        }
    }

    fn set_mtu(&mut self, server: u16) {
        self.mtu = server.clamp(DEFAULT_MTU, MAX_MTU);
    }

    /// A response to the request in flight; anything else is ignored.
    fn response(&mut self, pdu: &[u8], now: f64, events: &mut Vec<Event>) {
        let Some((sent, _)) = self.waiting else { return };
        let error = pdu[0] == ERROR_RESPONSE;
        let answers = if error { pdu.get(1) == Some(&sent) } else { pdu[0] == sent + 1 };
        if !answers || (error && pdu.len() < 5) {
            return;
        }
        self.waiting = None;
        let error_code = error.then(|| pdu[4]);

        match sent {
            EXCHANGE_MTU_REQUEST => self.on_mtu(pdu, error_code, now, events),
            READ_BY_GROUP_TYPE_REQUEST => self.on_services(pdu, error_code, now, events),
            READ_BY_TYPE_REQUEST => self.on_characteristics(pdu, error_code, now, events),
            FIND_INFORMATION_REQUEST => self.on_descriptors(pdu, error_code, now, events),
            WRITE_REQUEST => match error_code {
                None => events.push(Event::Log("notifications enabled".into())),
                Some(code) => events.push(Event::Log(format!(
                    "enabling notifications failed: ATT error 0x{code:02X}"
                ))),
            },
            _ => {}
        }
        if self.waiting.is_none()
            && let Some(next) = self.queued.pop_front()
        {
            self.request(next, now);
        }
    }

    fn on_mtu(&mut self, pdu: &[u8], error: Option<u8>, now: f64, events: &mut Vec<Event>) {
        match error {
            None if pdu.len() >= 3 => self.set_mtu(le16(pdu, 1)),
            _ => events.push(Event::Log("no MTU exchange; keeping the default".into())),
        }
        events.push(Event::Log(format!("MTU {}", self.mtu)));
        self.find_services(1, now);
    }

    fn find_services(&mut self, from: u16, now: f64) {
        self.step = Step::Services { from };
        let mut pdu = vec![READ_BY_GROUP_TYPE_REQUEST];
        pdu.extend(from.to_le_bytes());
        pdu.extend(0xFFFFu16.to_le_bytes());
        pdu.extend(PRIMARY_SERVICE.to_le_bytes());
        self.request(pdu, now);
    }

    fn on_services(&mut self, pdu: &[u8], error: Option<u8>, now: f64, events: &mut Vec<Event>) {
        let Step::Services { from } = self.step else { return };
        if error.is_some() {
            return self.finish(events, "Joy-Con 2 service not found");
        }
        // Each entry: start handle, end handle, then a 16- or 128-bit UUID.
        let Some(entries) = entries(pdu, &[6, 20]) else {
            return self.finish(events, "malformed service list");
        };
        let mut last = 0;
        for entry in entries {
            let (start, end) = (le16(entry, 0), le16(entry, 2));
            if start < from || end < start {
                return self.finish(events, "malformed service list");
            }
            if entry[4..] == self.service {
                events.push(Event::Log(format!("service at 0x{start:04X}-0x{end:04X}")));
                return self.find_characteristics(start, end, now);
            }
            last = last.max(end);
        }
        // Every end is at least `from`, so the next search starts further on.
        match last.checked_add(1) {
            Some(next) => self.find_services(next, now),
            None => self.finish(events, "Joy-Con 2 service not found"),
        }
    }

    fn find_characteristics(&mut self, from: u16, end: u16, now: f64) {
        self.step = Step::Characteristics { from, end };
        let mut pdu = vec![READ_BY_TYPE_REQUEST];
        pdu.extend(from.to_le_bytes());
        pdu.extend(end.to_le_bytes());
        pdu.extend(CHARACTERISTIC.to_le_bytes());
        self.request(pdu, now);
    }

    fn on_characteristics(
        &mut self,
        pdu: &[u8],
        error: Option<u8>,
        now: f64,
        events: &mut Vec<Event>,
    ) {
        let Step::Characteristics { from, end } = self.step else { return };
        if let Some(code) = error {
            if code != ATTRIBUTE_NOT_FOUND {
                events.push(Event::Log(format!("characteristic search: ATT error 0x{code:02X}")));
            }
            return self.characteristics_done(end, now, events);
        }
        // Each entry: declaration handle, properties, value handle, UUID.
        let Some(entries) = entries(pdu, &[7, 21]) else {
            return self.finish(events, "malformed characteristic list");
        };
        let mut last = from;
        for entry in entries {
            let (handle, value) = (le16(entry, 0), le16(entry, 3));
            if handle < from || handle > end || value <= handle || value > end {
                return self.finish(events, "malformed characteristic list");
            }
            last = handle;
            if self.notify_declaration.is_some_and(|n| handle > n.handle) {
                self.next_declaration.get_or_insert(handle);
            }
            let uuid = &entry[5..];
            if uuid == self.write_uuid {
                self.handles.write = Some(value);
            } else if uuid == self.notify_uuid {
                self.handles.notify = Some(value);
                self.notify_declaration = Some(Declaration { handle, value });
                self.next_declaration = None;
            }
        }
        match last.checked_add(1) {
            Some(next) if next <= end => self.find_characteristics(next, end, now),
            _ => self.characteristics_done(end, now, events),
        }
    }

    fn characteristics_done(&mut self, service_end: u16, now: f64, events: &mut Vec<Event>) {
        let Some(notify) = self.notify_declaration else {
            return self.finish(events, "notify characteristic not found");
        };
        let end = self.next_declaration.map_or(service_end, |next| next - 1);
        if notify.value >= end {
            return self.finish(events, "notify characteristic has no descriptors");
        }
        self.find_descriptors(notify.value + 1, end, now);
    }

    fn find_descriptors(&mut self, from: u16, end: u16, now: f64) {
        self.step = Step::Descriptors { from, end };
        let mut pdu = vec![FIND_INFORMATION_REQUEST];
        pdu.extend(from.to_le_bytes());
        pdu.extend(end.to_le_bytes());
        self.request(pdu, now);
    }

    fn on_descriptors(&mut self, pdu: &[u8], error: Option<u8>, now: f64, events: &mut Vec<Event>) {
        let Step::Descriptors { from, end } = self.step else { return };
        if error.is_some() {
            return self.finish(events, "notify descriptor not found");
        }
        // Format 1: handle and 16-bit UUID pairs; format 2: 128-bit UUIDs.
        let width = match pdu.get(1) {
            Some(1) => 4,
            Some(2) => 18,
            _ => return self.finish(events, "malformed descriptor list"),
        };
        let body = &pdu[2..];
        if body.is_empty() || !body.len().is_multiple_of(width) {
            return self.finish(events, "malformed descriptor list");
        }
        let mut last = from;
        for entry in body.chunks_exact(width) {
            let handle = le16(entry, 0);
            if handle < from || handle > end {
                return self.finish(events, "malformed descriptor list");
            }
            last = handle;
            if width == 4 && le16(entry, 2) == CLIENT_CHARACTERISTIC_CONFIGURATION {
                self.handles.cccd = Some(handle);
                return self.finish(events, "");
            }
        }
        match last.checked_add(1) {
            Some(next) if next <= end => self.find_descriptors(next, end, now),
            _ => self.finish(events, "notify descriptor not found"),
        }
    }

    /// Ends discovery, logging `problem` unless it is empty.
    fn finish(&mut self, events: &mut Vec<Event>, problem: &str) {
        self.step = Step::Done;
        if !problem.is_empty() {
            events.push(Event::Log(problem.into()));
        }
        let h = self.handles;
        let show = |handle: Option<u16>| handle.map_or("none".into(), |h| format!("0x{h:04X}"));
        events.push(Event::Log(format!(
            "handles: write {}, notify {}, descriptor {}",
            show(h.write),
            show(h.notify),
            show(h.cccd)
        )));
        events.push(Event::Discovered {
            write: h.write.is_some(),
            notify: h.notify.is_some() && h.cccd.is_some(),
        });
    }
}

fn le16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// The entries of a Read By Type or Read By Group Type response, if its
/// entry length is one of `widths` and the entries fill the PDU exactly.
fn entries<'a>(pdu: &'a [u8], widths: &[usize]) -> Option<std::slice::ChunksExact<'a, u8>> {
    let width = usize::from(*pdu.get(1)?);
    let body = &pdu[2..];
    if !widths.contains(&width) || body.is_empty() || !body.len().is_multiple_of(width) {
        return None;
    }
    Some(body.chunks_exact(width))
}
