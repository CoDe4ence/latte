//! WCAG 2 contrast.

use super::oklch::Rgb;

/// Relative luminance (WCAG 2), `0..=1`.
pub fn luminance(c: Rgb) -> f64 {
    let lin = |v: f64| if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// The contrast ratio of two colours, `1..=21`.
pub fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// What a contrast ratio passes: "AAA", "AA", "AA large" (3:1, also the UI-component minimum) or
/// "fail".
pub fn grade(ratio: f64) -> &'static str {
    if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA large"
    } else {
        "fail"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Rgb {
        Rgb::from_hex(s).unwrap_or(Rgb::BLACK)
    }

    #[test]
    fn known_pairs() {
        assert!((ratio(Rgb::BLACK, Rgb::WHITE) - 21.0).abs() < 1e-9);
        assert!((ratio(hex("#777777"), Rgb::WHITE) - 4.48).abs() < 0.01);
        assert!((ratio(hex("#767676"), Rgb::WHITE) - 4.54).abs() < 0.01);
        assert_eq!(ratio(hex("#0D99FF"), hex("#0D99FF")), 1.0);
        assert_eq!(grade(4.54), "AA");
        assert_eq!(grade(4.48), "AA large");
        assert_eq!(grade(2.0), "fail");
    }
}
