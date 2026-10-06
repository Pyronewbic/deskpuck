# Security

Please report vulnerabilities privately through the repository's **Security** tab (**Report a vulnerability**), not in a public issue. Only the latest release gets fixes. This is a one-person project, so replies are best effort.

Deskpuck has Accessibility permission, so it can post any keyboard and mouse input. The parts most worth scrutiny are:

- the report parser, which reads data from the paired Joy-Con, and during a pairing window from any nearby device that advertises as a Joy-Con 2;
- the `config.json` and `pairing.json` loaders, which decide which keys get pressed and which device may connect.

## Known limitation

Deskpuck connects only to the paired Joy-Con, except during the 60 seconds after you choose **Pair New Joy-Con...**, when the first device that advertises as a Joy-Con 2 is paired. Pairing remembers the Bluetooth identity the Mac sees; it identifies the Joy-Con but does not authenticate it, so a device that copies your Joy-Con's Bluetooth address could still connect. Use **Pause Mouse Control** or quit Deskpuck when you are not using the Joy-Con.
