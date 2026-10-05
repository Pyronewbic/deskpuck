# Contributing

Bug reports and pull requests are welcome. For anything larger than a fix, open an issue first so we can agree on the approach.

## Requirements

- Xcode command line tools with Swift 5.9 or later, for `swift build` and the C++ tests.
- Xcode 26, for `scripts/make-app.sh` (it compiles the app icon with `actool`).
- Python 3, for `scripts/make-dmg.sh` (it installs a pinned `dmgbuild` into `.venv`).
- Rust stable: the app's core is the [Rust workspace](#rust-workspace). Tested with 1.99. On Linux it also needs `libdbus-1-dev` and `pkg-config` for Bluetooth.
- `gitleaks` and `cargo-deny`, for the checks before a push (`brew install gitleaks cargo-deny`).

## Building the Mac app

```sh
scripts/build-rust.sh        # the Rust core as a static library, needed before swift build
swift build -c release       # the app
scripts/make-app.sh          # dist/Deskpuck.app
scripts/make-dmg.sh          # dist/Deskpuck-<version>.dmg and .zip
```

`make-app.sh` signs with a self-signed certificate named "Deskpuck Dev" (or the one named in `DESKPUCK_SIGN_IDENTITY`), so macOS keeps the Accessibility permission across rebuilds. If the certificate is missing, it prints the one-time steps to create it in Keychain Access. `--adhoc` skips the certificate, but then the permission resets on every build.

To debug input, run the same core without the app: `deskpuck-cli --monitor` shows a live readout of every Joy-Con report (see [Rust workspace](#rust-workspace)). Only one program can connect to the Joy-Con at a time, so quit the app first.

## Rust workspace

`rust/` holds the core: `deskpuck-core` (parser, engine and config), `deskpuck-ble` (the Bluetooth connection and `deskpuck-cli`), `deskpuck-inject` (input backends for macOS, Linux and Windows), `deskpuck-ffi` (the C interface the app links) and `deskpuck-replay` (a tool that drives the backends without Bluetooth).

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

`deskpuck-cli` reads the app's `config.json` (`--config` picks another file, `--verbose` prints connection detail) and is tested with a real Joy-Con on macOS. Bluetooth has not been run on Linux or Windows yet, and the Windows input backend has not been run on Windows.

## Changes

- `scripts/check.sh` must pass: Rust format, lints and tests, known advisories and licenses of every crate (`rust/deny.toml`), the C++ tests and the app build. Exit 1 means a check failed, 3 that one could not run. CI runs it on pushes to main, on pull requests and weekly.
- Turn on the pre-push hook once per clone, which scans the commits being pushed for secrets and then runs `scripts/check.sh`: `git config core.hooksPath .githooks`.
- A behavior change or bug fix includes a test that fails without it.
- Changes go to the Rust core. The C++ in `Sources/DeskpuckCore` is frozen until it is removed; `tests/run.sh` still runs its tests.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), one change per commit.
