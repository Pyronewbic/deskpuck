# Architecture

A Joy-Con 2 sends input reports over Bluetooth LE. Deskpuck parses each report, turns it into pointer, click, scroll and key output, and posts that output as system input events.

```
Bluetooth report -> parse -> engine -> post as OS input events
```

## Code map

**`rust/`**: the core, cross-platform.
- `deskpuck-core`: parses a report into fields (buttons, sticks, optical sensor, IMU), turns reports into output (pointer deltas, mouse buttons, wheel, key events with repeat), keeps the pointer on screen, and reads and writes `config.json` and `pairing.json`.
- `deskpuck-ble`: the Bluetooth connection. `receiver.rs` is a state machine with no I/O (inputs and the time in, commands out, every timer a deadline); `controller.rs` drives it through btleplug on a background thread. Shared by the app and the `deskpuck-cli` command-line tool.
- `deskpuck-inject`: turns engine output into events and posts them through a backend per OS: CGEvent, uinput, or SendInput.
- `deskpuck-ffi`: the C interface the app calls (`include/deskpuck.h`), built as a static library by `scripts/build-rust.sh`. Settings cross it as JSON.
- `deskpuck-replay`: feeds a built-in demo or a capture file through engine and backend, so the input layer can be tested without Bluetooth.

**`Sources/Deskpuck/`**: the menu bar app (Swift, AppKit and SwiftUI): status menu, Settings window, menu bar glyph. `Core.swift` wraps the C interface; `Sources/DeskpuckFFI/` is its module map.

**`Sources/DeskpuckCore/`**, **`tests/`**: the previous C++ core and its tests. The app no longer uses them, and they will be removed along with `Sources/deskpuck-cli/`, the old C++ command-line tool, which is no longer built. `tests/fixtures/joycon2_r_capture.txt` is a capture from real hardware that the Rust tests also read.

**`scripts/`**: build, sign and package the app.

## Rules that hold the code together

- The parser and engine never call the OS. The screen-edge clamp (`clamp_to_displays`) takes the display lookup as a function: the macOS backend passes the real one, tests pass fake displays. Everything else platform-specific lives in the Bluetooth driver (`controller.rs`) or an input backend.
- Key codes are macOS virtual key codes everywhere, including in `config.json`. The Linux and Windows backends translate them at the last step (`deskpuck-inject/src/keymap.rs`).
- Only the paired Joy-Con connects, except inside a pairing window. The receiver enforces this on discovery, and its single scan path is also where pausing stops all scanning. The pairing lives in its own `pairing.json` (`pairing.rs`), so a settings save never overwrites it.
- Settings are loaded, validated and saved only in Rust (`config.rs`). The app passes them across the C interface and shows the warnings or errors that come back.
- On macOS, pointer motion is posted as real move or drag events, never as cursor warps. The Dock and hot corners only react to real events.
- A pointer move is posted before the same report's button changes, so a press in that report starts a drag on the next move.
- Every path that starts a Bluetooth scan goes through one function, `scan` in `receiver.rs`, which is where Pause blocks scanning.
- The C header and the library are versioned together (`DP_ABI_VERSION`); the app refuses to start if the library's version differs from the header it was compiled with.
