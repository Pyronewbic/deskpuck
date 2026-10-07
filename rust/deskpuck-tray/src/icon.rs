//! The tray icon: Deskpuck's logo (rust/icons, rendered from the Mac app's
//! icon by scripts/make-icons.sh), dimmed until a Joy-Con is connected,
//! with pause bars while paused and an amber dot while a modifier is latched
//! (a latch changes every click and key, so it shows by the icon).

use std::sync::OnceLock;

pub const SIZE: u32 = 32;

const LOGO_PNG: &[u8] = include_bytes!("../../icons/deskpuck-32.png");
const INDIGO_DARK: [u8; 3] = [0x33, 0x27, 0x8F];
const AMBER: [u8; 3] = [0xFF, 0xC2, 0x47];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const DIMMED_ALPHA: u8 = 0x70;
/// Top right: the logo's own amber pointer fills the bottom right.
const LATCH_DOT: (f32, f32) = (26.0, 6.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub connected: bool,
    pub paused: bool,
    pub latched: bool,
}

/// Straight (not premultiplied) RGBA rows, SIZE x SIZE, from a PNG of that size.
pub fn decode(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    let rgba = info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight;
    (rgba && info.width == SIZE && info.height == SIZE).then(|| {
        pixels.truncate(info.buffer_size());
        pixels
    })
}

/// The logo, decoded once; blank if it could not be (a test proves it can).
fn logo() -> &'static [u8] {
    static LOGO: OnceLock<Vec<u8>> = OnceLock::new();
    LOGO.get_or_init(|| decode(LOGO_PNG).unwrap_or_else(|| vec![0; (SIZE * SIZE * 4) as usize]))
}

/// Straight (not premultiplied) RGBA rows, SIZE x SIZE.
pub fn rgba(look: Look) -> Vec<u8> {
    let mut pixels = logo().to_vec();
    let dimmed = !look.connected || look.paused;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = ((y * SIZE + x) * 4) as usize;
            if dimmed {
                pixels[i + 3] = (u16::from(pixels[i + 3]) * u16::from(DIMMED_ALPHA) / 255) as u8;
            }
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut color = None;
            if look.paused && (py - 16.0).abs() <= 7.0 {
                // White bars with a dark edge, readable on any part of the logo.
                let bar = |left: f32, inset: f32| px >= left + inset && px <= left + 4.0 - inset;
                if bar(10.0, 0.0) || bar(18.0, 0.0) {
                    let edge = !(bar(10.0, 1.0) || bar(18.0, 1.0)) || (py - 16.0).abs() > 6.0;
                    color = Some(if edge { INDIGO_DARK } else { WHITE });
                }
            }
            if look.latched {
                let from_dot = distance(px, py, LATCH_DOT.0, LATCH_DOT.1);
                if from_dot <= 5.5 {
                    color = Some(if from_dot > 4.0 { INDIGO_DARK } else { AMBER });
                }
            }
            if let Some([r, g, b]) = color {
                pixels[i..i + 4].copy_from_slice(&[r, g, b, 0xFF]);
            }
        }
    }
    pixels
}

fn distance(x: f32, y: f32, cx: f32, cy: f32) -> f32 {
    ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * SIZE + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    }

    const PLAIN: Look = Look { connected: true, paused: false, latched: false };

    #[test]
    fn the_logo_decodes_to_an_rgba_square_with_clear_corners() {
        let logo = decode(LOGO_PNG).expect("the committed logo decodes");
        assert_eq!(logo.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(rgba(PLAIN), logo, "connected: the logo as it is");
        assert_eq!(at(&logo, 16, 16)[3], 0xFF, "opaque in the middle");
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert!(at(&logo, x, y)[3] < 0x40, "rounded corner {x},{y}");
        }
    }

    #[test]
    fn a_png_of_the_wrong_size_or_kind_is_refused() {
        assert_eq!(decode(include_bytes!("../../icons/deskpuck-64.png")), None);
        assert_eq!(decode(b"not a png"), None);
        assert_eq!(decode(&LOGO_PNG[..LOGO_PNG.len() / 2]), None, "truncated");
    }

    #[test]
    fn dimmed_until_connected_and_while_paused() {
        let full = at(&rgba(PLAIN), 16, 26)[3];
        let dim = (u16::from(full) * u16::from(DIMMED_ALPHA) / 255) as u8;
        for look in [Look { connected: false, ..PLAIN }, Look { paused: true, ..PLAIN }] {
            assert_eq!(at(&rgba(look), 16, 26)[3], dim, "{look:?}");
        }
    }

    #[test]
    fn paused_shows_two_white_bars() {
        let pixels = rgba(Look { paused: true, ..PLAIN });
        assert_eq!(at(&pixels, 12, 16), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(at(&pixels, 20, 16), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(at(&pixels, 10, 16)[..3], INDIGO_DARK, "edge");
        assert_ne!(at(&pixels, 16, 16)[..3], WHITE, "gap between the bars");
        assert_ne!(at(&rgba(PLAIN), 12, 16), [0xFF, 0xFF, 0xFF, 0xFF], "no bars unless paused");
    }

    #[test]
    fn latched_adds_an_opaque_amber_dot_even_when_dimmed() {
        let look = Look { connected: false, latched: true, ..PLAIN };
        assert_eq!(at(&rgba(look), 26, 6), [0xFF, 0xC2, 0x47, 0xFF]);
        assert_ne!(at(&rgba(Look { connected: false, ..PLAIN }), 26, 6)[..3], AMBER);
    }
}
