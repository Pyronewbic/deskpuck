//! The Joy-Con 2 connection logic as a state machine with no I/O: feed it
//! inputs and the current time, carry out the outputs it returns. Timers are
//! deadlines checked by `tick`, so tests can drive every timeout directly.
//! Ported from Sources/DeskpuckCore/Joycon2BLEReceiver.mm.

use deskpuck_core::packet::{REPORT_MIN_SIZE, Report, parse_report};
use std::collections::HashMap;
use std::hash::Hash;

pub const MANUFACTURER_ID: u16 = 0x0553;
pub const WRITE_CHARACTERISTIC: &str = "649d4ac9-8eb7-4e6c-af44-1ea54fe5f005";
pub const NOTIFY_CHARACTERISTIC: &str = "ab7de9be-89fe-49ad-828f-118f09df7fd2";

/// Written without response once the characteristics are found: select every
/// feature (buttons, sticks, IMU, mouse...), then enable them.
pub const INIT_COMMANDS: [[u8; 12]; 2] = [
    [0x0c, 0x91, 0x01, 0x02, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
    [0x0c, 0x91, 0x01, 0x04, 0x00, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
];

pub const CONNECT_TIMEOUT: f64 = 60.0;
pub const RESCAN_AFTER_FAILURE: f64 = 2.0;
pub const RESCAN_AFTER_DISCONNECT: f64 = 3.0;
pub const DATA_TIMEOUT: f64 = 30.0;
pub const INIT_DELAY: f64 = 0.5;
pub const INIT_SPACING: f64 = 0.5;
/// Notifications are enabled on discovery and again after this delay, as the
/// C++ receiver does; some connections only start streaming on the second.
pub const RESUBSCRIBE_DELAY: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    BluetoothOff,
    Searching,
    Connecting,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input<Id> {
    AdapterPoweredOn,
    AdapterPoweredOff,
    Discovered { id: Id, name: Option<String>, manufacturer_ids: Vec<u16> },
    Connected(Id),
    ConnectFailed(Id),
    Disconnected(Id),
    CharacteristicsFound { id: Id, write: bool, notify: bool },
    Notification { id: Id, data: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Output<Id> {
    StartScan,
    StopScan,
    Connect(Id),
    Disconnect(Id),
    DiscoverServices(Id),
    Subscribe(Id),
    /// Write without response to the write characteristic.
    Write {
        id: Id,
        data: Vec<u8>,
    },
    Status {
        status: Status,
        name: Option<String>,
    },
    Report {
        report: Report,
        data: Vec<u8>,
    },
}

#[derive(Debug)]
struct Link<Id> {
    id: Id,
    name: Option<String>,
    data_deadline: f64,
    pending_writes: Vec<(f64, Vec<u8>)>,
    resubscribe_at: Option<f64>,
}

#[derive(Debug)]
pub struct Receiver<Id> {
    powered: bool,
    want_scan: bool,
    suspended: bool,
    /// At most one: a second Joy-Con is ignored while one is connecting or connected.
    connecting: HashMap<Id, (f64, Option<String>)>,
    link: Option<Link<Id>>,
    rescan_at: Option<f64>,
}

impl<Id> Default for Receiver<Id> {
    fn default() -> Self {
        Self {
            powered: false,
            want_scan: false,
            suspended: false,
            connecting: HashMap::new(),
            link: None,
            rescan_at: None,
        }
    }
}

impl<Id: Clone + Eq + Hash> Receiver<Id> {
    /// Starts looking for a Joy-Con, now or as soon as Bluetooth is on.
    pub fn start(&mut self) -> Vec<Output<Id>> {
        self.want_scan = true;
        let mut out = Vec::new();
        self.scan(&mut out);
        out
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    /// While suspended nothing starts a scan or a connection; an existing link
    /// is kept. Resuming scans again if a scan was wanted and nothing is linked.
    pub fn set_suspended(&mut self, suspended: bool) -> Vec<Output<Id>> {
        self.suspended = suspended;
        let mut out = Vec::new();
        if suspended {
            out.push(Output::StopScan);
        } else if self.connecting.is_empty() && self.link.is_none() {
            self.scan(&mut out);
        }
        out
    }

    // Every scan goes through here, so the suspend gate cannot be bypassed.
    fn scan(&mut self, out: &mut Vec<Output<Id>>) {
        if self.want_scan && !self.suspended && self.powered {
            out.push(Output::StartScan);
        }
    }

    fn status(out: &mut Vec<Output<Id>>, status: Status, name: Option<String>) {
        out.push(Output::Status { status, name });
    }

    pub fn handle(&mut self, input: Input<Id>, now: f64) -> Vec<Output<Id>> {
        let mut out = Vec::new();
        match input {
            Input::AdapterPoweredOn => {
                self.powered = true;
                if self.link.is_none() && self.connecting.is_empty() {
                    Self::status(&mut out, Status::Searching, None);
                }
                self.scan(&mut out);
            }
            Input::AdapterPoweredOff => {
                self.powered = false;
                Self::status(&mut out, Status::BluetoothOff, None);
            }
            Input::Discovered { id, name, manufacturer_ids } => {
                // A discovery queued before the scan stopped must not start a connection.
                let busy = self.link.is_some() || !self.connecting.is_empty();
                if self.suspended || busy || !manufacturer_ids.contains(&MANUFACTURER_ID) {
                    return out;
                }
                self.connecting.insert(id.clone(), (now + CONNECT_TIMEOUT, name.clone()));
                Self::status(&mut out, Status::Connecting, name);
                out.push(Output::Connect(id));
            }
            Input::Connected(id) => {
                let Some((_, name)) = self.connecting.remove(&id) else {
                    // Not ours, or it already timed out: let it go.
                    out.push(Output::Disconnect(id));
                    return out;
                };
                self.link = Some(Link {
                    id: id.clone(),
                    name: name.clone(),
                    data_deadline: now + DATA_TIMEOUT,
                    pending_writes: Vec::new(),
                    resubscribe_at: None,
                });
                Self::status(&mut out, Status::Connected, name);
                out.push(Output::DiscoverServices(id));
            }
            Input::ConnectFailed(id) => {
                if self.connecting.remove(&id).is_some() {
                    Self::status(&mut out, Status::Searching, None);
                    self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
                }
            }
            Input::Disconnected(id) => {
                if self.link.as_ref().is_some_and(|l| l.id == id) {
                    self.link = None;
                    Self::status(&mut out, Status::Searching, None);
                    self.rescan_at = Some(now + RESCAN_AFTER_DISCONNECT);
                } else if self.connecting.remove(&id).is_some() {
                    Self::status(&mut out, Status::Searching, None);
                    self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
                }
            }
            Input::CharacteristicsFound { id, write, notify } => {
                let Some(link) = self.link.as_mut().filter(|l| l.id == id) else { return out };
                if !(write && notify) {
                    // Without both there is no data, and the data timeout reconnects.
                    return out;
                }
                out.push(Output::Subscribe(id));
                link.pending_writes = INIT_COMMANDS
                    .iter()
                    .enumerate()
                    .map(|(i, cmd)| (now + INIT_DELAY + i as f64 * INIT_SPACING, cmd.to_vec()))
                    .collect();
                link.resubscribe_at = Some(now + RESUBSCRIBE_DELAY);
            }
            Input::Notification { id, data } => {
                let Some(link) = self.link.as_mut().filter(|l| l.id == id) else { return out };
                // Short reports are dropped without counting as data, as in the C++.
                if data.len() < REPORT_MIN_SIZE {
                    return out;
                }
                if let Some(report) = parse_report(&data) {
                    link.data_deadline = now + DATA_TIMEOUT;
                    out.push(Output::Report { report, data });
                }
            }
        }
        out
    }

    /// Fires every timer that is due at `now`.
    pub fn tick(&mut self, now: f64) -> Vec<Output<Id>> {
        let mut out = Vec::new();

        let expired: Vec<Id> = self
            .connecting
            .iter()
            .filter(|(_, (deadline, _))| *deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.connecting.remove(&id);
            out.push(Output::Disconnect(id));
            Self::status(&mut out, Status::Searching, None);
            self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
        }

        if let Some(link) = self.link.as_mut() {
            let (due, later): (Vec<_>, Vec<_>) =
                link.pending_writes.drain(..).partition(|(at, _)| *at <= now);
            link.pending_writes = later;
            out.extend(
                due.into_iter().map(|(_, data)| Output::Write { id: link.id.clone(), data }),
            );
            if link.resubscribe_at.is_some_and(|at| at <= now) {
                link.resubscribe_at = None;
                out.push(Output::Subscribe(link.id.clone()));
            }
            if link.data_deadline <= now {
                // Once: the Disconnected input that follows clears the link.
                link.data_deadline = f64::INFINITY;
                out.push(Output::Disconnect(link.id.clone()));
            }
        }

        if self.rescan_at.is_some_and(|at| at <= now) {
            self.rescan_at = None;
            self.scan(&mut out);
        }
        out
    }

    pub fn linked(&self) -> Option<(&Id, Option<&str>)> {
        self.link.as_ref().map(|l| (&l.id, l.name.as_deref()))
    }
}
