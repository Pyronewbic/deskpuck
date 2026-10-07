# Deskpuck

Use a Nintendo Switch 2 Joy-Con (R) as a mouse and keyboard on a Mac, Windows or Linux: slide it on a desk to move the pointer, click with the shoulder buttons, scroll with the stick, and map the other buttons to keys.

It runs as a normal app, with no driver, kernel extension or root access. It runs on:

- Apple Silicon Macs. Tested on macOS 26; the build targets macOS 13 and later, untested there.
- Windows 10 and 11 on x86-64 PCs. Tested on Windows 11.
- Linux on x86-64 with BlueZ. Tested on CachyOS with KDE Plasma (Wayland).

## Install

### macOS

1. Download `Deskpuck-<version>-macos-arm64.dmg` from the [latest release](https://github.com/Pyronewbic/deskpuck/releases/latest), open it, and drag Deskpuck to Applications. `SHA256SUMS` on the same page lists its checksum.
2. Open Deskpuck. macOS blocks it the first time because it is not notarized by Apple. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll to Security, and click **Open Anyway** next to the Deskpuck message. Confirm with your password or Touch ID.
4. When asked, allow **Bluetooth** and **Accessibility** access. Accessibility is what lets Deskpuck move the pointer and press keys.
5. Choose **Pair New Joy-Con...** from the menu bar icon, then hold the **SYNC** button on the Joy-Con until its lights flash, within 60 seconds. The menu bar icon fills in once it connects. From then on only this Joy-Con connects; hold SYNC to reconnect it.

If step 3 shows no Open Anyway button, run this in Terminal instead:

```sh
xattr -dr com.apple.quarantine /Applications/Deskpuck.app
```

### Windows

1. Download `Deskpuck-<version>-setup.exe` from the [latest release](https://github.com/Pyronewbic/deskpuck/releases/latest) and run it. Windows warns that it protected your PC, because Deskpuck is not code-signed: click **More info**, then **Run anyway**. It installs for you alone, without admin rights, and can start Deskpuck when you log in. To skip the installer, unzip `Deskpuck-<version>-windows-x86_64.zip` anywhere and run `deskpuck.exe`.
2. Deskpuck's icon appears in the system tray (it may be under the **^** arrow). Right-click it, choose **Pair New Joy-Con...**, then hold **SYNC** on the Joy-Con within 60 seconds. Do not pair the Joy-Con in Windows Settings. Left-click the icon for Settings.

### Linux

1. Download `Deskpuck-<version>-linux-x86_64.tar.gz` from the [latest release](https://github.com/Pyronewbic/deskpuck/releases/latest), unpack it, and run `./install.sh` in its folder. It installs Deskpuck for you alone, in `~/.local`, and adds it to the app menu; `./install.sh --uninstall` removes it.
2. Moving the pointer needs write access to `/dev/uinput`. Some systems give it already (Steam's controller support does); if the pointer does not move, allow it once, as root. This gives the person at the screen access to `/dev/uinput` only, not to other input devices:
   ```sh
   sudo tee /etc/udev/rules.d/60-deskpuck-uinput.rules <<'EOF'
   KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
   EOF
   sudo udevadm control --reload-rules && sudo udevadm trigger --name-match=uinput
   ```
3. Start Deskpuck from the app menu. On GNOME, its tray icon needs the AppIndicator extension. Right-click the icon, choose **Pair New Joy-Con...**, then hold **SYNC** within 60 seconds. Left-click the icon for Settings.

To check that a Windows or Linux download was built from this repository by its release workflow, run `gh attestation verify <file> --repo Pyronewbic/deskpuck` with the [GitHub CLI](https://cli.github.com).

## Use

| Joy-Con (R)          | Default                         |
| -------------------- | ------------------------------- |
| Slide on a surface   | Move the pointer                |
| R / ZR               | Left click / right click        |
| Stick up and down    | Scroll                          |
| Stick click          | Return                          |
| X / B / Y / A        | Up / Down / Left / Right arrows |

Change the key mappings, pointer speed, scrolling, key repeat and theme in **Settings...**, from the menu bar icon on a Mac or the tray icon on Windows and Linux (a left click opens it there too). A button can also press a shortcut such as Control+C: pick **Record Shortcut...** for it and press the keys. Shortcuts fire once per press and do not repeat. A button can also be a modifier key: **Shift (while held)** acts as Shift while you hold it, so R gives a Shift-click; **Shift (tap to latch)** turns Shift on with one tap and off with the next, and the icon shows it while it is on. Settings are saved in `config.json`: in `~/Library/Application Support/Deskpuck` on a Mac, `%APPDATA%\Deskpuck` on Windows and `~/.config/deskpuck` on Linux.

To use a different Joy-Con, choose **Pair New Joy-Con...** again; it replaces the paired one. The pairing is saved beside the settings, in `pairing.json`.

**Pause Mouse Control** stops the Joy-Con from moving the pointer or pressing keys, and stops Deskpuck from looking for a Joy-Con. A connected Joy-Con stays connected, so resuming is instant. Quit Deskpuck to release it completely. **Start at Login** starts Deskpuck when you log in.

## Troubleshooting

- **The pointer does not move (Mac):** check that Deskpuck is on in **System Settings > Privacy & Security > Accessibility**. If it is on but still nothing happens, reset the permission and allow it again:
  ```sh
  tccutil reset Accessibility com.pyronewbic.deskpuck
  ```
- **The Joy-Con does not connect:** if the menu says Not paired, pair it first (see Install). Otherwise hold SYNC until the lights flash, and make sure it is not connected to a Switch or another computer at the same time. If the menu says the Joy-Con is in use by another app, quit that app (for example `deskpuck-cli`).
- **The pointer does not move (Linux):** Deskpuck needs write access to `/dev/uinput` (Install, Linux, step 2).
- **Windows: the Joy-Con stops working in an admin window:** Windows does not let ordinary apps send input to apps running as administrator (such as an admin PowerShell). Switch away with the mouse, the keyboard or Alt+Tab, and the Joy-Con works again.

## Build from source

See [CONTRIBUTING.md](CONTRIBUTING.md). How the code is laid out: [ARCHITECTURE.md](ARCHITECTURE.md).

## Acknowledgements

Deskpuck learned a great deal from the following projects.

[seitanmen/Joycon2forMac](https://github.com/seitanmen/Joycon2forMac)

[ndeadly/switch2_controller_research](https://github.com/ndeadly/switch2_controller_research)

We thank the authors for their work on the Joy-Con 2 Bluetooth protocol.

Deskpuck is not affiliated with or endorsed by Nintendo. Nintendo Switch and Joy-Con are trademarks of Nintendo.

## License

MIT. See [LICENSE](LICENSE).
