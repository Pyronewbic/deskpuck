//! Deskpuck for Linux and Windows: the tray icon, or with --settings its
//! settings window. The Mac app is Sources/Deskpuck.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(any(target_os = "linux", windows))]
mod app;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod hook;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod icon;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod model;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod reload;
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod watch;

const USAGE: &str = "Usage: deskpuck [--settings [--config PATH]]";

/// What the command line asks for; `None` for anything else.
#[derive(Debug, PartialEq)]
enum Mode {
    Tray,
    Settings(Option<std::path::PathBuf>),
}

fn mode(args: &[std::ffi::OsString]) -> Option<Mode> {
    match args.split_first() {
        None => Some(Mode::Tray),
        Some((first, rest)) if first == "--settings" => {
            deskpuck_settings::config_path(rest).map(Mode::Settings)
        }
        Some(_) => None,
    }
}

#[cfg(any(target_os = "linux", windows))]
fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match mode(&args) {
        Some(Mode::Tray) => app::run(),
        Some(Mode::Settings(path)) => match deskpuck_settings::window::run(path) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(problem) => {
                eprintln!("deskpuck: {problem}");
                std::process::ExitCode::from(1)
            }
        },
        None => {
            eprintln!("{USAGE}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn main() -> std::process::ExitCode {
    let _ = (USAGE, mode(&[]));
    eprintln!("deskpuck runs on Linux and Windows; on macOS use the Deskpuck app.");
    std::process::ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_start_the_tray_and_settings_opens_the_window() {
        assert_eq!(mode(&args(&[])), Some(Mode::Tray));
        assert_eq!(
            mode(&args(&["--settings", "--config", "/tmp/c.json"])),
            Some(Mode::Settings(Some("/tmp/c.json".into())))
        );
        assert!(matches!(mode(&args(&["--settings"])), Some(Mode::Settings(_))));
        for bad in
            [&["--config", "x"][..], &["--settings", "--config"], &["--settings", "x"], &["-h"]]
        {
            assert_eq!(mode(&args(bad)), None, "{bad:?}");
        }
    }
}
