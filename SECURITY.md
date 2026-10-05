# Security

Please report vulnerabilities privately through the repository's **Security** tab (**Report a vulnerability**), not in a public issue. Only the latest release gets fixes. This is a one-person project, so replies are best effort.

Deskpuck has Accessibility permission, so it can post any keyboard and mouse input. The parts most worth scrutiny are:

- the report parser, which reads data from any nearby Bluetooth device that advertises as a Joy-Con 2;
- the `config.json` loader, which decides which keys get pressed.

## Known limitation

While Deskpuck is searching (not paused, no Joy-Con connected), it connects to the first nearby device that advertises as a Joy-Con 2. It does not yet remember which Joy-Con is yours, so a device built to imitate one could move the pointer, click, and press the keys mapped in Settings. Use **Pause Mouse Control** or quit Deskpuck when you are not using the Joy-Con. Pairing with a specific Joy-Con is planned; it will not prevent a device that copies your Joy-Con's Bluetooth address.
