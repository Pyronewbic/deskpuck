# Changelog

Notable changes to Deskpuck. Versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0]

First release. Apple Silicon Macs only, macOS 13 or later (tested on macOS 26).

- Use a Switch 2 Joy-Con (R) as a mouse: slide it to move the pointer, R and ZR to click, the stick to scroll.
- Map the other buttons to keys in Settings, with key repeat.
- Pause Mouse Control stops input and stops looking for a Joy-Con, while keeping a connected one linked.
- Signed with the hardened runtime and a stable certificate, so the Accessibility permission carries over to later versions.
- Known limitation: Deskpuck connects to any nearby device that advertises as a Joy-Con 2. See [SECURITY.md](https://github.com/Pyronewbic/deskpuck/blob/main/SECURITY.md).
