# Deskpuck

Use a Nintendo Switch 2 Joy-Con (R) as a mouse and keyboard on a Mac, Windows or Linux: slide it on a desk to move the pointer, click with the shoulder buttons, scroll with the stick, and map the other buttons to keys.

<!-- demo: 5-10 s GIF of a Joy-Con sliding on a desk, pointer following -->

It runs as a normal app, with no driver, kernel extension or root access.

## Install

Download from the [latest release](https://github.com/Pyronewbic/deskpuck/releases/latest):

| Runs on | Download |
| ------- | -------- |
| Apple Silicon Macs, macOS 13 or later (tested on 26) | `Deskpuck-<version>-macos-arm64.dmg` |
| Windows 10 and 11 on x86-64 (tested on 11) | `Deskpuck-<version>-setup.exe` |
| Linux on x86-64 with BlueZ (tested on CachyOS, KDE Plasma on Wayland) | `Deskpuck-<version>-linux-x86_64.tar.gz` |

`SHA256SUMS` on the same page lists each file's checksum. To check that a Windows or Linux download was built from this repository by its release workflow, run `gh attestation verify <file> --repo Pyronewbic/deskpuck` with the [GitHub CLI](https://cli.github.com).

### macOS

1. Open the disk image and drag Deskpuck to Applications. Open Deskpuck; macOS blocks it the first time because it is not notarized by Apple. Click **Done**, open **System Settings > Privacy & Security**, and click **Open Anyway** next to the Deskpuck message.
2. When asked, allow **Bluetooth** and **Accessibility** access. Accessibility is what lets Deskpuck move the pointer and press keys.
3. Choose **Pair New Joy-Con...** from the menu bar icon, then hold the **SYNC** button on the Joy-Con until its lights flash, within 60 seconds. From then on only this Joy-Con connects; hold SYNC to reconnect it.

<details>
<summary>No Open Anyway button?</summary>

Run this in Terminal, then open Deskpuck again:

```sh
xattr -dr com.apple.quarantine /Applications/Deskpuck.app
```

</details>

### Windows

1. Run the installer. Windows warns that it protected your PC, because Deskpuck is not code-signed: click **More info**, then **Run anyway**. It installs for you alone, without admin rights. To skip the installer, unzip `Deskpuck-<version>-windows-x86_64.zip` anywhere and run `deskpuck.exe`.
2. Right-click Deskpuck's icon in the system tray (it may be under the **^** arrow), choose **Pair New Joy-Con...**, then hold **SYNC** on the Joy-Con within 60 seconds. Do not pair the Joy-Con in Windows Settings.

### Linux

1. Unpack the archive and run `./install.sh` in its folder. It installs Deskpuck for you alone, in `~/.local`, and adds it to the app menu; `./install.sh --uninstall` removes it.
2. Start Deskpuck from the app menu. On GNOME, its tray icon needs the AppIndicator extension. Right-click the icon, choose **Pair New Joy-Con...**, then hold **SYNC** within 60 seconds.

<details>
<summary>The pointer does not move?</summary>

Moving the pointer needs write access to `/dev/uinput`. Some systems give it already (Steam's controller support does); if not, allow it once, as root. This gives the person at the screen access to `/dev/uinput` only, not to other input devices:

```sh
sudo tee /etc/udev/rules.d/60-deskpuck-uinput.rules <<'EOF'
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
EOF
sudo udevadm control --reload-rules && sudo udevadm trigger --name-match=uinput
```

</details>

## Use

| Joy-Con (R)          | Default                         |
| -------------------- | ------------------------------- |
| Slide on a surface   | Move the pointer                |
| R / ZR               | Left click / right click        |
| Stick up and down    | Scroll                          |
| Stick click          | Return                          |
| X / B / Y / A        | Up / Down / Left / Right arrows |

Change the key mappings, pointer speed, scrolling, key repeat and theme in **Settings...**, from the menu bar icon on a Mac or a left click on the tray icon elsewhere. A button can also press a recorded shortcut such as Control+C, or act as a modifier: held, or latched on with one tap and off with the next. Settings are saved in `config.json`, with the pairing beside it: in `~/Library/Application Support/Deskpuck` on a Mac, `%APPDATA%\Deskpuck` on Windows and `~/.config/deskpuck` on Linux.

**Pause Mouse Control** stops all input from the Joy-Con but keeps it connected, so resuming is instant; quit Deskpuck to release it completely. **Start at Login** starts Deskpuck when you log in. To use a different Joy-Con, choose **Pair New Joy-Con...** again; it replaces the paired one.

## Troubleshooting

- **The pointer does not move (Mac):** check that Deskpuck is on in **System Settings > Privacy & Security > Accessibility**. If it is on but still nothing happens, reset the permission and allow it again: `tccutil reset Accessibility com.pyronewbic.deskpuck`
- **The pointer does not move (Linux):** Deskpuck needs write access to `/dev/uinput` (Install, Linux).
- **The Joy-Con does not connect:** if the menu says Not paired, pair it first (see Install). Otherwise hold SYNC until the lights flash, and make sure it is not connected to a Switch or another computer at the same time. If the menu says the Joy-Con is in use by another app, quit that app (for example `deskpuck-cli`).
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
