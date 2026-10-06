//! Only paths that exit before Bluetooth or input injection start, so these
//! tests never touch hardware or trigger a permission prompt.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_deskpuck-cli");

fn cli(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("binary runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn help_exits_0() {
    let out = cli(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stderr(&out).contains("Usage: deskpuck-cli"));
}

#[test]
fn usage_errors_exit_2() {
    for args in [&["--bogus"][..], &["--config"], &["--mouse"]] {
        let out = cli(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
        assert!(stderr(&out).contains("Usage: deskpuck-cli"), "{args:?}");
    }
}

#[test]
fn missing_config_exits_2_before_bluetooth() {
    let out = cli(&["--config", "/nonexistent/deskpuck/config.json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("no such file"));
    // Positive control for "before Bluetooth": nothing about scanning was printed.
    assert!(!stderr(&out).contains("Searching"));
}

/// Runs with HOME in a temp dir, so the pairing file is one this test controls.
#[cfg(unix)]
fn cli_at_home(home: &std::path::Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .expect("binary runs")
}

#[cfg(unix)]
fn pairing_path(home: &std::path::Path) -> std::path::PathBuf {
    let dir = if cfg!(target_os = "macos") {
        "Library/Application Support/Deskpuck"
    } else {
        ".config/deskpuck"
    };
    home.join(dir).join("pairing.json")
}

#[cfg(unix)]
#[test]
fn not_paired_exits_2_before_bluetooth() {
    let home = tempfile::tempdir().unwrap();
    let out = cli_at_home(home.path(), &[]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("deskpuck-cli --pair"), "{}", stderr(&out));
    assert!(!stderr(&out).contains("Searching"));

    // A malformed file is named, proving the CLI read this HOME's pairing file.
    let path = pairing_path(home.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{").unwrap();
    let out = cli_at_home(home.path(), &[]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("pairing.json is not valid JSON"), "{}", stderr(&out));
    assert!(stderr(&out).contains("deskpuck-cli --pair"));
}

#[test]
fn help_mentions_pairing() {
    assert!(stderr(&cli(&["--help"])).contains("--pair"));
}
