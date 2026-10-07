//! Slider value text, spelled as the Mac settings window spells it.

/// "0.25x", "1x", "1.75x": two decimals with trailing zeros trimmed.
pub fn speed(speed: f64) -> String {
    let mut text = format!("{speed:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text + "x"
}

/// Below 1/s (only a hand-edited interval) the rate needs a decimal.
pub fn rate(rate: f64) -> String {
    if rate < 1.0 { format!("{rate:.1}/s") } else { format!("{rate:.0}/s") }
}

/// "0.15 s", "1.00 s".
pub fn delay(delay: f64) -> String {
    format!("{delay:.2} s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_trims_trailing_zeros() {
        assert_eq!(speed(0.25), "0.25x");
        assert_eq!(speed(0.5), "0.5x");
        assert_eq!(speed(1.0), "1x");
        assert_eq!(speed(1.75), "1.75x");
        assert_eq!(speed(4.0), "4x");
        // A hand-edited value is shown rounded, not trimmed to nothing.
        assert_eq!(speed(10.0), "10x");
        assert_eq!(speed(1.234), "1.23x");
    }

    #[test]
    fn rate_shows_a_decimal_only_below_one() {
        assert_eq!(rate(0.5), "0.5/s");
        assert_eq!(rate(1.0), "1/s");
        assert_eq!(rate(30.0), "30/s");
    }

    #[test]
    fn delay_keeps_two_decimals() {
        assert_eq!(delay(0.15), "0.15 s");
        assert_eq!(delay(1.0), "1.00 s");
    }
}
