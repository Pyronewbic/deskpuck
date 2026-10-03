# JoyMouse

Use a Nintendo Switch 2 Joy-Con (R) as a Mac mouse and keyboard: slide it on a desk to move the pointer, click with the shoulder buttons, scroll with the stick, and map the other buttons to keys.

It runs as a normal app with Bluetooth and Accessibility permission. No kernel extension, and no need to turn off System Integrity Protection.

Based on [seitanmen/Joycon2forMac](https://github.com/seitanmen/Joycon2forMac) (MIT). Bluetooth protocol details from [ndeadly/switch2_controller_research](https://github.com/ndeadly/switch2_controller_research).

Tested on macOS 26 on Apple Silicon.

## Install

1. Open `JoyMouse-<version>.dmg` and drag JoyMouse to Applications.
2. Open JoyMouse. macOS blocks it the first time because it is not notarized by Apple. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll to Security, and click **Open Anyway** next to the JoyMouse message. Confirm with your password or Touch ID.
4. When asked, allow **Bluetooth** and **Accessibility** access. Accessibility is what lets JoyMouse move the pointer and press keys.
5. Hold the **SYNC** button on the Joy-Con until its lights flash. The menu bar icon fills in once it connects.

If step 3 shows no Open Anyway button, run this in Terminal instead:

```sh
xattr -dr com.apple.quarantine /Applications/JoyMouse.app
```

## Use

| Joy-Con (R)          | Default                         |
| -------------------- | ------------------------------- |
| Slide on a surface   | Move the pointer                |
| R / ZR               | Left click / right click        |
| Stick up and down    | Scroll                          |
| Stick click          | Return                          |
| X / B / Y / A        | Up / Down / Left / Right arrows |

Change the key mappings, pointer speed, scrolling and key repeat from the menu bar icon: **Settings...**. Use **Pause Mouse Control** to set the Joy-Con down without moving the pointer.

Settings are saved to `~/Library/Application Support/JoyMouse/config.json`.

## Build from source

Requires Xcode command line tools (Swift 5.9 or later).

```sh
tests/run.sh                 # unit tests (AddressSanitizer and UBSan on)
swift build -c release       # app and command-line tool
scripts/make-app.sh          # dist/JoyMouse.app
scripts/make-dmg.sh          # dist/JoyMouse-<version>.dmg and .zip
```

`make-app.sh` signs with a self-signed certificate named "JoyMouse Dev" so macOS keeps the Accessibility permission across rebuilds; it prints the one-time steps to create it. `--adhoc` skips it, but then the permission resets on every build.

The command-line tool does the same without a menu bar icon:

```sh
.build/release/joymouse-cli [--config PATH] [--verbose] [--monitor]
```

## License

MIT. See [LICENSE](LICENSE).
