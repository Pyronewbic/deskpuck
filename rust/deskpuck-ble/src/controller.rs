//! Runs the receiver against real Bluetooth on a background thread, for both
//! the command-line tool and the Mac app. `Hub` holds every decision and is
//! tested without hardware; `Controller` only moves data between the Hub and
//! btleplug. On Linux the Joy-Con link itself is a direct ATT socket instead.

use crate::Session;
use crate::receiver::{
    Input, MANUFACTURER_ID, NOTIFY_CHARACTERISTIC, Output, Receiver, SERVICE, Status,
    WRITE_CHARACTERISTIC, device_name,
};
use btleplug::api::{
    Central, CentralEvent, CentralState, Characteristic, Manager as _, Peripheral as _,
    RetrievePeripheralsOptions, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::packet::Report;
use deskpuck_core::pairing::PairedDevice;
use deskpuck_inject::Sink;
use futures::StreamExt;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// What the user should see. Unauthorized and Unavailable come from the
/// Bluetooth stack itself rather than the receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStatus {
    BluetoothOff,
    BluetoothUnauthorized,
    Unavailable,
    NotPaired,
    Pairing,
    Searching,
    InUseElsewhere,
    Connecting,
    Connected,
}

impl From<Status> for LinkStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::BluetoothOff => LinkStatus::BluetoothOff,
            Status::NotPaired => LinkStatus::NotPaired,
            Status::Pairing => LinkStatus::Pairing,
            Status::Searching => LinkStatus::Searching,
            Status::InUseElsewhere => LinkStatus::InUseElsewhere,
            Status::Connecting => LinkStatus::Connecting,
            Status::Connected => LinkStatus::Connected,
        }
    }
}

pub type StatusHook = Box<dyn FnMut(LinkStatus, Option<&str>) + Send>;
/// Report, raw bytes, device name, and milliseconds since connecting.
pub type ReportHook = Box<dyn FnMut(&Report, &[u8], Option<&str>, u128) + Send>;
pub type MessageHook = Box<dyn Fn(&str) + Send>;
pub type LatchHook = Box<dyn FnMut(Modifiers) + Send>;

/// Callbacks run on the controller's thread, never on the caller's.
pub struct Hooks {
    pub status: StatusHook,
    /// Every parsed report, paused or not.
    pub report: Option<ReportHook>,
    /// Connection detail, for --verbose.
    pub log: Option<MessageHook>,
    /// Failures worth showing even without --verbose.
    pub error: MessageHook,
    /// The modifiers latched on by modifier buttons, whenever they change.
    pub latched: Option<LatchHook>,
}

/// The receiver plus the session: inputs in, Bluetooth commands out.
pub struct Hub<Id, S: Sink> {
    receiver: Receiver<Id>,
    session: Session<S>,
    hooks: Hooks,
    connected_at: f64,
    pairing_file: Option<PathBuf>,
    last_latched: Modifiers,
}

impl<Id: Clone + Eq + Hash + Display, S: Sink> Hub<Id, S> {
    /// Connects only to the Joy-Con paired in `pairing_file`, and saves a new
    /// pairing there. Without a file, a pairing lasts until the Hub is dropped.
    pub fn new(
        settings: EngineSettings,
        sink: S,
        hooks: Hooks,
        pairing_file: Option<PathBuf>,
    ) -> Self {
        let mut hub = Self {
            receiver: Receiver::default(),
            session: Session::new(settings, sink),
            hooks,
            connected_at: 0.0,
            pairing_file,
            last_latched: Modifiers::NONE,
        };
        if let Some(path) = &hub.pairing_file {
            match PairedDevice::load(path) {
                Ok(device) => hub.receiver.set_paired(device.map(|d| d.id)),
                Err(problem) => (hub.hooks.error)(&format!("{problem}; pair the Joy-Con again.")),
            }
        }
        hub
    }

    pub fn paired(&self) -> Option<&str> {
        self.receiver.paired()
    }

    pub fn wants_presence_check(&self) -> bool {
        self.receiver.wants_presence_check()
    }

    pub fn start_pairing(&mut self, now: f64) -> Vec<Output<Id>> {
        let out = self.receiver.start_pairing(now);
        self.route(out, now)
    }

    pub fn cancel_pairing(&mut self, now: f64) -> Vec<Output<Id>> {
        let out = self.receiver.cancel_pairing();
        self.route(out, now)
    }

    pub fn session(&self) -> &Session<S> {
        &self.session
    }

    pub fn start(&mut self, now: f64) -> Vec<Output<Id>> {
        let out = self.receiver.start();
        self.route(out, now)
    }

    pub fn input(&mut self, input: Input<Id>, now: f64) -> Vec<Output<Id>> {
        let out = self.receiver.handle(input, now);
        self.route(out, now)
    }

    pub fn tick(&mut self, now: f64) -> Vec<Output<Id>> {
        let out = self.receiver.tick(now);
        self.route(out, now)
    }

    /// Pausing releases held input and stops scanning; a connected Joy-Con
    /// stays connected so resuming is instant.
    pub fn set_paused(&mut self, paused: bool, now: f64) -> Vec<Output<Id>> {
        if let Err(e) = self.session.set_paused(paused) {
            (self.hooks.error)(&e.to_string());
        }
        let out = self.receiver.set_suspended(paused);
        self.route(out, now)
    }

    pub fn apply_settings(&mut self, settings: EngineSettings) {
        if let Err(e) = self.session.apply_settings(settings) {
            (self.hooks.error)(&e.to_string());
        }
        self.notify_latched();
    }

    fn notify_latched(&mut self) {
        let latched = self.session.latched();
        if latched != self.last_latched {
            self.last_latched = latched;
            if let Some(hook) = self.hooks.latched.as_mut() {
                hook(latched);
            }
        }
    }

    /// A status from the Bluetooth stack rather than the receiver.
    pub fn report_status(&mut self, status: LinkStatus) {
        (self.hooks.status)(status, None);
    }

    pub fn error(&self, message: &str) {
        (self.hooks.error)(message);
    }

    pub fn logging(&self) -> bool {
        self.hooks.log.is_some()
    }

    pub fn log(&self, message: impl FnOnce() -> String) {
        if let Some(log) = &self.hooks.log {
            log(&message());
        }
    }

    /// Releases held input; the caller then stops scanning and disconnects `linked()`.
    pub fn shutdown(&mut self) {
        if let Err(e) = self.session.shutdown() {
            (self.hooks.error)(&e.to_string());
        }
        self.notify_latched();
    }

    pub fn linked(&self) -> Option<&Id> {
        self.receiver.linked().map(|(id, _)| id)
    }

    /// Statuses and reports are handled here; Bluetooth commands are returned.
    fn route(&mut self, outputs: Vec<Output<Id>>, now: f64) -> Vec<Output<Id>> {
        let mut commands = Vec::new();
        for output in outputs {
            match output {
                Output::Status { status, name } => {
                    if status == Status::Connected {
                        self.connected_at = now;
                    }
                    if let Err(e) = self.session.status(status) {
                        (self.hooks.error)(&e.to_string());
                    }
                    (self.hooks.status)(status.into(), name.as_deref());
                }
                Output::Report { report, data } => {
                    if let Some(hook) = self.hooks.report.as_mut() {
                        let name = self.receiver.linked().and_then(|(_, n)| n);
                        let elapsed = ((now - self.connected_at).max(0.0) * 1000.0) as u128;
                        hook(&report, &data, name, elapsed);
                    }
                    if let Err(e) = self.session.report(&report, now) {
                        (self.hooks.error)(&e.to_string());
                    }
                }
                Output::Paired { id, name } => self.remember(&id, name.as_deref()),
                command => commands.push(command),
            }
        }
        self.notify_latched();
        commands
    }

    fn remember(&self, id: &Id, name: Option<&str>) {
        self.log(|| format!("paired with {id}"));
        let Some(path) = &self.pairing_file else { return };
        let Some(device) = PairedDevice::new(&id.to_string(), name) else {
            (self.hooks.error)("This Joy-Con's id cannot be saved; it stays paired until quit.");
            return;
        };
        if let Err(e) = device.save(path) {
            (self.hooks.error)(&format!("Could not save the pairing to {}: {e}", path.display()));
        }
    }
}

enum Command {
    SetPaused(bool),
    Apply(EngineSettings),
    StartPairing,
    CancelPairing,
    Shutdown,
}

/// Where the paired Joy-Con is remembered, and whether to open a pairing
/// window as soon as Bluetooth is ready.
#[derive(Clone, Debug, Default)]
pub struct PairingSetup {
    pub file: Option<PathBuf>,
    pub pair_at_start: bool,
}

/// A running connection. Dropping it releases held input, disconnects the
/// Joy-Con and waits for the thread, so no hook runs after drop returns.
pub struct Controller {
    tx: UnboundedSender<Command>,
    paused: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Controller {
    pub fn start<S: Sink + Send + 'static>(
        settings: EngineSettings,
        sink: S,
        hooks: Hooks,
        pairing: PairingSetup,
    ) -> std::io::Result<Self> {
        let (tx, rx) = unbounded_channel();
        let runtime =
            tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
        let thread = std::thread::Builder::new().name("deskpuck-ble".into()).spawn(move || {
            let hub = Hub::new(settings, sink, hooks, pairing.file);
            runtime.block_on(run(hub, rx, pairing.pair_at_start));
        })?;
        Ok(Self { tx, paused: Arc::default(), thread: Some(thread) })
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        let _ = self.tx.send(Command::SetPaused(paused));
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn apply_settings(&self, settings: EngineSettings) {
        let _ = self.tx.send(Command::Apply(settings));
    }

    /// Opens a pairing window of `PAIRING_WINDOW` seconds.
    pub fn start_pairing(&self) {
        let _ = self.tx.send(Command::StartPairing);
    }

    pub fn cancel_pairing(&self) {
        let _ = self.tx.send(Command::CancelPairing);
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Without Bluetooth, still honor pause and settings until shutdown.
async fn idle<S: Sink>(
    mut hub: Hub<PeripheralId, S>,
    mut rx: UnboundedReceiver<Command>,
    start: Instant,
) {
    while let Some(command) = rx.recv().await {
        match command {
            Command::SetPaused(paused) => {
                hub.set_paused(paused, start.elapsed().as_secs_f64());
            }
            Command::Apply(settings) => hub.apply_settings(settings),
            Command::StartPairing | Command::CancelPairing => {}
            Command::Shutdown => break,
        }
    }
    hub.shutdown();
}

async fn run<S: Sink>(
    mut hub: Hub<PeripheralId, S>,
    mut rx: UnboundedReceiver<Command>,
    pair_at_start: bool,
) {
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64();

    let manager = match Manager::new().await {
        Ok(manager) => manager,
        Err(btleplug::Error::PermissionDenied) => {
            hub.report_status(LinkStatus::BluetoothUnauthorized);
            return idle(hub, rx, start).await;
        }
        Err(e) => {
            hub.error(&format!("Bluetooth is unavailable: {e}"));
            hub.report_status(LinkStatus::Unavailable);
            return idle(hub, rx, start).await;
        }
    };
    let Some(adapter) = manager.adapters().await.ok().and_then(|a| a.into_iter().next()) else {
        hub.error("No Bluetooth adapter found.");
        hub.report_status(LinkStatus::Unavailable);
        return idle(hub, rx, start).await;
    };
    let mut events = match adapter.events().await {
        Ok(events) => events,
        Err(e) => {
            hub.error(&format!("Could not listen for Bluetooth events: {e}"));
            hub.report_status(LinkStatus::Unavailable);
            return idle(hub, rx, start).await;
        }
    };

    let (input_tx, mut input_rx) = unbounded_channel();
    let mut driver = Driver {
        adapter: adapter.clone(),
        tx: input_tx,
        characteristics: Arc::default(),
        streams: HashMap::new(),
        #[cfg(target_os = "linux")]
        links: HashMap::new(),
    };

    // Before the first status, so a pairing run never reports NotPaired first.
    let mut commands = if pair_at_start { hub.start_pairing(now()) } else { Vec::new() };
    commands.extend(hub.start(now()));
    match adapter.adapter_state().await {
        Ok(CentralState::PoweredOn) => commands.extend(hub.input(Input::AdapterPoweredOn, now())),
        Ok(CentralState::PoweredOff) => commands.extend(hub.input(Input::AdapterPoweredOff, now())),
        _ => {}
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let mut presence = tokio::time::interval(PRESENCE_POLL);
    loop {
        for command in &commands {
            driver.execute(command, &hub).await;
        }
        commands = tokio::select! {
            command = rx.recv() => match command {
                Some(Command::SetPaused(paused)) => hub.set_paused(paused, now()),
                Some(Command::Apply(settings)) => {
                    hub.apply_settings(settings);
                    Vec::new()
                }
                Some(Command::StartPairing) => hub.start_pairing(now()),
                Some(Command::CancelPairing) => hub.cancel_pairing(now()),
                Some(Command::Shutdown) | None => break,
            },
            Some(event) = events.next() => match driver.on_event(event) {
                Some(input) => hub.input(input, now()),
                None => Vec::new(),
            },
            Some(back) = input_rx.recv() => deliver(&mut hub, back, now()),
            _ = ticker.tick() => hub.tick(now()),
            _ = presence.tick() => {
                if hub.wants_presence_check() {
                    driver.check_presence();
                }
                Vec::new()
            }
        };
    }

    // Release held input first, then let the Joy-Con go before the thread ends.
    hub.shutdown();
    let _ = adapter.stop_scan().await;
    #[cfg(target_os = "linux")]
    for (_, link) in driver.links.drain() {
        link.task.abort();
    }
    if let Some(id) = hub.linked().cloned()
        && let Ok(p) = adapter.peripheral(&id).await
    {
        let _ = tokio::time::timeout(Duration::from_secs(2), p.disconnect()).await;
    }
}

/// How often to ask whether another program holds the paired Joy-Con.
const PRESENCE_POLL: Duration = Duration::from_secs(3);

/// The two characteristics of a linked Joy-Con, found during service discovery.
type Characteristics = Arc<Mutex<HashMap<PeripheralId, (Characteristic, Characteristic)>>>;

/// What a Driver task sends back: input for the receiver, or a failure for
/// the verbose log that would otherwise only show as a retry.
enum Back<Id> {
    Input(Input<Id>),
    Log(String),
}

impl<Id> From<Input<Id>> for Back<Id> {
    fn from(input: Input<Id>) -> Self {
        Back::Input(input)
    }
}

fn deliver<Id: Clone + Eq + Hash + Display, S: Sink>(
    hub: &mut Hub<Id, S>,
    back: Back<Id>,
    now: f64,
) -> Vec<Output<Id>> {
    match back {
        Back::Input(input) => hub.input(input, now),
        Back::Log(message) => {
            hub.log(|| message);
            Vec::new()
        }
    }
}

struct Driver {
    adapter: Adapter,
    tx: UnboundedSender<Back<PeripheralId>>,
    characteristics: Characteristics,
    streams: HashMap<PeripheralId, JoinHandle<()>>,
    #[cfg(target_os = "linux")]
    links: HashMap<PeripheralId, DirectLink>,
}

/// A Joy-Con link over a direct ATT socket, and how to reach its task.
#[cfg(target_os = "linux")]
struct DirectLink {
    task: JoinHandle<()>,
    commands: UnboundedSender<crate::l2cap::Command>,
}

fn uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or_default()
}

impl Driver {
    /// Reports the Joy-Cons connected to this computer by any program. A
    /// backend that cannot tell (BlueZ, WinRT) reports nothing, so the status
    /// simply stays Searching.
    fn check_presence(&self) {
        let (adapter, tx) = (self.adapter.clone(), self.tx.clone());
        tokio::spawn(async move {
            let options = RetrievePeripheralsOptions {
                identifiers: None,
                services: Some(vec![uuid(SERVICE)]),
            };
            if let Ok(found) = adapter.retrieve_peripherals(options).await {
                let _ =
                    tx.send(Input::SystemConnected(found.iter().map(|p| p.id()).collect()).into());
            }
        });
    }

    /// Turns a btleplug event into receiver input. Discoveries need the
    /// peripheral's properties, which are fetched off the main loop.
    fn on_event(&mut self, event: CentralEvent) -> Option<Input<PeripheralId>> {
        match event {
            CentralEvent::DeviceDiscovered(id)
            | CentralEvent::DeviceUpdated(id)
            | CentralEvent::ManufacturerDataAdvertisement { id, .. } => {
                let (adapter, tx) = (self.adapter.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let Ok(peripheral) = adapter.peripheral(&id).await else { return };
                    if let Ok(Some(props)) = peripheral.properties().await {
                        let manufacturer_ids = props.manufacturer_data.keys().copied().collect();
                        let data = props.manufacturer_data.get(&MANUFACTURER_ID);
                        let name = device_name(props.local_name, data.map(Vec::as_slice));
                        let _ = tx.send(Input::Discovered { id, name, manufacturer_ids }.into());
                    }
                });
                None
            }
            CentralEvent::DeviceDisconnected(id) => {
                if let Some(stream) = self.streams.remove(&id) {
                    stream.abort();
                }
                #[cfg(target_os = "linux")]
                let input = bluez_disconnected(id, &self.links);
                #[cfg(not(target_os = "linux"))]
                let input = Some(Input::Disconnected(id));
                input
            }
            CentralEvent::StateUpdate(CentralState::PoweredOn) => Some(Input::AdapterPoweredOn),
            CentralEvent::StateUpdate(CentralState::PoweredOff) => Some(Input::AdapterPoweredOff),
            _ => None,
        }
    }

    /// Carries out one receiver command. Slow operations run as tasks that
    /// report back through `tx`, so the loop never blocks.
    async fn execute<S: Sink>(
        &mut self,
        output: &Output<PeripheralId>,
        hub: &Hub<PeripheralId, S>,
    ) {
        #[cfg(target_os = "linux")]
        if self.execute_direct(output, hub) {
            return;
        }
        match output {
            Output::StartScan => {
                hub.log(|| "scanning".into());
                if let Err(e) = self.adapter.start_scan(ScanFilter::default()).await {
                    hub.error(&format!("Could not scan: {e}"));
                }
            }
            Output::StopScan => {
                let _ = self.adapter.stop_scan().await;
            }
            Output::Connect(id) => {
                hub.log(|| format!("connecting to {id}"));
                if hub.logging()
                    && let Ok(p) = self.adapter.peripheral(id).await
                    && let Ok(Some(props)) = p.properties().await
                {
                    hub.log(|| format!("advertised: {:02X?}", props.manufacturer_data));
                }
                let (adapter, tx, id) = (self.adapter.clone(), self.tx.clone(), id.clone());
                tokio::spawn(async move {
                    let connected = match adapter.peripheral(&id).await {
                        Ok(p) => p.connect().await,
                        Err(e) => Err(e),
                    };
                    let input = match connected {
                        Ok(()) => Input::Connected(id),
                        Err(e) => {
                            let _ = tx.send(Back::Log(format!("could not connect to {id}: {e}")));
                            Input::ConnectFailed(id)
                        }
                    };
                    let _ = tx.send(input.into());
                });
            }
            Output::Disconnect(id) => {
                hub.log(|| format!("disconnecting {id}"));
                if let Some(stream) = self.streams.remove(id) {
                    stream.abort();
                }
                let (adapter, id) = (self.adapter.clone(), id.clone());
                tokio::spawn(async move {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        let _ = p.disconnect().await;
                    }
                });
            }
            Output::DiscoverServices(id) => {
                let (adapter, tx, id) = (self.adapter.clone(), self.tx.clone(), id.clone());
                let characteristics = self.characteristics.clone();
                tokio::spawn(async move {
                    let Ok(p) = adapter.peripheral(&id).await else { return };
                    if let Err(e) = p.discover_services().await {
                        let _ =
                            tx.send(Back::Log(format!("service discovery failed on {id}: {e}")));
                    }
                    let found = p.characteristics();
                    let find = |want: &str| found.iter().find(|c| c.uuid == uuid(want)).cloned();
                    let (write, notify) = (find(WRITE_CHARACTERISTIC), find(NOTIFY_CHARACTERISTIC));
                    let both = (write.is_some(), notify.is_some());
                    if let (Some(w), Some(n)) = (write, notify)
                        && let Ok(mut map) = characteristics.lock()
                    {
                        map.insert(id.clone(), (w, n));
                    }
                    let found = Input::CharacteristicsFound { id, write: both.0, notify: both.1 };
                    let _ = tx.send(found.into());
                });
            }
            Output::Subscribe(id) => {
                let Some((write, notify)) =
                    self.characteristics.lock().ok().and_then(|m| m.get(id).cloned())
                else {
                    return;
                };
                hub.log(|| {
                    format!(
                        "services: write {}, notify {}",
                        write.service_uuid, notify.service_uuid
                    )
                });
                let Ok(peripheral) = self.adapter.peripheral(id).await else { return };
                if !self.streams.contains_key(id) {
                    // One reader per connection, started before notifications are enabled.
                    let (p, tx, id2, want) =
                        (peripheral.clone(), self.tx.clone(), id.clone(), notify.uuid);
                    let reader = tokio::spawn(async move {
                        let Ok(mut stream) = p.notifications().await else { return };
                        while let Some(n) = stream.next().await {
                            if n.uuid == want {
                                let _ = tx.send(
                                    Input::Notification { id: id2.clone(), data: n.value }.into(),
                                );
                            }
                        }
                    });
                    self.streams.insert(id.clone(), reader);
                }
                hub.log(|| "enabling notifications".into());
                tokio::spawn(async move {
                    let _ = peripheral.subscribe(&notify).await;
                });
            }
            Output::Write { id, data } => {
                let Some((write, _)) =
                    self.characteristics.lock().ok().and_then(|m| m.get(id).cloned())
                else {
                    return;
                };
                hub.log(|| format!("init command {data:02X?}"));
                let (adapter, id, data) = (self.adapter.clone(), id.clone(), data.clone());
                tokio::spawn(async move {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        let _ = p.write(&write, &data, WriteType::WithoutResponse).await;
                    }
                });
            }
            Output::Status { .. } | Output::Report { .. } | Output::Paired { .. } => {}
        }
    }
}

#[cfg(target_os = "linux")]
impl Driver {
    /// Carries out the link commands over a direct ATT socket; false for
    /// anything else, which btleplug handles.
    fn execute_direct<S: Sink>(
        &mut self,
        output: &Output<PeripheralId>,
        hub: &Hub<PeripheralId, S>,
    ) -> bool {
        use crate::l2cap::Command;
        let send = |links: &HashMap<PeripheralId, DirectLink>, id, command| {
            if let Some(link) = links.get(id) {
                let _ = link.commands.send(command);
            }
        };
        match output {
            Output::Connect(id) => {
                hub.log(|| format!("connecting to {id} over a direct ATT link"));
                let (commands, rx) = unbounded_channel();
                let task = tokio::spawn(direct_link(
                    self.adapter.clone(),
                    id.clone(),
                    rx,
                    self.tx.clone(),
                ));
                if let Some(old) = self.links.insert(id.clone(), DirectLink { task, commands }) {
                    old.task.abort();
                }
            }
            Output::Disconnect(id) => {
                hub.log(|| format!("disconnecting {id}"));
                let Some(link) = self.links.remove(id) else { return true };
                link.task.abort();
                // The aborted task cannot say so itself, and the receiver waits for it.
                let _ = self.tx.send(Input::Disconnected(id.clone()).into());
                // BlueZ may still hold the LE link, and a linked Joy-Con stops advertising.
                let (adapter, id) = (self.adapter.clone(), id.clone());
                tokio::spawn(async move {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        let _ = tokio::time::timeout(Duration::from_secs(2), p.disconnect()).await;
                    }
                });
            }
            Output::DiscoverServices(id) => send(&self.links, id, Command::Discover),
            Output::Subscribe(id) => {
                hub.log(|| "enabling notifications".into());
                send(&self.links, id, Command::Subscribe);
            }
            Output::Write { id, data } => {
                hub.log(|| format!("init command {data:02X?}"));
                send(&self.links, id, Command::Write(data.clone()));
            }
            _ => return false,
        }
        true
    }
}

/// BlueZ's disconnect signal, as receiver input. A direct link reports its
/// own end from the socket, and a late signal from BlueZ (after a connect
/// retry, say) would make the receiver forget a link that is still open.
#[cfg(target_os = "linux")]
fn bluez_disconnected<Id: Eq + Hash, L>(id: Id, links: &HashMap<Id, L>) -> Option<Input<Id>> {
    (!links.contains_key(&id)).then_some(Input::Disconnected(id))
}

/// Connects to `id` directly and runs the link until it ends, reporting
/// every step as receiver input.
#[cfg(target_os = "linux")]
async fn direct_link(
    adapter: Adapter,
    id: PeripheralId,
    commands: UnboundedReceiver<crate::l2cap::Command>,
    tx: UnboundedSender<Back<PeripheralId>>,
) {
    use crate::att::{Client, Event, uuid_le};
    use btleplug::api::AddressType;
    let socket = async {
        let peripheral = adapter.peripheral(&id).await.map_err(|e| e.to_string())?;
        let props = peripheral.properties().await.map_err(|e| e.to_string())?;
        let props = props.ok_or("its address is unknown")?;
        let random = props.address_type == Some(AddressType::Random);
        let address = props.address.into_inner();
        let log = |m| drop(tx.send(Back::Log(format!("{id}: {m}"))));
        crate::l2cap::connect_retrying(|| crate::l2cap::connect(address, random), log)
            .await
            .map_err(|e| e.to_string())
    };
    let socket = match socket.await {
        Ok(socket) => socket,
        Err(e) => {
            let _ = tx.send(Back::Log(format!("could not connect to {id}: {e}")));
            let _ = tx.send(Input::ConnectFailed(id).into());
            return;
        }
    };
    let _ = tx.send(Input::Connected(id.clone()).into());
    let u = |s| uuid_le(s).unwrap_or_default();
    let client = Client::new(u(SERVICE), u(NOTIFY_CHARACTERISTIC), u(WRITE_CHARACTERISTIC));
    let why = crate::l2cap::run(socket, client, commands, |event| {
        let back = match event {
            Event::Discovered { write, notify } => {
                Input::CharacteristicsFound { id: id.clone(), write, notify }.into()
            }
            Event::Notification(data) => Input::Notification { id: id.clone(), data }.into(),
            Event::Log(message) => Back::Log(format!("{id}: {message}")),
        };
        let _ = tx.send(back);
    })
    .await;
    let _ = tx.send(Back::Log(format!("link to {id} ended: {why}")));
    let _ = tx.send(Input::Disconnected(id).into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use deskpuck_inject::RecordingSink;

    #[cfg(target_os = "linux")]
    #[test]
    fn bluez_cannot_end_a_direct_link_the_receiver_still_holds() {
        let links = HashMap::from([(7u32, ())]);
        assert_eq!(bluez_disconnected(7, &links), None);
        assert_eq!(bluez_disconnected(8, &links), Some(Input::Disconnected(8)), "control");

        // The race: a stale BlueZ signal arriving after the link came up.
        let mut hub = hub_logging_to(Arc::default());
        hub.start_pairing(0.0);
        hub.start(0.0);
        hub.input(Input::AdapterPoweredOn, 0.0);
        hub.input(Input::Discovered { id: 7, name: None, manufacturer_ids: vec![0x0553] }, 0.1);
        hub.input(Input::Connected(7), 1.0);
        if let Some(input) = bluez_disconnected(7, &links) {
            hub.input(input, 1.1);
        }
        assert_eq!(hub.linked(), Some(&7));
    }

    fn hub_logging_to(log: Arc<Mutex<Vec<String>>>) -> Hub<u32, RecordingSink> {
        let hooks = Hooks {
            status: Box::new(|_, _| {}),
            report: None,
            log: Some(Box::new(move |m: &str| log.lock().unwrap().push(m.to_owned()))),
            error: Box::new(|m| panic!("unexpected error: {m}")),
            latched: None,
        };
        Hub::new(EngineSettings::default(), RecordingSink::default(), hooks, None)
    }

    #[test]
    fn task_failures_reach_the_verbose_log_and_inputs_reach_the_receiver() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut hub = hub_logging_to(log.clone());
        let out = deliver(&mut hub, Back::Log("could not connect to 7: timed out".into()), 0.0);
        assert!(out.is_empty());
        assert_eq!(*log.lock().unwrap(), ["could not connect to 7: timed out"]);

        // A pairing window gives powering on something to do: start scanning.
        let mut direct = hub_logging_to(Arc::default());
        direct.start_pairing(0.0);
        direct.start(0.0);
        let want = direct.input(Input::AdapterPoweredOn, 0.0);
        assert_eq!(want, [Output::StartScan], "control");
        hub.start_pairing(0.0);
        hub.start(0.0);
        assert_eq!(deliver(&mut hub, Input::AdapterPoweredOn.into(), 0.0), want);
    }
}
