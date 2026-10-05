//! Connects to a Joy-Con 2 over Bluetooth LE and drives the pointer, like the
//! Mac app's deskpuck-cli. The protocol logic is the tested state machine in
//! `receiver`; this file only carries its commands out through btleplug.

use btleplug::api::{
    Central, CentralEvent, CentralState, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use deskpuck_ble::receiver::{
    Input, NOTIFY_CHARACTERISTIC, Output, Receiver, Status, WRITE_CHARACTERISTIC,
};
use deskpuck_ble::{Monitor, Session};
use deskpuck_core::config::Config;
use deskpuck_core::engine::EngineSettings;
use deskpuck_inject::InjectError;
use futures::StreamExt;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Status output that never panics on a closed stream.
macro_rules! note {
    ($($arg:tt)*) => {{
        let _ = writeln!(std::io::stderr().lock(), $($arg)*);
    }};
}

const USAGE: &str = "\
Usage: deskpuck-blecli [--config PATH] [--verbose] [--monitor]

Connects to a Joy-Con 2 over Bluetooth and uses it as a mouse and keyboard.
Hold SYNC on the Joy-Con to connect. Ctrl+C releases any held input and quits.
Quit the Deskpuck app first: only one program can connect at a time.

  --config PATH  settings file (default: the Deskpuck app's config.json)
  --verbose      print Bluetooth connection detail
  --monitor      show a live readout of every Joy-Con report

Exit status: 0 quit with Ctrl+C, 1 Bluetooth failed, 2 bad usage or config
path, 3 the OS refused Bluetooth or input injection (permission needed).";

struct Args {
    config: Option<PathBuf>,
    verbose: bool,
    monitor: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { config: None, verbose: false, monitor: false };
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--config") => {
                args.config = Some(it.next().ok_or("--config needs a path")?.into())
            }
            Some("--verbose") => args.verbose = true,
            Some("--monitor") => args.monitor = true,
            Some("--help" | "-h") => return Err(String::new()),
            _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
        }
    }
    Ok(args)
}

fn load_settings(path: Option<PathBuf>) -> Result<EngineSettings, String> {
    let explicit = path.is_some();
    let Some(path) = path.or_else(Config::default_path) else {
        return Ok(EngineSettings::default());
    };
    // Config::load treats a missing file as "use defaults"; an explicit path must exist.
    if explicit && !path.exists() {
        return Err(format!("{}: no such file", path.display()));
    }
    let (config, warnings) = Config::load(&path);
    for warning in warnings {
        note!("config: {warning}");
    }
    Ok(config.engine_settings())
}

fn status_line(status: Status, name: Option<&str>) -> String {
    let name = name.unwrap_or("Joy-Con");
    match status {
        Status::BluetoothOff => "Bluetooth is off".into(),
        Status::Searching => "Searching: hold SYNC on the Joy-Con".into(),
        Status::Connecting => format!("Connecting to {name}..."),
        Status::Connected => format!("Connected to {name}"),
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(problem) => {
            if !problem.is_empty() {
                note!("deskpuck-blecli: {problem}\n");
            }
            note!("{USAGE}");
            return ExitCode::from(if problem.is_empty() { 0 } else { 2 });
        }
    };
    let settings = match load_settings(args.config.clone()) {
        Ok(settings) => settings,
        Err(problem) => {
            note!("deskpuck-blecli: {problem}");
            return ExitCode::from(2);
        }
    };
    // Checked before Bluetooth: without it every event would be dropped silently.
    let mut sink = match deskpuck_inject::platform_sink() {
        Ok(sink) => sink,
        Err(e) => {
            note!("deskpuck-blecli: {e}");
            return ExitCode::from(if matches!(e, InjectError::NotPermitted(_)) { 3 } else { 1 });
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            note!("deskpuck-blecli: could not start: {e}");
            return ExitCode::from(1);
        }
    };
    runtime.block_on(run(&args, Session::new(settings, sink.as_mut())))
}

/// The two characteristics of a linked Joy-Con, found during service discovery.
type Characteristics = Arc<Mutex<HashMap<PeripheralId, (Characteristic, Characteristic)>>>;

struct Driver {
    adapter: Adapter,
    tx: UnboundedSender<Input<PeripheralId>>,
    characteristics: Characteristics,
    streams: HashMap<PeripheralId, JoinHandle<()>>,
    verbose: bool,
}

fn uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or_default()
}

impl Driver {
    fn log(&self, message: impl FnOnce() -> String) {
        if self.verbose {
            note!("ble: {}", message());
        }
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
                        let _ = tx.send(Input::Discovered {
                            id,
                            name: props.local_name,
                            manufacturer_ids,
                        });
                    }
                });
                None
            }
            CentralEvent::DeviceDisconnected(id) => {
                if let Some(stream) = self.streams.remove(&id) {
                    stream.abort();
                }
                Some(Input::Disconnected(id))
            }
            CentralEvent::StateUpdate(CentralState::PoweredOn) => Some(Input::AdapterPoweredOn),
            CentralEvent::StateUpdate(CentralState::PoweredOff) => Some(Input::AdapterPoweredOff),
            _ => None,
        }
    }

    /// Carries out one receiver command. Slow operations run as tasks that
    /// report back through `tx`, so the main loop never blocks.
    async fn execute(&mut self, output: &Output<PeripheralId>) {
        match output {
            Output::StartScan => {
                self.log(|| "scanning".into());
                if let Err(e) = self.adapter.start_scan(ScanFilter::default()).await {
                    note!("deskpuck-blecli: could not scan: {e}");
                }
            }
            Output::StopScan => {
                let _ = self.adapter.stop_scan().await;
            }
            Output::Connect(id) => {
                self.log(|| format!("connecting to {id}"));
                let (adapter, tx, id) = (self.adapter.clone(), self.tx.clone(), id.clone());
                tokio::spawn(async move {
                    let connected = match adapter.peripheral(&id).await {
                        Ok(p) => p.connect().await.is_ok(),
                        Err(_) => false,
                    };
                    let _ = tx.send(if connected {
                        Input::Connected(id)
                    } else {
                        Input::ConnectFailed(id)
                    });
                });
            }
            Output::Disconnect(id) => {
                self.log(|| format!("disconnecting {id}"));
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
                let verbose = self.verbose;
                tokio::spawn(async move {
                    let Ok(p) = adapter.peripheral(&id).await else { return };
                    if let Err(e) = p.discover_services().await
                        && verbose
                    {
                        note!("ble: service discovery failed: {e}");
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
                    let _ =
                        tx.send(Input::CharacteristicsFound { id, write: both.0, notify: both.1 });
                });
            }
            Output::Subscribe(id) => {
                let Some((_, notify)) =
                    self.characteristics.lock().ok().and_then(|m| m.get(id).cloned())
                else {
                    return;
                };
                let Ok(peripheral) = self.adapter.peripheral(id).await else { return };
                if !self.streams.contains_key(id) {
                    // One reader per connection, started before notifications are enabled.
                    let (p, tx, id2, want) =
                        (peripheral.clone(), self.tx.clone(), id.clone(), notify.uuid);
                    let reader = tokio::spawn(async move {
                        let Ok(mut stream) = p.notifications().await else { return };
                        while let Some(n) = stream.next().await {
                            if n.uuid == want {
                                let _ =
                                    tx.send(Input::Notification { id: id2.clone(), data: n.value });
                            }
                        }
                    });
                    self.streams.insert(id.clone(), reader);
                }
                self.log(|| "enabling notifications".into());
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
                self.log(|| format!("init command {:02X?}", data));
                let (adapter, id, data) = (self.adapter.clone(), id.clone(), data.clone());
                tokio::spawn(async move {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        let _ = p.write(&write, &data, WriteType::WithoutResponse).await;
                    }
                });
            }
            Output::Status { .. } | Output::Report { .. } => {}
        }
    }
}

async fn run(args: &Args, mut session: Session<'_>) -> ExitCode {
    let manager = match Manager::new().await {
        Ok(manager) => manager,
        Err(btleplug::Error::PermissionDenied) => {
            note!(
                "deskpuck-blecli: Bluetooth access is off for this terminal app. Turn it on in System Settings > \
                 Privacy & Security > Bluetooth, then run again."
            );
            return ExitCode::from(3);
        }
        Err(e) => {
            note!("deskpuck-blecli: Bluetooth is unavailable: {e}");
            return ExitCode::from(1);
        }
    };
    let Some(adapter) = manager.adapters().await.ok().and_then(|a| a.into_iter().next()) else {
        note!("deskpuck-blecli: no Bluetooth adapter found");
        return ExitCode::from(1);
    };
    let mut events = match adapter.events().await {
        Ok(events) => events,
        Err(e) => {
            note!("deskpuck-blecli: could not listen for Bluetooth events: {e}");
            return ExitCode::from(1);
        }
    };

    let (tx, mut rx) = unbounded_channel();
    let mut driver = Driver {
        adapter: adapter.clone(),
        tx,
        characteristics: Arc::default(),
        streams: HashMap::new(),
        verbose: args.verbose,
    };
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64();
    let mut receiver = Receiver::default();
    let mut monitor = Monitor::default();
    let mut connected_at = Instant::now();

    let mut outputs = receiver.start();
    match adapter.adapter_state().await {
        Ok(CentralState::PoweredOn) => {
            outputs.extend(receiver.handle(Input::AdapterPoweredOn, now()))
        }
        Ok(CentralState::PoweredOff) => {
            outputs.extend(receiver.handle(Input::AdapterPoweredOff, now()))
        }
        _ => {}
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        for output in &outputs {
            match output {
                Output::Status { status, name } => {
                    if *status == Status::Connected {
                        connected_at = Instant::now();
                    }
                    if !args.monitor || *status != Status::Connected {
                        note!("{}", status_line(*status, name.as_deref()));
                    }
                    if let Err(e) = session.status(*status) {
                        note!("deskpuck-blecli: {e}");
                    }
                }
                Output::Report { report, data } => {
                    if args.monitor {
                        let name = receiver.linked().and_then(|(_, n)| n);
                        let screen =
                            monitor.screen(name, connected_at.elapsed().as_millis(), report, data);
                        let _ = std::io::stdout().lock().write_all(screen.as_bytes());
                    }
                    if let Err(e) = session.report(report, now()) {
                        note!("deskpuck-blecli: {e}");
                    }
                }
                other => driver.execute(other).await,
            }
        }
        outputs = tokio::select! {
            _ = &mut ctrl_c => break,
            Some(event) = events.next() => match driver.on_event(event) {
                Some(input) => receiver.handle(input, now()),
                None => Vec::new(),
            },
            Some(input) = rx.recv() => receiver.handle(input, now()),
            _ = ticker.tick() => receiver.tick(now()),
        };
    }

    // Ctrl+C: release held input first, then let the Joy-Con go.
    if let Err(e) = session.shutdown() {
        note!("deskpuck-blecli: {e}");
    }
    let _ = adapter.stop_scan().await;
    if let Some((id, _)) = receiver.linked()
        && let Ok(p) = adapter.peripheral(id).await
    {
        let _ = tokio::time::timeout(Duration::from_secs(2), p.disconnect()).await;
    }
    ExitCode::SUCCESS
}
