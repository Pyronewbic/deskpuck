# Deskpuck

Use a Nintendo Switch 2 Joy-Con (R) as a Mac mouse and keyboard: slide it on a desk to move the pointer, click with the shoulder buttons, scroll with the stick, and map the other buttons to keys.

It runs as a normal app with Bluetooth and Accessibility permission. No kernel extension, and no need to turn off System Integrity Protection.

Tested on macOS 26 on Apple Silicon. The build targets macOS 13 and later, untested there.

## Install

1. Open `Deskpuck-<version>.dmg` and drag Deskpuck to Applications.
2. Open Deskpuck. macOS blocks it the first time because it is not notarized by Apple. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll to Security, and click **Open Anyway** next to the Deskpuck message. Confirm with your password or Touch ID.
4. When asked, allow **Bluetooth** and **Accessibility** access. Accessibility is what lets Deskpuck move the pointer and press keys.
5. Hold the **SYNC** button on the Joy-Con until its lights flash. The menu bar icon fills in once it connects.

If step 3 shows no Open Anyway button, run this in Terminal instead:

```sh
xattr -dr com.apple.quarantine /Applications/Deskpuck.app
```

## Use

| Joy-Con (R)          | Default                         |
| -------------------- | ------------------------------- |
| Slide on a surface   | Move the pointer                |
| R / ZR               | Left click / right click        |
| Stick up and down    | Scroll                          |
| Stick click          | Return                          |
| X / B / Y / A        | Up / Down / Left / Right arrows |

Change the key mappings, pointer speed, scrolling and key repeat from the menu bar icon: **Settings...**. Settings are saved to `~/Library/Application Support/Deskpuck/config.json`.

**Pause Mouse Control** stops the Joy-Con from moving the pointer or pressing keys, and stops Deskpuck from looking for a Joy-Con. A connected Joy-Con stays connected, so resuming is instant. Quit Deskpuck to release it completely.

## Troubleshooting

- **The pointer does not move:** check that Deskpuck is on in **System Settings > Privacy & Security > Accessibility**. If it is on but still nothing happens, reset the permission and allow it again:
  ```sh
  tccutil reset Accessibility com.pyronewbic.deskpuck
  ```
- **The Joy-Con does not connect:** hold SYNC until the lights flash, and make sure it is not connected to a Switch or another computer at the same time.

## Linux and Windows

Deskpuck's core, in [`rust/`](rust), is cross-platform. Its input layer is tested on Linux (X11), but Bluetooth has not been tested on Linux or Windows yet, and there is no app for them. See [CONTRIBUTING.md](CONTRIBUTING.md#rust-workspace).

## Build from source

See [CONTRIBUTING.md](CONTRIBUTING.md). How the code is laid out: [ARCHITECTURE.md](ARCHITECTURE.md).

## Credits

Based on [seitanmen/Joycon2forMac](https://github.com/seitanmen/Joycon2forMac) (MIT). Bluetooth protocol details from [ndeadly/switch2_controller_research](https://github.com/ndeadly/switch2_controller_research).

Deskpuck is not affiliated with or endorsed by Nintendo. Nintendo Switch and Joy-Con are trademarks of Nintendo.

## License

MIT. See [LICENSE](LICENSE).
