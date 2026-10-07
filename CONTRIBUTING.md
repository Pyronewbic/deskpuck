# Contributing

Bug reports and pull requests are welcome. For anything larger than a fix, open an issue first so we can agree on the approach.

## Requirements

- Xcode command line tools with Swift 5.9 or later, for `swift build`.
- Xcode 26, for `scripts/make-app.sh` (it compiles the app icon with `actool`).
- Python 3, for `scripts/make-dmg.sh` (it installs a pinned `dmgbuild` into `.venv`).
- Rust through rustup: the app's core is the [Rust workspace](#rust-workspace), and `rust/rust-toolchain.toml` pins the version, which rustup installs on first use. On Linux it also needs `libdbus-1-dev` and `pkg-config` for Bluetooth.
- `gitleaks`, `cargo-deny` and `cargo-nextest`, for the checks before a push (`brew install gitleaks cargo-deny cargo-nextest`).

## Building the Mac app

```sh
scripts/build-rust.sh        # the Rust core as a static library, needed before swift build
swift build -c release       # the app
scripts/make-app.sh          # dist/Deskpuck.app
scripts/make-dmg.sh          # dist/Deskpuck-<version>.dmg and .zip
```

Releases are built and signed on the maintainer's Mac by `scripts/release.sh`, which runs every check, builds the disk image, and with `--publish` pushes a signed tag and creates a draft GitHub release. The notes come from [CHANGELOG.md](CHANGELOG.md).

`make-app.sh` signs with a self-signed certificate named "Deskpuck Dev" (or the one named in `DESKPUCK_SIGN_IDENTITY`), so macOS keeps the Accessibility permission across rebuilds. If the certificate is missing, it prints the one-time steps to create it in Keychain Access. `--adhoc` skips the certificate, but then the permission resets on every build.

To debug input, run the same core without the app: `deskpuck-cli --monitor` shows a live readout of every Joy-Con report (see [Rust workspace](#rust-workspace)). Only one program can connect to the Joy-Con at a time, so quit the app first.

## Rust workspace

`rust/` holds the core: `deskpuck-core` (parser, engine and config), `deskpuck-ble` (the Bluetooth connection and `deskpuck-cli`), `deskpuck-inject` (input backends for macOS, Linux and Windows), `deskpuck-ffi` (the C interface the app links), `deskpuck-replay` (a tool that drives the backends without Bluetooth), `deskpuck-tray` (the Linux and Windows app, built as `deskpuck`) and `deskpuck-settings` (its settings window, a library the app runs with `--settings`).

```sh
cd rust
cargo test
cargo clippy --all-targets
cargo run --release --bin deskpuck-cli -- --monitor   # Joy-Con over Bluetooth
cargo run --release -p deskpuck-replay -- --dry-run      # print the events
cargo run --release -p deskpuck-replay                   # post them for real
```

Without `--dry-run`, the replay moves the real pointer, scrolls and presses arrow keys for about eight seconds after a three-second countdown. It never clicks or presses Return. Focus a text editor first. `--help` lists the options and exit codes.

On Linux the replay needs write access to `/dev/uinput`:

```sh
sudo tee /etc/udev/rules.d/60-deskpuck.rules <<'EOF'
KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
EOF
sudo usermod -aG input "$USER"   # then log out and back in
```

`deskpuck-cli` reads the app's `config.json` and `pairing.json` (`--pair` pairs a Joy-Con, `--config` picks another settings file, `--verbose` prints connection detail) and is tested with a real Joy-Con on macOS and Linux. On Linux, BlueZ never finishes resolving a Joy-Con 2's services, so Deskpuck talks ATT over its own L2CAP socket instead (no root needed). At BlueZ's default connection interval a Joy-Con sends about 21 reports a second; for a smoother pointer, set a shorter default once as root (this applies to every Bluetooth LE device on the machine), then reconnect, since the first connection after the change can still use the old interval. With 15-30 ms the Joy-Con sent about 45 a second:

```sh
sudo sed -i -e 's/^#MinConnectionInterval=$/MinConnectionInterval=12/' -e 's/^#MaxConnectionInterval=$/MaxConnectionInterval=24/' /etc/bluetooth/main.conf
grep -E '^(Min|Max)ConnectionInterval=' /etc/bluetooth/main.conf
sudo systemctl restart bluetooth
# To undo:
sudo sed -i -e 's/^MinConnectionInterval=12$/#MinConnectionInterval=/' -e 's/^MaxConnectionInterval=24$/#MaxConnectionInterval=/' /etc/bluetooth/main.conf && sudo systemctl restart bluetooth
```

The `grep` should print both lines; if it does not, your `main.conf` lacks the commented defaults, so set them by hand under `[LE]`. Running the commands twice changes nothing, and the undo touches only those two lines.

Shorter intervals (7.5-15 ms, about 89 a second) failed to establish a connection several times in testing. On Windows (tested on Windows 11), Deskpuck asks for a 15 ms interval itself, about 66 reports a second, so nothing needs changing there.

`cargo build --release -p deskpuck-tray` builds the app as `rust/target/release/deskpuck`; start it from there. Its menu has the Mac menu's status, pairing, pause and latch lines, and **Settings...** opens the settings window with the Mac app's controls (so does a left click on the icon). **Open Settings File** edits `config.json` by hand; the tray applies any change to the file within a second. If the file has problems when the window opens, its first save keeps a copy as `config.json.bak`. Settings live in `~/.config/deskpuck/config.json` on Linux and `%APPDATA%\Deskpuck\config.json` on Windows, with `pairing.json` beside them; modifier names keep the Mac spelling (`option` presses Alt, `command` the Super or Windows key). On Linux the icon is a StatusNotifierItem over D-Bus: KDE shows it, GNOME needs the AppIndicator extension, and until something shows the icon the app does not connect to the Joy-Con. On Windows it opens no console window. Like the CLI, it needs the `/dev/uinput` access above on Linux.

## Changes

- `scripts/check.sh` must pass: Rust format, lints and tests (with cargo-nextest, then doctests), a check that every file in `rust/*/tests/` ran, known advisories and licenses of every crate (`rust/deny.toml`) and, on macOS, the app build. Exit 1 means a check failed, 3 that one could not run. CI runs it on pushes to main, on pull requests and weekly; each check is a labelled section of the log, and each run's summary lists the test suites and their counts.
- Turn on the pre-push hook once per clone, which scans the commits being pushed for secrets and then runs `scripts/check.sh`: `git config core.hooksPath .githooks`.
- A behavior change or bug fix includes a test that fails without it.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), one change per commit.
