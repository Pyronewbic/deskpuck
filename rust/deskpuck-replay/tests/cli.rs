use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_deskpuck-replay");
const FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/joycon2_r_capture.txt");

fn replay(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("binary runs")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn usage_errors_exit_2() {
    for args in
        [&["--bogus"][..], &["--capture"], &["--dry-run", "--config", "/nonexistent/config.json"]]
    {
        let out = replay(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
    let help = replay(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(stderr(&help).contains("Usage: deskpuck-replay"));
}

#[test]
fn bad_capture_exits_2_before_posting() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bad = dir.path().join("bad.txt");
    std::fs::write(&bad, "zz\n").expect("write");
    let out = replay(&["--dry-run", "--capture", bad.to_str().expect("utf-8 path")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("line 1: not hex"));
    assert!(out.stdout.is_empty());
}

#[test]
fn dry_run_reports_what_it_posted() {
    let out = replay(&["--dry-run"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let lines = String::from_utf8_lossy(&out.stdout).lines().count();
    let summary = stderr(&out);
    // The summary counts what went through the sink, so it matches the printed lines.
    assert!(summary.contains(&format!("posted {lines} events")), "{summary}");
    assert!(summary.contains(" 0 buttons"), "{summary}");

    let resting = replay(&["--dry-run", "--capture", FIXTURE]);
    assert_eq!(resting.status.code(), Some(0));
    assert!(stderr(&resting).contains("replayed 18 reports (0 too short), posted 0 events"));
}

#[test]
fn closed_stdout_fails_cleanly_instead_of_panicking() {
    let mut child = Command::new(BIN)
        .arg("--dry-run")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    // Close the read end before the child writes anything.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("child exits");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Could not write the event list"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}
