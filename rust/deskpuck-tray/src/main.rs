//! Deskpuck's tray icon for Linux and Windows; the Mac app is Sources/Deskpuck.
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
mod watch;

#[cfg(any(target_os = "linux", windows))]
fn main() -> std::process::ExitCode {
    app::run()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn main() -> std::process::ExitCode {
    eprintln!("deskpuck-tray runs on Linux and Windows; on macOS use the Deskpuck app.");
    std::process::ExitCode::from(2)
}
