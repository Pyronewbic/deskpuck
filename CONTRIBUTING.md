# Contributing

Bug reports and pull requests are welcome. For anything larger than a fix, open an issue first so we can agree on the approach. How the code is laid out: [ARCHITECTURE.md](ARCHITECTURE.md).

## Requirements

- Rust through rustup: `rust/rust-toolchain.toml` pins the version, which rustup installs on first use. On Linux it also needs `libdbus-1-dev` and `pkg-config` for Bluetooth.
- For the Mac app: Xcode command line tools with Swift 5.9 or later, and Xcode 26 for `scripts/make-app.sh` (it compiles the app icon with `actool`).
- `gitleaks`, `cargo-deny` and `cargo-nextest`, for the checks before a push (`brew install gitleaks cargo-deny cargo-nextest`).

## Build and run

```sh
scripts/build-rust.sh        # Mac: the Rust core as a static library, needed before swift build
scripts/make-app.sh          # Mac: dist/Deskpuck.app

cd rust
cargo test
cargo build --release -p deskpuck-tray                # Linux and Windows: rust/target/release/deskpuck
cargo run --release --bin deskpuck-cli -- --monitor   # live readout of every Joy-Con report
cargo run --release -p deskpuck-replay -- --dry-run   # print the replay's events
cargo run --release -p deskpuck-replay                # post them for real
```

`make-app.sh` signs with a self-signed certificate named "Deskpuck Dev" and prints how to create it if it is missing. `--adhoc` skips it, but then macOS forgets the Accessibility permission on every build.

Only one program can connect to the Joy-Con at a time, so quit the app before running `deskpuck-cli`. Without `--dry-run`, the replay moves the real pointer, scrolls and presses arrow keys for about eight seconds after a three-second countdown. It never clicks or presses Return. Focus a text editor first. On Linux, both need the `/dev/uinput` access from the [README](README.md#linux).

## Changes

- `scripts/check.sh` must pass: Rust format, lints and tests (with cargo-nextest, then doctests), a check that every file in `rust/*/tests/` ran, known advisories and licenses of every crate (`rust/deny.toml`) and, on macOS, the app build. Exit 1 means a check failed, 3 that one could not run. CI runs it on pushes to main, on pull requests and weekly.
- Turn on the pre-push hook once per clone, which scans the commits being pushed for secrets and then runs `scripts/check.sh`: `git config core.hooksPath .githooks`.
- A behavior change or bug fix includes a test that fails without it.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), one change per commit.

Releases are made by the maintainer; see [RELEASING.md](RELEASING.md).
