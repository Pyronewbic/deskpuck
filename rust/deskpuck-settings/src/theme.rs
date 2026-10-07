//! The accent the window tints its controls with: the desktop's own accent
//! where it shares one, as the Mac app uses the macOS accent, and Deskpuck's
//! indigo where it does not.

use eframe::egui::style::{HandleShape, WidgetVisuals};
use eframe::egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, Vec2,
};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    pub dark: Color32,
    pub light: Color32,
}

impl Accent {
    /// Deskpuck's indigo; lighter in dark mode so controls keep their contrast.
    pub const BRAND: Accent = Accent {
        dark: Color32::from_rgb(0x8C, 0x73, 0xF0),
        light: Color32::from_rgb(0x5B, 0x3F, 0xD0),
    };

    pub fn for_theme(self, theme: Theme) -> Color32 {
        match theme {
            Theme::Dark => self.dark,
            Theme::Light => self.light,
        }
    }
}

/// Black or white, whichever reads better on `fill` (WCAG relative luminance).
pub fn text_on(fill: Color32) -> Color32 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let luminance =
        0.2126 * linear(fill.r()) + 0.7152 * linear(fill.g()) + 0.0722 * linear(fill.b());
    // Where black and white give the same contrast ratio.
    if luminance > 0.179 { Color32::BLACK } else { Color32::WHITE }
}

/// The colors of a grouped settings form, close to the Mac app's and to
/// Windows 11 Settings in each theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub window: Color32,
    pub card: Color32,
    pub card_stroke: Color32,
    pub separator: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_stroke: Color32,
    pub toggle_off: Color32,
}

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            window: Color32::from_gray(0x20),
            card: Color32::from_gray(0x2B),
            card_stroke: Color32::from_white_alpha(15),
            separator: Color32::from_white_alpha(20),
            control: Color32::from_gray(0x3A),
            control_hover: Color32::from_gray(0x44),
            control_stroke: Color32::from_white_alpha(20),
            toggle_off: Color32::from_gray(0x6B),
        },
        Theme::Light => Palette {
            window: Color32::from_gray(0xF3),
            card: Color32::WHITE,
            card_stroke: Color32::from_gray(0xE5),
            separator: Color32::from_black_alpha(20),
            control: Color32::from_gray(0xF0),
            control_hover: Color32::from_gray(0xE8),
            control_stroke: Color32::from_gray(0xD5),
            toggle_off: Color32::from_gray(0xC9),
        },
    }
}

/// Corner radius of buttons, pickers and segments.
pub const CONTROL_RADIUS: u8 = 5;
/// Corner radius of section cards and menus.
pub const CARD_RADIUS: u8 = 8;
/// Every picker is this wide, so they line up like the Mac's pop-up buttons.
pub const PICKER_WIDTH: f32 = 190.0;

fn control(fill: Color32, stroke: Stroke, text: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: stroke,
        corner_radius: CornerRadius::same(CONTROL_RADIUS),
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    }
}

/// The whole look for both themes, tinted with `accent`: slider trails,
/// selected entries and segments, switches, focus, the text cursor.
pub fn apply(ctx: &Context, accent: Accent) {
    for theme in [Theme::Dark, Theme::Light] {
        let fill = accent.for_theme(theme);
        let colors = palette(theme);
        ctx.style_mut_of(theme, |style| {
            style.text_styles = [
                (TextStyle::Heading, FontId::new(13.0, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
                (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
                (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
            ]
            .into();

            let spacing = &mut style.spacing;
            spacing.item_spacing = Vec2::new(8.0, 6.0);
            spacing.button_padding = Vec2::new(10.0, 5.0);
            spacing.interact_size.y = 28.0;
            spacing.combo_width = PICKER_WIDTH;
            spacing.slider_width = 160.0;
            spacing.slider_rail_height = 4.0;
            spacing.menu_margin = 4.into();

            let visuals = &mut style.visuals;
            let text = visuals.widgets.inactive.fg_stroke.color;
            let border = Stroke::new(1.0, colors.control_stroke);
            visuals.panel_fill = colors.window;
            visuals.window_fill = colors.card;
            visuals.extreme_bg_color = colors.control;
            visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
            visuals.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
            visuals.widgets.noninteractive.bg_fill = colors.card;
            visuals.widgets.noninteractive.weak_bg_fill = colors.card;
            visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.separator);
            visuals.widgets.inactive = control(colors.control, border, text);
            visuals.widgets.hovered = control(colors.control_hover, border, text);
            visuals.widgets.open = control(colors.control_hover, border, text);
            // egui draws a focused widget as active: the accent outline is the focus ring.
            visuals.widgets.active = control(colors.control_hover, Stroke::new(1.0, fill), text);
            visuals.handle_shape = HandleShape::Circle;
            visuals.selection.bg_fill = fill;
            visuals.selection.stroke = Stroke::new(1.0, text_on(fill));
            visuals.hyperlink_color = fill;
            visuals.text_cursor.stroke = Stroke::new(2.0, fill);
            visuals.slider_trailing_fill = true;
        });
    }
}

/// The desktop portal's accent-color: RGB in 0..=1. Anything outside that
/// range is how the portal says the desktop has no accent.
pub fn from_portal((r, g, b): (f64, f64, f64)) -> Option<Color32> {
    let channel = |v: f64| (0.0..=1.0).contains(&v).then(|| (v * 255.0).round() as u8);
    Some(Color32::from_rgb(channel(r)?, channel(g)?, channel(b)?))
}

/// Reads the system accent off the UI thread, so a slow or missing desktop
/// service never holds up the window; `poll` hands over a finished read.
#[derive(Default)]
pub struct AccentReader {
    pending: Option<Receiver<Option<Accent>>>,
}

impl AccentReader {
    /// Starts a read unless one is already running.
    pub fn refresh(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(system_accent());
        });
        self.pending = Some(rx);
    }

    pub fn reading(&self) -> bool {
        self.pending.is_some()
    }

    /// The accent to use once a read has finished: the system's, or the brand
    /// indigo when there is none.
    pub fn poll(&mut self) -> Option<Accent> {
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(found) => found.unwrap_or(Accent::BRAND),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Accent::BRAND,
        };
        self.pending = None;
        Some(result)
    }
}

#[cfg(windows)]
fn system_accent() -> Option<Accent> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    let settings = UISettings::new().ok()?;
    let color = |kind| settings.GetColorValue(kind).ok().map(|c| Color32::from_rgb(c.R, c.G, c.B));
    // The shades Windows' own controls use on dark and light backgrounds.
    Some(Accent {
        dark: color(UIColorType::AccentLight2)?,
        light: color(UIColorType::AccentDark1)?,
    })
}

#[cfg(target_os = "linux")]
fn system_accent() -> Option<Accent> {
    use std::time::Duration;
    let bus = zbus::blocking::connection::Builder::session()
        .ok()?
        .method_timeout(Duration::from_secs(2))
        .build()
        .ok()?;
    let reply = bus
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.freedesktop.appearance", "accent-color"),
        )
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    let rgb: (f64, f64, f64) = zbus::zvariant::Value::from(value).downcast().ok()?;
    let accent = from_portal(rgb)?;
    Some(Accent { dark: accent, light: accent })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_accent() -> Option<Accent> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_get_their_own_accent() {
        let ctx = Context::default();
        let accent = Accent {
            dark: Color32::from_rgb(0xF4, 0x9C, 0xD8),
            light: Color32::from_rgb(0x20, 0x30, 0x80),
        };
        apply(&ctx, accent);
        for theme in [Theme::Dark, Theme::Light] {
            let style = ctx.style_of(theme);
            let fill = accent.for_theme(theme);
            assert_eq!(style.visuals.selection.bg_fill, fill);
            assert_eq!(style.visuals.hyperlink_color, fill);
            assert_eq!(style.visuals.text_cursor.stroke.color, fill);
            assert_eq!(style.visuals.widgets.active.bg_stroke.color, fill, "focus ring");
            assert!(style.visuals.slider_trailing_fill);
            assert_eq!(style.visuals.panel_fill, palette(theme).window);
            assert_eq!(style.visuals.widgets.inactive.bg_fill, palette(theme).control);
            assert_eq!(style.spacing.combo_width, PICKER_WIDTH);
            assert_eq!(style.text_styles[&TextStyle::Body].size, 14.0);
        }
        // Light pink takes dark text, deep blue takes white.
        assert_eq!(ctx.style_of(Theme::Dark).visuals.selection.stroke.color, Color32::BLACK);
        assert_eq!(ctx.style_of(Theme::Light).visuals.selection.stroke.color, Color32::WHITE);
    }

    #[test]
    fn each_theme_has_its_own_palette_with_cards_set_off_from_the_window() {
        let (dark, light) = (palette(Theme::Dark), palette(Theme::Light));
        assert_ne!(dark, light);
        assert!(dark.card.r() > dark.window.r(), "dark cards sit lighter than the window");
        assert!(light.card.r() > light.window.r(), "light cards are white on grey");
    }

    #[test]
    fn text_on_picks_the_readable_colour() {
        assert_eq!(text_on(Color32::WHITE), Color32::BLACK);
        assert_eq!(text_on(Color32::BLACK), Color32::WHITE);
        assert_eq!(text_on(Accent::BRAND.light), Color32::WHITE);
        // The two greys either side of the crossover.
        assert_eq!(text_on(Color32::from_gray(0x75)), Color32::WHITE);
        assert_eq!(text_on(Color32::from_gray(0x76)), Color32::BLACK);
    }

    #[test]
    fn portal_colours_are_read_and_unset_ones_are_not() {
        assert_eq!(from_portal((0.0, 0.5, 1.0)), Some(Color32::from_rgb(0, 128, 255)));
        assert_eq!(from_portal((1.0, 1.0, 1.0)), Some(Color32::WHITE));
        // The portal's "no accent" value, and anything else out of range.
        assert_eq!(from_portal((-1.0, -1.0, -1.0)), None);
        assert_eq!(from_portal((0.5, 1.5, 0.5)), None);
        assert_eq!(from_portal((f64::NAN, 0.5, 0.5)), None);
    }

    #[test]
    fn a_finished_read_is_handed_over_once() {
        let mut reader = AccentReader::default();
        assert_eq!(reader.poll(), None, "nothing started yet");
        reader.refresh();
        assert!(reader.reading());
        let accent = loop {
            if let Some(accent) = reader.poll() {
                break accent;
            }
            std::thread::yield_now();
        };
        // No system accent where these tests run without a desktop: the brand.
        if cfg!(not(any(windows, target_os = "linux"))) {
            assert_eq!(accent, Accent::BRAND);
        }
        assert_eq!(reader.poll(), None, "handed over once");
        assert!(!reader.reading());
    }
}
