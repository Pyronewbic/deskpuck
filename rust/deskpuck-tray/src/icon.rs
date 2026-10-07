//! The tray icon, drawn in code: an indigo puck, dimmed until a Joy-Con is
//! connected, with pause bars while paused and an amber dot while a modifier
//! is latched (a latch changes every click and key, so it shows by the icon).

pub const SIZE: u32 = 32;

const INDIGO: [u8; 3] = [0x6A, 0x4B, 0xD8];
const INDIGO_DARK: [u8; 3] = [0x33, 0x27, 0x8F];
const AMBER: [u8; 3] = [0xFF, 0xC2, 0x47];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const DIMMED_ALPHA: u8 = 0x70;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub connected: bool,
    pub paused: bool,
    pub latched: bool,
}

/// Straight (not premultiplied) RGBA rows, SIZE x SIZE.
pub fn rgba(look: Look) -> Vec<u8> {
    let alpha = if look.connected && !look.paused { 0xFF } else { DIMMED_ALPHA };
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let from_center = distance(px, py, 16.0, 16.0);
            let mut color = None;
            if from_center <= 14.5 {
                color = Some((if from_center > 12.5 { INDIGO_DARK } else { INDIGO }, alpha));
            }
            if look.paused && from_center <= 12.5 && (py - 16.0).abs() <= 6.0 {
                let bar = |left: f32| px >= left && px <= left + 3.0;
                if bar(10.5) || bar(18.5) {
                    color = Some((WHITE, 0xFF));
                }
            }
            if look.latched {
                let from_dot = distance(px, py, 26.0, 26.0);
                if from_dot <= 5.5 {
                    color = Some((if from_dot > 4.0 { INDIGO_DARK } else { AMBER }, 0xFF));
                }
            }
            if let Some(([r, g, b], a)) = color {
                let i = ((y * SIZE + x) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&[r, g, b, a]);
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
    fn connected_puck_is_opaque_indigo_with_clear_corners() {
        let pixels = rgba(PLAIN);
        assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
        assert_eq!(at(&pixels, 16, 16), [0x6A, 0x4B, 0xD8, 0xFF]);
        assert_eq!(at(&pixels, 16, 2)[..3], INDIGO_DARK, "rim");
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(at(&pixels, x, y)[3], 0, "corner {x},{y}");
        }
    }

    #[test]
    fn dimmed_until_connected_and_while_paused() {
        for look in [Look { connected: false, ..PLAIN }, Look { paused: true, ..PLAIN }] {
            assert_eq!(at(&rgba(look), 16, 4)[3], DIMMED_ALPHA, "{look:?}");
        }
    }

    #[test]
    fn paused_shows_two_white_bars() {
        let pixels = rgba(Look { paused: true, ..PLAIN });
        assert_eq!(at(&pixels, 11, 16), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(at(&pixels, 19, 16), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(at(&pixels, 15, 16)[..3], INDIGO, "gap between the bars");
        assert_eq!(at(&rgba(PLAIN), 11, 16)[..3], INDIGO, "no bars unless paused");
    }

    #[test]
    fn latched_adds_an_opaque_amber_dot_even_when_dimmed() {
        let look = Look { connected: false, latched: true, ..PLAIN };
        assert_eq!(at(&rgba(look), 26, 26), [0xFF, 0xC2, 0x47, 0xFF]);
        assert_ne!(at(&rgba(Look { connected: false, ..PLAIN }), 26, 26)[..3], AMBER);
    }
}
