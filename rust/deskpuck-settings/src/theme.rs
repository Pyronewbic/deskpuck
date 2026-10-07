//! Deskpuck's accent color, as the Mac settings window tints its controls.

use eframe::egui::{Color32, Context, Stroke, Theme};

/// Brand indigo; lighter in dark mode so controls keep their contrast.
pub fn accent(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(0x8C, 0x73, 0xF0),
        Theme::Light => Color32::from_rgb(0x5B, 0x3F, 0xD0),
    }
}

/// Tints what egui draws from the selection color: slider trails, selected
/// menu entries, the modifier chips, selected text.
pub fn apply(ctx: &Context) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            style.visuals.selection.bg_fill = accent(theme);
            style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            style.visuals.hyperlink_color = accent(theme);
            style.visuals.slider_trailing_fill = true;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_get_their_own_accent() {
        let ctx = Context::default();
        apply(&ctx);
        for theme in [Theme::Dark, Theme::Light] {
            let style = ctx.style_of(theme);
            assert_eq!(style.visuals.selection.bg_fill, accent(theme));
            assert_eq!(style.visuals.hyperlink_color, accent(theme));
            assert!(style.visuals.slider_trailing_fill);
        }
        assert_ne!(accent(Theme::Dark), accent(Theme::Light), "dark is lighter for contrast");
    }
}
