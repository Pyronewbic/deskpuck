use deskpuck_core::config::Config;
use deskpuck_core::engine::EngineSettings;
use deskpuck_inject::{InjectError, InputEvent, Sink};
use deskpuck_replay::{Frame, Summary, demo, read_capture, run};
use std::cell::Cell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Status output that never panics: a closed stderr must not crash the replay.
macro_rules! note {
    ($($arg:tt)*) => {{
        let _ = writeln!(std::io::stderr().lock(), $($arg)*);
    }};
}

const USAGE: &str = "\
Usage: deskpuck-replay [--capture PATH] [--config PATH] [--dry-run]

Replays Joy-Con 2 reports through the Deskpuck engine and posts the result as
real pointer, scroll and key input. Without --capture it plays a built-in demo:
a square, a push into the bottom screen edge, a scroll, and arrow-key taps.
The demo never clicks and never presses Return.

  --capture PATH  replay a capture file (hex report per line)
  --config PATH   use this config.json instead of the built-in defaults
  --dry-run       print the events instead of posting them

Exit status: 0 done, 1 injection failed, 2 bad usage or input,
3 the OS refused input injection (permission needed).";

const MAX_CAPTURE_BYTES: u64 = 4 * 1024 * 1024;
const COUNTDOWN_SECONDS: u64 = 3;

struct Args {
    capture: Option<PathBuf>,
    config: Option<PathBuf>,
    dry_run: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { capture: None, config: None, dry_run: false };
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--capture") => {
                args.capture = Some(it.next().ok_or("--capture needs a path")?.into())
            }
            Some("--config") => {
                args.config = Some(it.next().ok_or("--config needs a path")?.into())
            }
            Some("--dry-run") => args.dry_run = true,
            Some("--help" | "-h") => return Err(String::new()),
            _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
        }
    }
    Ok(args)
}

fn load_frames(capture: Option<&PathBuf>) -> Result<Vec<Frame>, String> {
    let Some(path) = capture else { return Ok(demo()) };
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut text = String::new();
    file.take(MAX_CAPTURE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if text.len() as u64 > MAX_CAPTURE_BYTES {
        return Err(format!("{}: larger than 4 MB", path.display()));
    }
    read_capture(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn load_settings(path: Option<&PathBuf>) -> Result<EngineSettings, String> {
    let Some(path) = path else { return Ok(EngineSettings::default()) };
    // Config::load treats a missing file as "use defaults"; an explicit path must exist.
    if !path.exists() {
        return Err(format!("{}: no such file", path.display()));
    }
    let (config, warnings) = Config::load(path);
    for warning in warnings {
        note!("config: {warning}");
    }
    Ok(config.engine_settings())
}

struct PrintSink<'a> {
    at: &'a Cell<f64>,
}

impl Sink for PrintSink<'_> {
    fn post(&mut self, event: &InputEvent) -> Result<(), InjectError> {
        writeln!(std::io::stdout().lock(), "{:7.3}  {event:?}", self.at.get())
            .map_err(|e| InjectError::Failed(format!("Could not write the event list: {e}")))
    }
}

#[cfg(target_os = "macos")]
fn live_sink() -> Result<Box<dyn Sink>, InjectError> {
    Ok(Box::new(deskpuck_inject::macos::MacSink::new()?))
}

#[cfg(target_os = "linux")]
fn live_sink() -> Result<Box<dyn Sink>, InjectError> {
    Ok(Box::new(deskpuck_inject::linux::LinuxSink::new()?))
}

#[cfg(windows)]
fn live_sink() -> Result<Box<dyn Sink>, InjectError> {
    Ok(Box::new(deskpuck_inject::windows::WindowsSink::new()?))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn live_sink() -> Result<Box<dyn Sink>, InjectError> {
    Err(InjectError::Failed("Live injection is not available on this OS; use --dry-run.".into()))
}

/// Mapped keys this OS cannot type; the sink skips them, so say so up front.
fn untranslatable_keys(settings: &EngineSettings) -> Vec<u16> {
    if cfg!(target_os = "macos") {
        return Vec::new();
    }
    settings
        .key_mappings
        .iter()
        .map(|m| m.key_code)
        .filter(|&code| deskpuck_inject::keymap::lookup(code).is_none())
        .collect()
}

fn report(summary: &Summary) {
    note!(
        "replayed {} reports ({} too short), posted {} events: {} moves, {} buttons, {} scrolls, {} keys",
        summary.reports,
        summary.skipped,
        summary.events(),
        summary.moves,
        summary.buttons,
        summary.scrolls,
        summary.keys
    );
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(problem) => {
            if !problem.is_empty() {
                note!("deskpuck-replay: {problem}\n");
            }
            note!("{USAGE}");
            return ExitCode::from(if problem.is_empty() { 0 } else { 2 });
        }
    };
    let (frames, settings) = match load_frames(args.capture.as_ref())
        .and_then(|f| Ok((f, load_settings(args.config.as_ref())?)))
    {
        Ok(loaded) => loaded,
        Err(problem) => {
            note!("deskpuck-replay: {problem}");
            return ExitCode::from(2);
        }
    };

    for code in untranslatable_keys(&settings) {
        note!("deskpuck-replay: key code {code} has no equivalent on this OS and will be skipped");
    }

    let result = if args.dry_run {
        let at = Cell::new(0.0);
        run(&frames, settings, &mut PrintSink { at: &at }, &mut |t| at.set(t))
    } else {
        let mut sink = match live_sink() {
            Ok(sink) => sink,
            Err(e) => {
                note!("deskpuck-replay: {e}");
                return ExitCode::from(if matches!(e, InjectError::NotPermitted(_)) {
                    3
                } else {
                    2
                });
            }
        };
        let seconds = frames.last().map_or(0.0, |f| f.at);
        note!("Posting real input for {seconds:.1} s in {COUNTDOWN_SECONDS} s. Ctrl+C to cancel.");
        for left in (1..=COUNTDOWN_SECONDS).rev() {
            note!("  {left}...");
            std::thread::sleep(Duration::from_secs(1));
        }
        let start = Instant::now();
        let mut pace = |at: f64| {
            let due = start + Duration::from_secs_f64(at);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        };
        run(&frames, settings, sink.as_mut(), &mut pace)
    };

    match result {
        Ok(summary) => {
            report(&summary);
            ExitCode::SUCCESS
        }
        Err(e) => {
            note!("deskpuck-replay: {e}");
            ExitCode::from(if matches!(e, InjectError::NotPermitted(_)) { 3 } else { 1 })
        }
    }
}
