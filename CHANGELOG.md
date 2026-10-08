# Changelog

Notable changes to Deskpuck. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- The Mac app includes `deskpuck-cli`, for bug reports: `/Applications/Deskpuck.app/Contents/MacOS/deskpuck-cli --verbose`.

## [0.3.0] - 2026-10-08

### Added

- **Linux and Windows.** Deskpuck now runs on Linux and Windows too. It is a tray icon with the Mac menu's status, pairing, pause and latched-modifier lines, and a settings window with the Mac app's controls: left-click the icon or choose **Settings...**. Pair as on the Mac with **Pair New Joy-Con...**; do not pair the Joy-Con in the system's Bluetooth settings.
  - Windows: install with `Deskpuck-0.3.0-setup.exe` (for you alone, no admin needed) or unzip the portable `.zip`. Deskpuck is not code-signed, so Windows first shows "Windows protected your PC": choose More info, then Run anyway.
  - Linux: unpack the `.tar.gz` and run `./install.sh`, which installs for you alone and adds Deskpuck to the app menu (`./install.sh --uninstall` removes it). No root is needed for Bluetooth; moving the pointer needs write access to `/dev/uinput` (see the README). On GNOME, the tray icon needs the AppIndicator extension.
  - Tested on Windows 11 and on CachyOS with KDE Plasma (Wayland, BlueZ 5.87).
  - The Windows and Linux downloads carry a GitHub build attestation: `gh attestation verify <file> --repo Pyronewbic/deskpuck` checks that this repository's release workflow built them.
- **Start at Login** in the menu, on every platform.
- **Theme** in Settings: System, Light or Dark. In `config.json`: `"appearance": "dark"`.
- Known limitation: Windows does not let Deskpuck send input to apps running as administrator (such as an admin PowerShell). While one is in front, switch away with the mouse, the keyboard or Alt+Tab.

### Changed

- Settings uses the system accent colour instead of Deskpuck's indigo.

### Fixed

- The Joy-Con's lights settle on player 1 once it is connected, as on a Switch 2. Before, they kept sweeping as if it were still searching.
- Settings shows pointer speeds such as 1.75x exactly; it rounded them (1.8x).
- Scanning stops before Deskpuck connects, and a connect that fails is cancelled, so the Joy-Con is not left linked.
- `deskpuck-cli --verbose` says why a connect or service discovery failed.

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

[Unreleased]: https://github.com/Pyronewbic/deskpuck/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Pyronewbic/deskpuck/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Pyronewbic/deskpuck/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Pyronewbic/deskpuck/releases/tag/v0.1.0
