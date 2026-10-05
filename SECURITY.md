# Security

Please report vulnerabilities privately through the repository's **Security** tab (**Report a vulnerability**), not in a public issue. Only the latest release gets fixes. This is a one-person project, so replies are best effort.

Deskpuck has Accessibility permission, so it can post any keyboard and mouse input. The parts most worth scrutiny are:

- the report parser, which reads data from any nearby Bluetooth device that advertises as a Joy-Con 2;
- the `config.json` loader, which decides which keys get pressed.
