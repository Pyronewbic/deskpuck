//! The daily update check and the one-click update: the system's curl does the
//! fetching, and deskpuck-core decides what to fetch and whether it is genuine.

use deskpuck_core::release::{self, Download, Package, Release};
use deskpuck_core::update::{LATEST_URL, allowed_by_config, evaluate};
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::io::{Read, Write};
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
    let mut command = request(curl, current);
    command.args(["-I", "-m", "10", "-o", NULL, "-w", "%{redirect_url}", LATEST_URL]);
    let mut output = Vec::new();
    capture(&mut command, &mut output, MAX_OUTPUT).ok()?;
    evaluate(std::str::from_utf8(&output).ok()?, current)
}

/// How this copy takes an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    /// Downloads, verifies and replaces itself.
    InPlace,
    /// Opens the release page: a copy the installer or install.sh did not put here.
    Browser,
    /// A package manager installed it and owns its updates.
    PackageManager,
}

pub fn install_kind(program: &Path) -> Install {
    if package_managed(program) {
        Install::PackageManager
    } else if Package::this_platform().is_some() && platform::installed_in_place(program) {
        Install::InPlace
    } else {
        Install::Browser
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Download,
    Verify,
    Install,
}

/// What to do once the tray has shut down.
#[derive(Debug, PartialEq, Eq)]
pub enum Ready {
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    Exit,
    #[cfg_attr(windows, allow(dead_code))]
    Relaunch(PathBuf),
}

/// Downloads `version` into a fresh private folder, verifies it, and installs as much
/// as this platform can while the tray still runs. Nothing installs unless it verifies.
pub fn install(
    curl: &Path,
    version: &str,
    current: &str,
    program: &Path,
) -> Result<Ready, Failure> {
    without_panics(|| install_verified(curl, version, current, program))
}

/// A panic is reported as a failed install, so the menu never stays on Downloading.
fn without_panics(install: impl FnOnce() -> Result<Ready, Failure>) -> Result<Ready, Failure> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(install)).unwrap_or(Err(Failure::Install))
}

fn install_verified(
    curl: &Path,
    version: &str,
    current: &str,
    program: &Path,
) -> Result<Ready, Failure> {
    let package = Package::this_platform().ok_or(Failure::Install)?;
    let release = Release::new(version, current, package).ok_or(Failure::Verify)?;
    let dir = tempfile::Builder::new()
        .prefix("deskpuck-update-")
        .tempdir()
        .map_err(|_| Failure::Download)?;
    for download in release.downloads() {
        fetch(curl, &download, current, dir.path())?;
    }
    let package = release::verify(dir.path(), &release).map_err(|_| Failure::Verify)?;
    platform::install(dir, &package, &release, &platform::relaunch_path(program))
}

/// github.com answers each download with a redirect, followed only to the release-asset
/// host, so the file streams into `dir`, cut off at its cap.
pub fn fetch(curl: &Path, download: &Download, current: &str, dir: &Path) -> Result<(), Failure> {
    let mut command = request(curl, current);
    command.args(["-I", "-m", "60", "-o", NULL, "-w", "%{redirect_url}", &download.url]);
    let mut location = Vec::new();
    capture(&mut command, &mut location, release::MAX_URL as u64)
        .map_err(|()| Failure::Download)?;
    let location = String::from_utf8(location).map_err(|_| Failure::Download)?;
    if !release::redirect_allowed(&location) {
        return Err(Failure::Download);
    }
    let path = dir.join(&download.name);
    let mut file = std::fs::File::create_new(&path).map_err(|_| Failure::Download)?;
    let mut command = request(curl, current);
    command.args(["--connect-timeout", "30", "-Y", "1024", "-y", "60", "-m", "1800"]);
    command.args(["-o", "-", &location]);
    if capture(&mut command, &mut file, download.max_bytes).is_err() || file.sync_all().is_err() {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(Failure::Download);
    }
    Ok(())
}

/// -q first, so no .curlrc can add options; -g, so a URL is never a glob pattern.
fn request(curl: &Path, current: &str) -> Command {
    let mut command = Command::new(curl);
    command
        .args(["-q", "-fsS", "-g", "--proto", "=https", "--max-redirs", "0"])
        .args(["-A", &format!("Deskpuck/{current}")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    command
}

/// Copies the command's output into `sink`; Err past `max` bytes or on a failed exit.
fn capture(command: &mut Command, sink: &mut dyn Write, max: u64) -> Result<(), ()> {
    let mut child = command.spawn().map_err(|_| ())?;
    let stdout = child.stdout.take().ok_or(())?;
    let copied = std::io::copy(&mut stdout.take(max + 1), sink);
    if !copied.is_ok_and(|bytes| bytes <= max) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(());
    }
    if child.wait().is_ok_and(|status| status.success()) { Ok(()) } else { Err(()) }
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
pub fn curl() -> PathBuf {
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

/// Where install.sh puts the tray, from the same variables it reads.
#[cfg_attr(windows, allow(dead_code))]
pub fn install_target(
    xdg_bin_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    match xdg_bin_home.filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir).join("deskpuck")),
        None => home
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(".local").join("bin").join("deskpuck")),
    }
}

/// Whether `program` is deskpuck.exe in the folder the installer recorded. Windows
/// compares paths without regard to case; Inno Setup ends the folder with a backslash.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn installer_owns(program: &str, install_location: &str) -> bool {
    let folder = install_location.trim_end_matches('\\');
    !folder.is_empty() && program.eq_ignore_ascii_case(&format!("{folder}\\deskpuck.exe"))
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Failure, Ready, Release};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    /// Only a copy install.sh put in place, so the update replaces this very program.
    pub fn installed_in_place(program: &Path) -> bool {
        super::install_target(std::env::var_os("XDG_BIN_HOME"), std::env::var_os("HOME"))
            .and_then(|target| std::fs::canonicalize(target).ok())
            .is_some_and(|target| target == program)
    }

    /// Where install.sh put the new tray: when that path is a symlink, install.sh replaces
    /// the link, and the resolved `program` would still be the old binary.
    pub fn relaunch_path(program: &Path) -> PathBuf {
        super::install_target(std::env::var_os("XDG_BIN_HOME"), std::env::var_os("HOME"))
            .unwrap_or_else(|| program.to_owned())
    }

    /// The tarball's own install.sh replaces the running programs; the new tray starts
    /// once this one has let go of the Joy-Con.
    pub fn install(
        dir: tempfile::TempDir,
        package: &Path,
        release: &Release,
        relaunch: &Path,
    ) -> Result<Ready, Failure> {
        let quiet = |command: &mut Command| {
            command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()
        };
        let unpacked =
            quiet(Command::new("tar").arg("-xzf").arg(package).arg("-C").arg(dir.path()));
        if !unpacked.is_ok_and(|status| status.success()) {
            return Err(Failure::Install);
        }
        let folder = release.package().strip_suffix(".tar.gz").ok_or(Failure::Install)?;
        let script = dir.path().join(folder).join("install.sh");
        if !quiet(Command::new("/bin/sh").arg(script)).is_ok_and(|status| status.success()) {
            return Err(Failure::Install);
        }
        Ok(Ready::Relaunch(relaunch.to_owned()))
    }
}

#[cfg(windows)]
mod platform {
    use super::{Failure, Ready, Release};
    use std::path::Path;

    pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{F0FA84CD-5402-434B-9A3E-9C18C110CD17}_is1";

    pub fn relaunch_path(program: &Path) -> std::path::PathBuf {
        program.to_owned()
    }

    /// Only a copy the per-user installer put in place; a portable copy keeps the release page.
    pub fn installed_in_place(program: &Path) -> bool {
        let location = crate::login::read_registry(UNINSTALL_KEY, "InstallLocation");
        program
            .to_str()
            .zip(location)
            .is_some_and(|(program, location)| super::installer_owns(program, &location))
    }

    /// The installer closes whatever still runs from the old copy, installs, and starts
    /// the new one (deskpuck.iss runs it even when silent).
    pub fn install(
        dir: tempfile::TempDir,
        package: &Path,
        _release: &Release,
        _program: &Path,
    ) -> Result<Ready, Failure> {
        std::process::Command::new(package)
            .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"])
            .spawn()
            .map_err(|_| Failure::Install)?;
        // Windows cannot delete the folder while the installer runs from it.
        let _ = dir.keep();
        Ok(Ready::Exit)
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use super::{Failure, Ready, Release};
    use std::path::Path;

    pub fn installed_in_place(_program: &Path) -> bool {
        false
    }

    pub fn relaunch_path(program: &Path) -> std::path::PathBuf {
        program.to_owned()
    }

    pub fn install(
        _dir: tempfile::TempDir,
        _package: &Path,
        _release: &Release,
        _program: &Path,
    ) -> Result<Ready, Failure> {
        Err(Failure::Install)
    }
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

    const FIXTURES: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../deskpuck-core/tests/fixtures/release");
    const ASSET_HOST: &str = "https://release-assets.githubusercontent.com/";

    /// A stand-in curl for release downloads: a HEAD request is redirected to `redirect`
    /// plus the file's name, and a download prints that file from `serve`. Each call's
    /// arguments go to `log`, one call per line.
    fn release_curl(dir: &Path, redirect: &str, serve: &Path) -> PathBuf {
        let program = dir.join("curl");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nfor a; do last=$a; done\necho \"$*\" >> '{log}'\n\
                 case \" $* \" in\n  *\" -I \"*) printf '%s%s' '{redirect}' \"${{last##*/}}\" ;;\n  \
                 *) cat '{serve}'/\"${{last##*/}}\" || exit 22 ;;\nesac\n",
                log = dir.join("log").display(),
                serve = serve.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    fn calls(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("log"))
            .unwrap_or_default()
            .lines()
            .map(Into::into)
            .collect()
    }

    fn sums_download() -> Download {
        Release::new("0.4.0", "0.3.0", Package::MacZip).unwrap().downloads()[0].clone()
    }

    #[test]
    fn a_download_follows_one_redirect_to_the_asset_host_only() {
        let (bin, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let curl = release_curl(bin.path(), ASSET_HOST, Path::new(FIXTURES));
        assert_eq!(fetch(&curl, &sums_download(), "0.3.0", out.path()), Ok(()));
        let fetched = std::fs::read(out.path().join("SHA256SUMS")).unwrap();
        assert_eq!(fetched, std::fs::read(Path::new(FIXTURES).join("SHA256SUMS")).unwrap());
        let made = calls(bin.path());
        assert_eq!(made.len(), 2, "{made:?}");
        assert!(made[0].ends_with(&sums_download().url), "{made:?}");
        assert!(made[1].ends_with(&format!("-o - {ASSET_HOST}SHA256SUMS")), "{made:?}");
        for call in &made {
            assert!(call.starts_with("-q -fsS -g --proto =https --max-redirs 0 -A Deskpuck/0.3.0"));
        }

        let (bin, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let curl = release_curl(bin.path(), "https://evil.example/", Path::new(FIXTURES));
        assert_eq!(fetch(&curl, &sums_download(), "0.3.0", out.path()), Err(Failure::Download));
        assert_eq!(calls(bin.path()).len(), 1, "nothing is fetched from another host");
        assert!(!out.path().join("SHA256SUMS").exists());
    }

    #[test]
    fn a_redirect_as_long_as_the_core_allows_is_followed() {
        let (bin, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let long = format!("{ASSET_HOST}{}/", "a".repeat(2000));
        let curl = release_curl(bin.path(), &long, Path::new(FIXTURES));
        assert_eq!(fetch(&curl, &sums_download(), "0.3.0", out.path()), Ok(()));
        let too_long = format!("{ASSET_HOST}{}/", "a".repeat(release::MAX_URL));
        let curl = release_curl(bin.path(), &too_long, Path::new(FIXTURES));
        std::fs::remove_file(out.path().join("SHA256SUMS")).unwrap();
        assert_eq!(fetch(&curl, &sums_download(), "0.3.0", out.path()), Err(Failure::Download));
    }

    #[test]
    fn an_oversized_or_failed_download_leaves_no_file() {
        let (bin, out, serve) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let curl = release_curl(bin.path(), ASSET_HOST, serve.path());
        let mut download = sums_download();
        download.max_bytes = 10;
        std::fs::write(serve.path().join("SHA256SUMS"), [b'x'; 11]).unwrap();
        assert_eq!(fetch(&curl, &download, "0.3.0", out.path()), Err(Failure::Download));
        assert!(!out.path().join("SHA256SUMS").exists());

        std::fs::write(serve.path().join("SHA256SUMS"), [b'x'; 10]).unwrap();
        assert_eq!(fetch(&curl, &download, "0.3.0", out.path()), Ok(()), "control: at the cap");
        std::fs::remove_file(out.path().join("SHA256SUMS")).unwrap();

        std::fs::remove_file(serve.path().join("SHA256SUMS")).unwrap();
        assert_eq!(fetch(&curl, &download, "0.3.0", out.path()), Err(Failure::Download));
        assert!(!out.path().join("SHA256SUMS").exists());
    }

    #[test]
    fn a_file_already_in_the_folder_is_never_written_through() {
        let (bin, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let curl = release_curl(bin.path(), ASSET_HOST, Path::new(FIXTURES));
        let target = out.path().join("elsewhere");
        std::os::unix::fs::symlink(&target, out.path().join("SHA256SUMS")).unwrap();
        assert_eq!(fetch(&curl, &sums_download(), "0.3.0", out.path()), Err(Failure::Download));
        assert!(!target.exists());
    }

    #[test]
    fn a_release_not_signed_with_the_release_key_installs_nothing() {
        if Package::this_platform().is_none() {
            return;
        }
        let (bin, serve) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // The test-key fixtures, laid out as a real release serves them.
        for name in ["SHA256SUMS", "SHA256SUMS.minisig"] {
            std::fs::copy(Path::new(FIXTURES).join(name), serve.path().join(name)).unwrap();
        }
        for entry in std::fs::read_dir(Path::new(FIXTURES).join("assets")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), serve.path().join(entry.file_name())).unwrap();
        }
        let curl = release_curl(bin.path(), ASSET_HOST, serve.path());
        let program = bin.path().join("deskpuck");
        assert_eq!(install(&curl, "0.4.0", "0.3.0", &program), Err(Failure::Verify));
        assert_eq!(calls(bin.path()).len(), 6, "all three files were fetched before the refusal");
        assert_eq!(install(&curl, "0.3.0", "0.3.0", &program), Err(Failure::Verify), "not newer");
        assert_eq!(calls(bin.path()).len(), 6, "and nothing fetched for it");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_tarball_install_script_runs_and_the_tray_relaunches() {
        let dir = tempfile::tempdir().unwrap();
        let release = Release::new("0.4.0", "0.3.0", Package::LinuxTarball).unwrap();
        let tarball = |script: &str| {
            let source = tempfile::tempdir().unwrap();
            let folder = source.path().join("Deskpuck-0.4.0-linux-x86_64");
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(folder.join("install.sh"), script).unwrap();
            let out = tempfile::tempdir().unwrap();
            let package = out.path().join(release.package());
            let made = Command::new("tar")
                .arg("-czf")
                .arg(&package)
                .arg("-C")
                .arg(source.path())
                .arg("Deskpuck-0.4.0-linux-x86_64")
                .status()
                .unwrap();
            assert!(made.success());
            let staged = tempfile::tempdir().unwrap();
            std::fs::copy(&package, staged.path().join(release.package())).unwrap();
            staged
        };
        let marker = dir.path().join("installed");
        let program = dir.path().join("deskpuck");
        let staged = tarball(&format!("touch '{}'\n", marker.display()));
        let package = staged.path().join(release.package());
        assert_eq!(
            platform::install(staged, &package, &release, &program),
            Ok(Ready::Relaunch(program.clone()))
        );
        assert!(marker.exists(), "install.sh ran");

        let staged = tarball("exit 1\n");
        let package = staged.path().join(release.package());
        assert_eq!(platform::install(staged, &package, &release, &program), Err(Failure::Install));
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;

    #[test]
    fn a_panic_during_an_install_is_a_failed_install() {
        assert_eq!(without_panics(|| panic!("simulated")), Err(Failure::Install));
        assert_eq!(without_panics(|| Ok(Ready::Exit)), Ok(Ready::Exit), "control");
        assert_eq!(without_panics(|| Err(Failure::Verify)), Err(Failure::Verify));
    }

    #[test]
    fn install_target_reads_the_variables_install_sh_reads() {
        let target = |xdg: Option<&str>, home: Option<&str>| {
            install_target(xdg.map(Into::into), home.map(Into::into))
        };
        assert_eq!(target(None, Some("/home/me")), Some("/home/me/.local/bin/deskpuck".into()));
        assert_eq!(target(Some("/opt/bin"), Some("/home/me")), Some("/opt/bin/deskpuck".into()));
        assert_eq!(target(Some(""), Some("/home/me")), Some("/home/me/.local/bin/deskpuck".into()));
        assert_eq!(target(None, None), None);
        assert_eq!(target(None, Some("")), None);
    }

    #[test]
    fn only_the_installers_own_copy_updates_in_place() {
        let location = r"C:\Users\Me\AppData\Local\Programs\Deskpuck\";
        assert!(installer_owns(
            r"C:\Users\Me\AppData\Local\Programs\Deskpuck\deskpuck.exe",
            location
        ));
        assert!(installer_owns(
            r"c:\users\me\appdata\local\programs\deskpuck\DESKPUCK.EXE",
            location
        ));
        assert!(installer_owns(r"D:\Apps\Deskpuck\deskpuck.exe", r"D:\Apps\Deskpuck"));
        for program in [
            r"C:\Users\Me\Downloads\Deskpuck-0.3.0-windows-x86_64\deskpuck.exe",
            r"C:\Users\Me\AppData\Local\Programs\Deskpuck\deskpuck-cli.exe",
            r"C:\Users\Me\AppData\Local\Programs\Deskpuck\sub\deskpuck.exe",
        ] {
            assert!(!installer_owns(program, location), "{program}");
        }
        assert!(!installer_owns(r"\deskpuck.exe", r"\"), "an empty folder owns nothing");
        assert!(!installer_owns(r"deskpuck.exe", ""));
    }

    #[test]
    fn the_installer_matches_what_the_update_expects() {
        let iss = include_str!("../../../packaging/windows/deskpuck.iss");
        assert!(
            iss.contains("AppId={{F0FA84CD-5402-434B-9A3E-9C18C110CD17}\n"),
            "the uninstall key"
        );
        assert!(iss.contains("PrivilegesRequired=lowest\n"), "per-user, so no UAC prompt");
        let run = iss.lines().find(|line| line.starts_with(r#"Filename: "{app}\deskpuck.exe""#));
        let run = run.expect("a [Run] entry starts Deskpuck");
        assert!(!run.contains("skipifsilent"), "a silent update must start the new copy: {run}");
    }
}

/// The shipped download, verify and install against a published release (v0.3.0, the first
/// signed with the release key, or DESKPUCK_LIVE_UPDATE_VERSION), with no test-only address.
/// It changes this machine, so it runs only when asked:
/// `DESKPUCK_LIVE_UPDATE_TEST=1 cargo test -p deskpuck-tray live -- --ignored`, as the Release
/// checks workflow does. On Linux it installs into XDG_BIN_HOME and XDG_DATA_HOME, which must
/// name scratch folders.
#[cfg(all(test, any(target_os = "linux", windows)))]
mod live_tests {
    use super::*;

    fn published() -> String {
        std::env::var("DESKPUCK_LIVE_UPDATE_VERSION").unwrap_or_else(|_| "0.3.0".into())
    }

    fn asked() {
        assert_eq!(
            std::env::var("DESKPUCK_LIVE_UPDATE_TEST").as_deref(),
            Ok("1"),
            "installs a release on this machine; set DESKPUCK_LIVE_UPDATE_TEST=1 to mean it"
        );
    }

    #[cfg(windows)]
    fn eventually<T>(what: &str, seconds: u64, mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = std::time::Instant::now() + Duration::from_secs(seconds);
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "downloads and installs the published release; see the module comment"]
    fn live_update_installs_the_published_release() {
        use std::os::unix::fs::PermissionsExt;
        asked();
        let scratch = |name| PathBuf::from(std::env::var_os(name).expect("a scratch folder"));
        let (bin, data) = (scratch("XDG_BIN_HOME"), scratch("XDG_DATA_HOME"));
        let target = bin.join("deskpuck");
        assert!(!target.exists(), "XDG_BIN_HOME must start empty: {}", bin.display());

        let result = install(&curl(), &published(), "0.0.1", Path::new("/nonexistent/deskpuck"));
        assert_eq!(result, Ok(Ready::Relaunch(target.clone())));
        for program in ["deskpuck", "deskpuck-cli"] {
            let mode = std::fs::metadata(bin.join(program)).expect(program).permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{program} is executable");
        }
        let entry = std::fs::read_to_string(data.join("applications/deskpuck.desktop")).unwrap();
        assert!(entry.contains(&format!("Exec=\"{}\"", target.display())), "{entry}");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "downloads and installs the published release; see the module comment"]
    fn live_update_installs_the_published_release() {
        asked();
        let program = Path::new(r"C:\nonexistent\deskpuck.exe");
        assert_eq!(install(&curl(), &published(), "0.0.1", program), Ok(Ready::Exit));

        // The installer runs on after install() returns, then starts the new copy itself.
        let read = |name| crate::login::read_registry(platform::UNINSTALL_KEY, name);
        eventually("the installer to record the version", 300, || {
            (read("DisplayVersion") == Some(published())).then_some(())
        });
        let folder = read("InstallLocation").expect("the install folder");
        let installed = Path::new(&folder).join("deskpuck.exe");
        assert!(installed.is_file(), "{}", installed.display());
        eventually("the installer to start the new copy", 120, || {
            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", "(Get-Process deskpuck -ErrorAction Ignore).Path"])
                .output()
                .ok()?;
            let paths = String::from_utf8_lossy(&output.stdout).into_owned();
            paths.lines().any(|path| Path::new(path.trim()) == installed).then_some(())
        });
    }
}
