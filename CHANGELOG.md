# Changelog

Notable changes to Deskpuck. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `deskpuck`, the app for Linux and Windows: a tray icon with status, pairing, pause, latched modifiers, and opening or reloading `config.json`.
- A settings window for Linux and Windows with the Mac app's controls, opened from the tray's Settings... item or a left click (`deskpuck --settings`); the tray applies changes to `config.json` as soon as the file changes.
- CI builds and tests the Rust workspace on Windows.
- Linux: a Joy-Con 2 connects over a direct ATT link, since BlueZ cannot resolve its services. No root needed.
- A Theme setting (System, Light or Dark) for the settings window, on every platform; `appearance` in `config.json`.
- The Linux and Windows settings window is laid out like the Mac's, in titled cards with switches and a fixed-width value beside each slider, sized to fit its content. It uses the system UI font (Segoe UI on Windows, fontconfig's sans-serif on Linux) and the system accent colour, on Linux where the desktop portal shares one.
- Settings uses the system accent colour on the Mac too.
- The Linux and Windows tray icon and settings window show the Deskpuck logo, the Mac app's icon, instead of a plain indigo dot; on Windows the .exe files carry it too.

### Fixed

- Scanning stops before connecting, and a failed connect is cancelled so the Joy-Con is not left linked.
- `--verbose` logs why a connect or service discovery failed.
- A Joy-Con is named by its model even when BlueZ reports its address as the name.
- Linux: a connect the Joy-Con fails to establish is tried again at once, up to three times.
- Linux and Windows: starting Deskpuck while it is already running leaves the running copy alone instead of starting a second tray.
- The Joy-Con's lights stop the searching sweep once it is connected and show player 1, as on a Switch 2.
- Windows: the tests pass in a clone made with Git for Windows' default settings (files now check out with LF line endings).
- Windows: the Joy-Con sends about 66 reports a second instead of 16, so the pointer moves smoothly: Deskpuck asks Windows for its throughput-optimized connection parameters while connected.

## [0.2.0] - 2026-10-06

### Changed

- **Upgrading:** Deskpuck now connects only to the Joy-Con you pair, and nothing connects until you pair once; holding SYNC alone no longer connects. Choose **Pair New Joy-Con...** in the menu, then hold SYNC within 60 seconds. Pairing another Joy-Con replaces it. `deskpuck-cli --pair` does the same from the terminal, and the app and the tool share the pairing.
- The menu says when the paired Joy-Con is in use by another app on this Mac (for example `deskpuck-cli`), instead of searching for it.
- A Joy-Con shows as Joy-Con 2 (L) or (R) as soon as it is found.

### Added

- Shortcuts: a button can press a key with modifiers, such as Control+C. Choose **Record Shortcut...** in Settings. In `config.json` a mapping is either a key code or `{"key": 8, "modifiers": ["control"]}`; existing files load unchanged.
- Modifier buttons: a button can act as Control, Option, Shift or Command while held, or latch it on and off with a tap, including for clicks. The menu bar shows a latched modifier, and pausing, quitting or changing settings releases it. In `config.json`: `{"modifier": "shift", "latch": true}`.

### Removed

- The old C++ core and its command-line tool. The app and `deskpuck-cli` share one Rust core.

### Fixed

- Held keys, clicks and latched modifiers are released even if macOS refuses one input event.

### Security

- Deskpuck no longer connects to any nearby device that advertises as a Joy-Con 2, only to the paired one (see Changed). This resolves the 0.1.0 known limitation.
- Device names have hidden characters (such as text-direction overrides) removed and are capped at 64 characters.

## [0.1.0] - 2026-10-05

First release. Apple Silicon Macs only, macOS 13 or later (tested on macOS 26).

- Use a Switch 2 Joy-Con (R) as a mouse: slide it to move the pointer, R and ZR to click, the stick to scroll.
- Map the other buttons to keys in Settings, with key repeat.
- Pause Mouse Control stops input and stops looking for a Joy-Con, while keeping a connected one linked.
- Signed with the hardened runtime and a stable certificate, so the Accessibility permission carries over to later versions.
- Known limitation: Deskpuck connects to any nearby device that advertises as a Joy-Con 2. See [SECURITY.md](https://github.com/Pyronewbic/deskpuck/blob/main/SECURITY.md).

[Unreleased]: https://github.com/Pyronewbic/deskpuck/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Pyronewbic/deskpuck/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Pyronewbic/deskpuck/releases/tag/v0.1.0
