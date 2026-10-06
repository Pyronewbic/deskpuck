//! The Joy-Con 2 connection logic as a state machine with no I/O: feed it
//! inputs and the current time, carry out the outputs it returns. Timers are
//! deadlines checked by `tick`, so tests can drive every timeout directly.

use deskpuck_core::packet::{REPORT_MIN_SIZE, Report, parse_report};
use deskpuck_core::pairing::clean_name;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;

pub const MANUFACTURER_ID: u16 = 0x0553;
pub const WRITE_CHARACTERISTIC: &str = "649d4ac9-8eb7-4e6c-af44-1ea54fe5f005";
pub const NOTIFY_CHARACTERISTIC: &str = "ab7de9be-89fe-49ad-828f-118f09df7fd2";
/// Holds both characteristics; recorded from real Joy-Con 2s (L and R).
pub const SERVICE: &str = "ab7de9be-89fe-49ad-828f-118f09df7fd0";

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
/// Mac app always has; some connections only start streaming on the second.
pub const RESUBSCRIBE_DELAY: f64 = 2.0;
/// How long a pairing window accepts a Joy-Con that is not the paired one.
pub const PAIRING_WINDOW: f64 = 60.0;

/// A name for a Joy-Con 2 from its manufacturer data (Nintendo's vendor id,
/// then the product id), for when the advertised name has not arrived yet:
/// it comes in a scan response that can follow the first discovery.
pub fn name_from_manufacturer_data(data: &[u8]) -> Option<&'static str> {
    let field = |at: usize| Some(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]));
    if field(3)? != 0x057E {
        return None;
    }
    match field(5)? {
        0x2066 => Some("Joy-Con 2 (R)"),
        0x2067 => Some("Joy-Con 2 (L)"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    BluetoothOff,
    /// Nothing is paired and no pairing window is open, so nothing connects.
    NotPaired,
    /// A pairing window is open: the first Joy-Con found becomes the paired one.
    Pairing,
    /// Looking for the paired Joy-Con.
    Searching,
    /// The paired Joy-Con is connected to this computer, but by another program.
    InUseElsewhere,
    Connecting,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input<Id> {
    AdapterPoweredOn,
    AdapterPoweredOff,
    Discovered {
        id: Id,
        name: Option<String>,
        manufacturer_ids: Vec<u16>,
    },
    Connected(Id),
    ConnectFailed(Id),
    Disconnected(Id),
    CharacteristicsFound {
        id: Id,
        write: bool,
        notify: bool,
    },
    Notification {
        id: Id,
        data: Vec<u8>,
    },
    /// Joy-Cons connected to this computer by any program, polled while
    /// `wants_presence_check`.
    SystemConnected(Vec<Id>),
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
    /// A Joy-Con connected during a pairing window is now the paired one.
    Paired {
        id: Id,
        name: Option<String>,
    },
}

#[derive(Debug)]
struct Link<Id> {
    id: Id,
    name: Option<String>,
    data_deadline: f64,
    pending_writes: Vec<(f64, Vec<u8>)>,
    resubscribe_at: Option<f64>,
    /// Accepted by a pairing window and not yet confirmed as a Joy-Con.
    pairing: bool,
}

#[derive(Debug)]
struct Pending {
    deadline: f64,
    name: Option<String>,
    pairing: bool,
}

#[derive(Debug)]
pub struct Receiver<Id> {
    powered: bool,
    want_scan: bool,
    suspended: bool,
    /// At most one: a second Joy-Con is ignored while one is connecting or connected.
    connecting: HashMap<Id, Pending>,
    link: Option<Link<Id>>,
    rescan_at: Option<f64>,
    /// The paired Joy-Con's id, as its `Display` string.
    paired: Option<String>,
    pairing_until: Option<f64>,
    /// The paired Joy-Con was last seen connected by another program.
    held_elsewhere: bool,
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
            paired: None,
            pairing_until: None,
            held_elsewhere: false,
        }
    }
}

impl<Id: Clone + Eq + Hash + Display> Receiver<Id> {
    /// Starts looking for a Joy-Con, now or as soon as Bluetooth is on.
    pub fn start(&mut self) -> Vec<Output<Id>> {
        self.want_scan = true;
        let mut out = Vec::new();
        self.scan(&mut out);
        out
    }

    /// Set before `start`, from the stored pairing.
    pub fn set_paired(&mut self, id: Option<String>) {
        self.paired = id;
    }

    pub fn paired(&self) -> Option<&str> {
        self.paired.as_deref()
    }

    pub fn is_pairing(&self) -> bool {
        self.pairing_until.is_some()
    }

    /// Only while searching for the paired Joy-Con is it worth asking whether
    /// another program holds it; then it does not advertise, so a scan never finds it.
    pub fn wants_presence_check(&self) -> bool {
        self.want_scan
            && self.powered
            && !self.suspended
            && self.paired.is_some()
            && self.pairing_until.is_none()
            && self.link.is_none()
            && self.connecting.is_empty()
    }

    /// Opens a pairing window, letting go of any Joy-Con linked now so a new
    /// one can connect. Ignored while suspended, which never connects.
    pub fn start_pairing(&mut self, now: f64) -> Vec<Output<Id>> {
        let mut out = Vec::new();
        if self.suspended {
            return out;
        }
        self.pairing_until = Some(now + PAIRING_WINDOW);
        self.held_elsewhere = false;
        let dropped: Vec<Id> = self.connecting.drain().map(|(id, _)| id).collect();
        out.extend(dropped.into_iter().map(Output::Disconnect));
        if let Some(link) = self.link.take() {
            out.push(Output::Disconnect(link.id));
        }
        self.rescan_at = None;
        if self.powered {
            self.idle_status(&mut out);
        }
        self.scan(&mut out);
        out
    }

    /// Closes the pairing window, dropping a Joy-Con it accepted that is not
    /// confirmed yet, even one still finishing after the window expired. The
    /// paired Joy-Con, if any, is searched for again.
    pub fn cancel_pairing(&mut self) -> Vec<Output<Id>> {
        let mut out = Vec::new();
        let window_open = self.pairing_until.take().is_some();
        if !self.drop_unconfirmed(&mut out) && !window_open {
            return out;
        }
        self.pairing_ended(&mut out);
        self.scan(&mut out);
        out
    }

    /// Disconnects any Joy-Con accepted by a pairing window and not yet
    /// confirmed, so nothing is paired without the user's window open.
    fn drop_unconfirmed(&mut self, out: &mut Vec<Output<Id>>) -> bool {
        let unconfirmed: Vec<Id> =
            self.connecting.iter().filter(|(_, p)| p.pairing).map(|(id, _)| id.clone()).collect();
        let mut dropped = !unconfirmed.is_empty();
        for id in unconfirmed {
            self.connecting.remove(&id);
            out.push(Output::Disconnect(id));
        }
        if let Some(link) = self.link.take_if(|l| l.pairing) {
            out.push(Output::Disconnect(link.id));
            dropped = true;
        }
        dropped
    }

    fn pairing_ended(&mut self, out: &mut Vec<Output<Id>>) {
        if self.powered && self.link.is_none() && self.connecting.is_empty() {
            self.idle_status(out);
        }
        if self.paired.is_none() {
            out.push(Output::StopScan);
        }
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
            // Pausing must not let a pairing finish; the window itself survives.
            if self.drop_unconfirmed(&mut out)
                && self.powered
                && self.link.is_none()
                && self.connecting.is_empty()
            {
                self.idle_status(&mut out);
            }
        } else if self.connecting.is_empty() && self.link.is_none() {
            self.scan(&mut out);
        }
        out
    }

    // Every scan goes through here, so the suspend and pairing gates cannot be bypassed.
    fn scan(&mut self, out: &mut Vec<Output<Id>>) {
        let anything_to_find = self.paired.is_some() || self.pairing_until.is_some();
        if self.want_scan && !self.suspended && self.powered && anything_to_find {
            out.push(Output::StartScan);
        }
    }

    fn status(out: &mut Vec<Output<Id>>, status: Status, name: Option<String>) {
        out.push(Output::Status { status, name });
    }

    /// The status while nothing is linked or connecting.
    fn idle_status(&self, out: &mut Vec<Output<Id>>) {
        let status = if self.pairing_until.is_some() {
            Status::Pairing
        } else if self.paired.is_some() && self.held_elsewhere {
            Status::InUseElsewhere
        } else if self.paired.is_some() {
            Status::Searching
        } else {
            Status::NotPaired
        };
        Self::status(out, status, None);
    }

    pub fn handle(&mut self, input: Input<Id>, now: f64) -> Vec<Output<Id>> {
        let mut out = Vec::new();
        match input {
            Input::AdapterPoweredOn => {
                self.powered = true;
                if self.link.is_none() && self.connecting.is_empty() {
                    self.idle_status(&mut out);
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
                // Outside a pairing window only the paired Joy-Con may connect.
                let pairing = self.pairing_until.is_some_and(|until| now < until);
                if !pairing && self.paired.as_deref() != Some(id.to_string().as_str()) {
                    return out;
                }
                // The advertised name is attacker-chosen and ends up in terminals and menus.
                let name = name.as_deref().and_then(clean_name);
                self.held_elsewhere = false;
                self.connecting.insert(
                    id.clone(),
                    Pending { deadline: now + CONNECT_TIMEOUT, name: name.clone(), pairing },
                );
                // A rescan still pending from an earlier drop is moot now.
                self.rescan_at = None;
                Self::status(&mut out, Status::Connecting, name);
                out.push(Output::Connect(id));
            }
            Input::Connected(id) => {
                let Some(Pending { name, pairing, .. }) = self.connecting.remove(&id) else {
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
                    pairing,
                });
                Self::status(&mut out, Status::Connected, name);
                out.push(Output::DiscoverServices(id));
            }
            Input::ConnectFailed(id) => {
                if self.connecting.remove(&id).is_some() {
                    self.idle_status(&mut out);
                    self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
                }
            }
            Input::Disconnected(id) => {
                if self.link.as_ref().is_some_and(|l| l.id == id) {
                    self.link = None;
                    self.idle_status(&mut out);
                    self.rescan_at = Some(now + RESCAN_AFTER_DISCONNECT);
                } else if self.connecting.remove(&id).is_some() {
                    self.idle_status(&mut out);
                    self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
                }
            }
            Input::CharacteristicsFound { id, write, notify } => {
                let Some(link) = self.link.as_mut().filter(|l| l.id == id) else { return out };
                if !(write && notify) {
                    // Without both there is no data, and the data timeout reconnects.
                    return out;
                }
                // Both characteristics make it a Joy-Con 2, so a pairing link is confirmed.
                if link.pairing {
                    link.pairing = false;
                    self.pairing_until = None;
                    self.paired = Some(id.to_string());
                    out.push(Output::Paired { id: id.clone(), name: link.name.clone() });
                }
                out.push(Output::Subscribe(id));
                link.pending_writes = INIT_COMMANDS
                    .iter()
                    .enumerate()
                    .map(|(i, cmd)| (now + INIT_DELAY + i as f64 * INIT_SPACING, cmd.to_vec()))
                    .collect();
                link.resubscribe_at = Some(now + RESUBSCRIBE_DELAY);
            }
            Input::SystemConnected(ids) => {
                if !self.wants_presence_check() {
                    return out;
                }
                let paired = self.paired.as_deref();
                let held = ids.iter().any(|id| Some(id.to_string().as_str()) == paired);
                if held != self.held_elsewhere {
                    self.held_elsewhere = held;
                    self.idle_status(&mut out);
                }
            }
            Input::Notification { id, data } => {
                let Some(link) = self.link.as_mut().filter(|l| l.id == id) else { return out };
                // Short reports are dropped without counting as data.
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
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.connecting.remove(&id);
            out.push(Output::Disconnect(id));
            self.idle_status(&mut out);
            self.rescan_at = Some(now + RESCAN_AFTER_FAILURE);
        }

        // A Joy-Con already accepted by the window may still finish connecting.
        if self.pairing_until.is_some_and(|until| until <= now) {
            self.pairing_until = None;
            self.pairing_ended(&mut out);
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
