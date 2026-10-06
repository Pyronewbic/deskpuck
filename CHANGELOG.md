# Changelog

Notable changes to Deskpuck. Versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

- Pairing: Deskpuck now connects only to the Joy-Con you pair. Choose **Pair New Joy-Con...** in the menu, then hold SYNC within 60 seconds. Pairing another Joy-Con replaces it. **Upgrading:** nothing connects until you pair once; holding SYNC alone no longer connects. `deskpuck-cli --pair` does the same from the terminal, and the app and the tool share the pairing.
- The menu says when the paired Joy-Con is in use by another app on this Mac (for example `deskpuck-cli`), instead of searching for it.
- A Joy-Con shows as Joy-Con 2 (L) or (R) as soon as it is found.
- The old C++ core and its command-line tool are gone; the app and `deskpuck-cli` share one Rust core.
- Shortcuts: a button can press a key with modifiers, such as Control+C. Choose **Record Shortcut...** in Settings. In `config.json` a mapping is either a key code or `{"key": 8, "modifiers": ["control"]}`; existing files load unchanged.

## [0.1.0]

First release. Apple Silicon Macs only, macOS 13 or later (tested on macOS 26).

- Use a Switch 2 Joy-Con (R) as a mouse: slide it to move the pointer, R and ZR to click, the stick to scroll.
- Map the other buttons to keys in Settings, with key repeat.
- Pause Mouse Control stops input and stops looking for a Joy-Con, while keeping a connected one linked.
- Signed with the hardened runtime and a stable certificate, so the Accessibility permission carries over to later versions.
- Known limitation: Deskpuck connects to any nearby device that advertises as a Joy-Con 2. See [SECURITY.md](https://github.com/Pyronewbic/deskpuck/blob/main/SECURITY.md).
