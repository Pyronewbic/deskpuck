# Architecture

A Joy-Con 2 sends input reports over Bluetooth LE. Deskpuck parses each report, turns it into pointer, click, scroll and key output, and posts that output as system input events.

```
Bluetooth report -> parse -> engine -> post as OS input events
```

## Code map

**`Sources/DeskpuckCore/`**: the shipping core (C++ and Objective-C++), used by the app and the command-line tool.
- `Joycon2Packet`: parses a report into fields (buttons, sticks, optical sensor, IMU).
- `Joycon2Engine`, `Joycon2InputMapping`: turn reports into output (pointer deltas, mouse buttons, wheel, key events with repeat) and keep the pointer on screen.
- `DPConfig`: reads and writes `config.json`.
- `Joycon2BLEReceiver`: CoreBluetooth scanning, connection, and the init commands that start the report stream.
- `DPController`: connects the receiver to the engine and posts the output as CGEvents.

**`Sources/Deskpuck/`**: the menu bar app (Swift, AppKit and SwiftUI): status menu, Settings window, menu bar glyph.

**`Sources/deskpuck-cli/`**: the same controller without a user interface, with a live report monitor.

**`tests/`**: C++ and Objective-C++ suites run by `tests/run.sh`. `fixtures/joycon2_r_capture.txt` is a capture from real hardware, shared with the Rust tests.

**`rust/`**: a cross-platform port of the core.
- `deskpuck-core`: parser, engine, mapping and config, ported from the C++ with the same tests.
- `deskpuck-inject`: turns engine output into events and posts them through a backend per OS: CGEvent, uinput, or SendInput.
- `deskpuck-replay`: feeds a built-in demo or a capture file through engine and backend, so the input layer can be tested without Bluetooth.

**`scripts/`**: build, sign and package the app.

## Rules that hold the code together

- The parser and engine never call the OS. The one display query, `systemDisplayLookup`, is passed to the screen-edge clamp as a function, so tests use fake displays. Everything else platform-specific lives in `DPController`, the receiver, or a Rust backend.
- Key codes are macOS virtual key codes everywhere, including in `config.json`. The Linux and Windows backends translate them at the last step (`deskpuck-inject/src/keymap.rs`).
- Both cores read and write the same `config.json` schema (version 1), with the same validation. A file written by one loads in the other without warnings.
- On macOS, pointer motion is posted as real move or drag events, never as cursor warps. The Dock and hot corners only react to real events.
- A pointer move is posted before the same report's button changes, so a press in that report starts a drag on the next move.
- Every path that starts a Bluetooth scan goes through `startScan` in the receiver, which is where Pause blocks scanning.
- The C++ core is what ships today. The Rust core must stay in step with it, test for test.
