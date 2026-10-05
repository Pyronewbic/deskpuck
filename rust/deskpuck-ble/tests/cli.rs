//! Only paths that exit before Bluetooth or input injection start, so these
//! tests never touch hardware or trigger a permission prompt.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_deskpuck-blecli");

fn blecli(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("binary runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn help_exits_0() {
    let out = blecli(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stderr(&out).contains("Usage: deskpuck-blecli"));
}

#[test]
fn usage_errors_exit_2() {
    for args in [&["--bogus"][..], &["--config"], &["--mouse"]] {
        let out = blecli(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains("Usage: deskpuck-blecli"), "{args:?}");
    }
}

#[test]
fn missing_config_exits_2_before_bluetooth() {
    let out = blecli(&["--config", "/nonexistent/deskpuck/config.json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("no such file"));
    // Positive control for "before Bluetooth": nothing about scanning was printed.
    assert!(!stderr(&out).contains("Searching"));
}
