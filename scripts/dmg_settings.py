# dmgbuild settings for the Deskpuck disk image. See scripts/make-dmg.sh.
app = defines["app"]  # noqa: F821 (provided by dmgbuild -D app=...)

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}

background = "builtin-arrow"
window_rect = ((200, 120), (640, 300))
default_view = "icon-view"
icon_size = 112
text_size = 13
show_icon_preview = False
icon_locations = {
    "Deskpuck.app": (160, 140),
    "Applications": (480, 140),
}
