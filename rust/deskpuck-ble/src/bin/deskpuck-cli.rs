//! Connects to a Joy-Con 2 over Bluetooth LE and drives the pointer, like the
//! Mac app without a menu bar icon. All the work happens in `controller`; this
//! file only parses options and prints.

use deskpuck_ble::Monitor;
use deskpuck_ble::controller::{
    Controller, Hooks, LinkStatus, MessageHook, PairingSetup, ReportHook,
};
use deskpuck_ble::receiver::PAIRING_WINDOW;
use deskpuck_core::config::Config;
use deskpuck_core::engine::EngineSettings;
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::pairing::PairedDevice;
use deskpuck_inject::InjectError;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

/// Status output that never panics on a closed stream.
macro_rules! note {
    ($($arg:tt)*) => {{
        let _ = writeln!(std::io::stderr().lock(), $($arg)*);
    }};
}

const USAGE: &str = "\
Usage: deskpuck-cli [--pair] [--config PATH] [--verbose] [--monitor]

Connects to a Joy-Con 2 over Bluetooth and uses it as a mouse and keyboard.
Only the paired Joy-Con connects; pair one first with --pair. Hold SYNC on
the Joy-Con to connect. Ctrl+C releases any held input and quits.
Quit the Deskpuck app first: only one program can connect at a time. The
app and this tool share the pairing.

  --pair         for 60 seconds, connect to any Joy-Con and pair it,
                 replacing the paired one; then carry on as usual
  --config PATH  settings file (default: the Deskpuck app's config.json)
  --verbose      print Bluetooth connection detail
  --monitor      show a live readout of every Joy-Con report

Exit status: 0 quit (Ctrl+C, the terminal closed, or SIGTERM), 1 Bluetooth
failed, 2 bad usage or config path, or no Joy-Con paired, 3 the OS refused
Bluetooth or input injection (permission needed).";

struct Args {
    config: Option<PathBuf>,
    pair: bool,
    verbose: bool,
    monitor: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { config: None, pair: false, verbose: false, monitor: false };
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--config") => {
                args.config = Some(it.next().ok_or("--config needs a path")?.into())
            }
            Some("--pair") => args.pair = true,
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

const NOT_PAIRED: &str =
    "No Joy-Con is paired. Run deskpuck-cli --pair and hold SYNC on the Joy-Con.";

/// Without --pair there must be a usable pairing, or nothing could connect.
fn check_paired(pair: bool, file: Option<&std::path::Path>) -> Result<(), String> {
    if pair {
        return Ok(());
    }
    match file.map(PairedDevice::load) {
        Some(Ok(Some(_))) => Ok(()),
        Some(Err(problem)) => Err(format!("{problem}.\n{NOT_PAIRED}")),
        Some(Ok(None)) | None => Err(NOT_PAIRED.into()),
    }
}

fn status_line(status: LinkStatus, name: Option<&str>) -> String {
    let name = name.unwrap_or("Joy-Con");
    match status {
        LinkStatus::BluetoothOff => "Bluetooth is off".into(),
        LinkStatus::BluetoothUnauthorized => {
            "Bluetooth access is off for this terminal app. Turn it on in System \
             Settings > Privacy & Security > Bluetooth, then run again."
                .into()
        }
        LinkStatus::Unavailable => "Bluetooth is unavailable".into(),
        LinkStatus::NotPaired => NOT_PAIRED.into(),
        LinkStatus::Pairing => {
            format!("Pairing for {PAIRING_WINDOW:.0} seconds: hold SYNC on the Joy-Con to use")
        }
        LinkStatus::Searching => "Searching: hold SYNC on the paired Joy-Con".into(),
        LinkStatus::InUseElsewhere => {
            "The paired Joy-Con is in use by another app; quit it to connect here".into()
        }
        LinkStatus::Connecting => format!("Connecting to {name}..."),
        LinkStatus::Connected => format!("Connected to {name}"),
    }
}

/// Resolves on Ctrl+C, a closed terminal (SIGHUP) or SIGTERM, so each of them
/// releases held input instead of killing the process with a key still down.
fn quit_signals() -> std::io::Result<impl Future<Output = ()>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut hangup = signal(SignalKind::hangup())?;
        let mut terminate = signal(SignalKind::terminate())?;
        Ok(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = hangup.recv() => {}
                _ = terminate.recv() => {}
            }
        })
    }
    #[cfg(not(unix))]
    Ok(async {
        let _ = tokio::signal::ctrl_c().await;
    })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(problem) => {
            if !problem.is_empty() {
                note!("deskpuck-cli: {problem}\n");
            }
            note!("{USAGE}");
            return ExitCode::from(if problem.is_empty() { 0 } else { 2 });
        }
    };
    let settings = match load_settings(args.config.clone()) {
        Ok(settings) => settings,
        Err(problem) => {
            note!("deskpuck-cli: {problem}");
            return ExitCode::from(2);
        }
    };
    let pairing_file = PairedDevice::default_path();
    if let Err(problem) = check_paired(args.pair, pairing_file.as_deref()) {
        note!("deskpuck-cli: {problem}");
        return ExitCode::from(2);
    }
    // Checked before Bluetooth: without it every event would be dropped silently.
    let sink = match deskpuck_inject::platform_sink() {
        Ok(sink) => sink,
        Err(e) => {
            note!("deskpuck-cli: {e}");
            return ExitCode::from(if matches!(e, InjectError::NotPermitted(_)) { 3 } else { 1 });
        }
    };

    // A fatal status ends the run with its exit code; a quit signal ends it with 0.
    let (exit_tx, exit_rx) = mpsc::channel::<u8>();
    let quit_tx = exit_tx.clone();
    let monitor_mode = args.monitor;
    let mut monitor = Monitor::default();
    let hooks = Hooks {
        status: Box::new(move |status, name| {
            // Unavailable is explained by the error hook just before it.
            if status != LinkStatus::Unavailable
                && (!monitor_mode || status != LinkStatus::Connected)
            {
                note!("{}", status_line(status, name));
            }
            match status {
                LinkStatus::BluetoothUnauthorized => drop(exit_tx.send(3)),
                LinkStatus::Unavailable => drop(exit_tx.send(1)),
                // The pairing window closed with nothing paired.
                LinkStatus::NotPaired => drop(exit_tx.send(2)),
                _ => {}
            }
        }),
        report: args.monitor.then(|| {
            Box::new(move |report: &_, data: &[u8], name: Option<&str>, elapsed: u128| {
                let screen = monitor.screen(name, elapsed, report, data);
                let _ = std::io::stdout().lock().write_all(screen.as_bytes());
            }) as ReportHook
        }),
        log: args.verbose.then(|| Box::new(|message: &str| note!("ble: {message}")) as MessageHook),
        latched: Some(Box::new(|modifiers: Modifiers| {
            if modifiers.is_empty() {
                note!("Latched modifiers released");
            } else {
                note!("Latched: {}", modifiers.names().join("+"));
            }
        })),
        error: Box::new(|message| note!("deskpuck-cli: {message}")),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            note!("deskpuck-cli: could not start: {e}");
            return ExitCode::from(1);
        }
    };
    let quit = match runtime.block_on(async { quit_signals() }) {
        Ok(quit) => quit,
        Err(e) => {
            note!("deskpuck-cli: could not start: {e}");
            return ExitCode::from(1);
        }
    };
    runtime.spawn(async move {
        quit.await;
        let _ = quit_tx.send(0);
    });
    let pairing = PairingSetup { file: pairing_file, pair_at_start: args.pair };
    let controller = match Controller::start(settings, sink, hooks, pairing) {
        Ok(controller) => controller,
        Err(e) => {
            note!("deskpuck-cli: could not start: {e}");
            return ExitCode::from(1);
        }
    };
    let code = runtime
        .block_on(async { tokio::task::spawn_blocking(move || exit_rx.recv().unwrap_or(1)).await })
        .unwrap_or(1);
    // Dropping the controller releases held input and disconnects.
    drop(controller);
    ExitCode::from(code)
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn hangup_and_terminate_request_a_quit() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        for sig in ["-HUP", "-TERM"] {
            runtime.block_on(async {
                let mut quit = Box::pin(super::quit_signals().expect("signal handlers"));
                // Control: nothing resolves before the signal arrives.
                let early = tokio::time::timeout(Duration::from_millis(100), &mut quit).await;
                assert!(early.is_err(), "{sig} quit resolved with no signal");
                let pid = std::process::id().to_string();
                assert!(Command::new("kill").args([sig, &pid]).status().unwrap().success());
                tokio::time::timeout(Duration::from_secs(5), quit).await.expect(sig);
            });
        }
    }
}
