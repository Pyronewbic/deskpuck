# Security

Please report vulnerabilities privately through the repository's **Security** tab (**Report a vulnerability**), not in a public issue. Only the latest release gets fixes. This is a one-person project, so replies are best effort.

Deskpuck has Accessibility permission, so it can post any keyboard and mouse input. The parts most worth scrutiny are:

- the report parser, which reads data from the paired Joy-Con, and during a pairing window from any nearby device that advertises as a Joy-Con 2;
- the `config.json` and `pairing.json` loaders, which decide which keys get pressed and which device may connect;
- the daily update check: one HTTPS request to github.com for the latest-release redirect. Its answer is treated as untrusted (only this repository's tag page with a plain version number is accepted), and the page it opens is a fixed address;
- the one-click update, the only code Deskpuck downloads: size-capped files from fixed github.com addresses, redirected only to `release-assets.githubusercontent.com`. Nothing is unpacked or run until the minisign signature over `SHA256SUMS` verifies with the built-in [release key](packaging/minisign.pub) and the package matches its line there; only a newer version is accepted, and on the Mac the new app must carry this app's code signature. Withholding updates is not prevented.

The release key's secret half is on the maintainer's Mac only, passphrase-protected. With one maintainer there is no second signer. Loss and rotation: [RELEASING.md](RELEASING.md#the-release-key).

## Known limitation

Deskpuck connects only to the paired Joy-Con, except during the 60 seconds after you choose **Pair New Joy-Con...**, when the first device that advertises as a Joy-Con 2 is paired. Pairing remembers the Bluetooth identity the Mac sees; it identifies the Joy-Con but does not authenticate it, so a device that copies your Joy-Con's Bluetooth address could still connect. Use **Pause Mouse Control** or quit Deskpuck when you are not using the Joy-Con.

On Linux, the `deskpuck` tray's menu is served over D-Bus, so other programs running as you can choose its items, including **Pair New Joy-Con...**. Those programs can already rewrite `pairing.json` or post input through `/dev/uinput`, so this gives them nothing new.
