//! The daily update check: the system's curl asks for the latest-release redirect,
//! and deskpuck-core decides whether it names a newer version.

use deskpuck_core::update::{LATEST_URL, allowed_by_config, evaluate};
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const MAX_OUTPUT: u64 = 1024;
const FIRST_CHECK: (u64, u64) = (30, 120);
const DAY: u64 = 24 * 60 * 60;
const JITTER: u64 = 60 * 60;

/// Checks after a jittered delay, then daily, when the settings file allows it;
/// `found` gets each newer version once.
pub fn watch(config: Option<PathBuf>, found: impl Fn(String) + Send + 'static) {
    std::thread::spawn(move || {
        let curl = curl();
        let mut wait = FIRST_CHECK.0 + random(FIRST_CHECK.1 - FIRST_CHECK.0);
        let mut last = None;
        loop {
            std::thread::sleep(Duration::from_secs(wait));
            wait = DAY + random(JITTER);
            if !config.as_deref().is_none_or(allowed_by_config) {
                continue;
            }
            if let Some(version) = check(&curl, env!("CARGO_PKG_VERSION"))
                && last.as_ref() != Some(&version)
            {
                last = Some(version.clone());
                found(version);
            }
        }
    });
}

/// The newer version, or None for anything else, including any failure.
pub fn check(curl: &Path, current: &str) -> Option<String> {
    let mut command = Command::new(curl);
    command
        .args(["-q", "-fsSI", "--proto", "=https", "--max-redirs", "0", "-m", "10"])
        .args(["-A", &format!("Deskpuck/{current}")])
        .args(["-o", NULL, "-w", "%{redirect_url}", LATEST_URL])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    let mut child = command.spawn().ok()?;
    let mut output = Vec::new();
    let read = child.stdout.take()?.take(MAX_OUTPUT + 1).read_to_end(&mut output);
    if read.is_err() || output.len() as u64 > MAX_OUTPUT {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    if !child.wait().ok()?.success() {
        return None;
    }
    evaluate(std::str::from_utf8(&output).ok()?, current)
}

/// Installed under /usr means a package manager put it there and owns updates.
pub fn package_managed(program: &Path) -> bool {
    cfg!(target_os = "linux") && program.starts_with("/usr")
}

#[cfg(windows)]
const NULL: &str = "NUL";
#[cfg(not(windows))]
const NULL: &str = "/dev/null";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// By absolute path on Windows, so a curl.exe earlier on PATH cannot stand in.
fn curl() -> PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        Path::new(&root).join("System32").join("curl.exe")
    } else {
        PathBuf::from("curl")
    }
}

fn random(below: u64) -> u64 {
    RandomState::new().hash_one(std::process::id()) % below.max(1)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in curl that records its arguments, prints `output` and exits with `status`.
    fn fake_curl(dir: &Path, output: &str, status: i32) -> PathBuf {
        let program = dir.join("curl");
        let args = dir.join("args");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s' '{output}'\nexit {status}\n",
                args.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    const NEWER: &str = "https://github.com/Pyronewbic/deskpuck/releases/tag/v0.4.0";

    #[test]
    fn a_newer_release_is_found_with_a_locked_down_request() {
        let dir = tempfile::tempdir().unwrap();
        let curl = fake_curl(dir.path(), NEWER, 0);
        assert_eq!(check(&curl, "0.3.0"), Some("0.4.0".into()));
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(args.first(), Some(&"-q"), "-q must come first to skip .curlrc: {args:?}");
        for expected in ["--proto", "=https", "--max-redirs", "Deskpuck/0.3.0", LATEST_URL] {
            assert!(args.contains(&expected), "{expected} missing from {args:?}");
        }
    }

    #[test]
    fn anything_but_a_clean_newer_answer_is_nothing() {
        let dir = tempfile::tempdir().unwrap();
        for (output, status) in [
            (NEWER, 22),
            ("https://github.com/Attacker/deskpuck/releases/tag/v9.9.9", 0),
            ("https://github.com/Pyronewbic/deskpuck/releases/tag/v0.3.0", 0),
            ("", 0),
        ] {
            let curl = fake_curl(dir.path(), output, status);
            assert_eq!(check(&curl, "0.3.0"), None, "{output:?} exit {status}");
        }
        assert_eq!(check(&dir.path().join("no-such-curl"), "0.3.0"), None);
    }

    #[test]
    fn endless_output_is_cut_off_at_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("curl");
        std::fs::write(&program, "#!/bin/sh\nexec yes\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || tx.send(check(&program, "0.3.0")));
        let answer =
            rx.recv_timeout(Duration::from_secs(10)).expect("check returns despite endless output");
        assert_eq!(answer, None);
    }

    #[test]
    fn only_linux_installs_under_usr_are_package_managed() {
        assert_eq!(package_managed(Path::new("/usr/bin/deskpuck")), cfg!(target_os = "linux"));
        assert!(!package_managed(Path::new("/home/me/.local/bin/deskpuck")));
        assert!(!package_managed(Path::new("/usrlocal/deskpuck")));
    }
}
