Deskpuck for Linux
==================

Use a Nintendo Switch 2 Joy-Con as a mouse and keyboard.

Install for yourself (no root needed):  ./install.sh
Then start Deskpuck from your app menu. Its icon appears in the system tray
(on GNOME, turn on the AppIndicator extension). Right-click it for the menu;
choose Pair New Joy-Con..., then hold SYNC on the Joy-Con. Left-click the
icon for Settings. Remove it again with:  ./install.sh --uninstall

Moving the pointer needs write access to /dev/uinput; see the README below.

deskpuck-cli is the command-line tool: deskpuck-cli --help.

More: https://github.com/Pyronewbic/deskpuck
