//! sRGB ↔ OKLab ↔ OKLCH (Björn Ottosson's OKLab), hex parsing and formatting, and gamut mapping
//! by reducing chroma.

/// An sRGB colour, components in `0..=1` (gamma-encoded).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// A colour in OKLCH: lightness `0..=1`, chroma `0..~0.4`, hue in degrees `0..360`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_gamma(c: f64) -> f64 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

impl Rgb {
    pub const WHITE: Rgb = Rgb { r: 1.0, g: 1.0, b: 1.0 };
    pub const BLACK: Rgb = Rgb { r: 0.0, g: 0.0, b: 0.0 };

    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `#` optional; alpha ignored).
    pub fn from_hex(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let full: String = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => h[..6].to_string(),
            _ => return None,
        };
        let byte = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).ok().map(|v| f64::from(v) / 255.0);
        Some(Rgb { r: byte(0)?, g: byte(2)?, b: byte(4)? })
    }

    /// The 8-bit channels, rounded and clamped.
    pub fn bytes(self) -> [u8; 3] {
        let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [q(self.r), q(self.g), q(self.b)]
    }

    /// `#RRGGBB`, upper case.
    pub fn to_hex(self) -> String {
        let [r, g, b] = self.bytes();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Whether every channel is within `0..=1` (with a little slack for float error).
    pub fn in_gamut(self) -> bool {
        const E: f64 = 1e-6;
        [self.r, self.g, self.b].iter().all(|v| (-E..=1.0 + E).contains(v))
    }

    pub fn to_oklch(self) -> Oklch {
        let (r, g, b) = (to_linear(self.r), to_linear(self.g), to_linear(self.b));
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
        let ok_l = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
        let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
        let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
        let c = (a * a + bb * bb).sqrt();
        let h = if c < 1e-9 { 0.0 } else { bb.atan2(a).to_degrees().rem_euclid(360.0) };
        Oklch { l: ok_l, c, h }
    }
}

impl Oklch {
    /// The colour in sRGB, unclamped (it may be out of gamut).
    pub fn to_rgb_unclamped(self) -> Rgb {
        let (a, b) = (self.c * self.h.to_radians().cos(), self.c * self.h.to_radians().sin());
        let l = self.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
        let m = self.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
        let s = self.l - 0.089_484_177_5 * a - 1.291_485_548 * b;
        let (l, m, s) = (l * l * l, m * m * m, s * s * s);
        let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
        let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
        let bl = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s;
        Rgb { r: gamma_signed(r), g: gamma_signed(g), b: gamma_signed(bl) }
    }

    /// The closest colour in sRGB with the same lightness and hue: chroma is reduced until it fits.
    pub fn to_rgb(self) -> Rgb {
        let l = self.l.clamp(0.0, 1.0);
        let fits = |c: f64| Oklch { l, c, h: self.h }.to_rgb_unclamped().in_gamut();
        if fits(self.c) {
            return clamp(Oklch { l, ..self }.to_rgb_unclamped());
        }
        let (mut lo, mut hi) = (0.0, self.c.max(0.0));
        for _ in 0..24 {
            let mid = (lo + hi) / 2.0;
            if fits(mid) { lo = mid } else { hi = mid }
        }
        clamp(Oklch { l, c: lo, h: self.h }.to_rgb_unclamped())
    }
}

fn gamma_signed(v: f64) -> f64 {
    if v < 0.0 { -to_gamma(-v) } else { to_gamma(v) }
}

fn clamp(c: Rgb) -> Rgb {
    Rgb { r: c.r.clamp(0.0, 1.0), g: c.g.clamp(0.0, 1.0), b: c.b.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        assert_eq!(Rgb::from_hex("#f0a").map(Rgb::to_hex).as_deref(), Some("#FF00AA"));
        assert_eq!(Rgb::from_hex("0D99FF80").map(Rgb::to_hex).as_deref(), Some("#0D99FF"));
        assert_eq!(Rgb::from_hex("#12345"), None);
        assert_eq!(Rgb::from_hex("red"), None);
    }

    #[test]
    fn oklch_round_trips() {
        for hex in ["#000000", "#FFFFFF", "#0D99FF", "#DC2626", "#16A34A", "#7B0051", "#808080"] {
            let c = Rgb::from_hex(hex).unwrap_or(Rgb::BLACK);
            assert_eq!(c.to_oklch().to_rgb().to_hex(), hex, "{hex}");
        }
    }

    #[test]
    fn known_values() {
        let white = Rgb::WHITE.to_oklch();
        assert!((white.l - 1.0).abs() < 1e-3 && white.c < 1e-3);
        let black = Rgb::BLACK.to_oklch();
        assert!(black.l.abs() < 1e-6);
        // sRGB red is about L 0.628, C 0.258, H 29.2.
        let red = Rgb { r: 1.0, g: 0.0, b: 0.0 }.to_oklch();
        assert!((red.l - 0.628).abs() < 2e-3 && (red.c - 0.258).abs() < 2e-3 && (red.h - 29.23).abs() < 0.5, "{red:?}");
    }

    #[test]
    fn out_of_gamut_maps_in() {
        let c = Oklch { l: 0.9, c: 0.4, h: 260.0 }.to_rgb();
        assert!(c.in_gamut());
        assert!((c.to_oklch().l - 0.9).abs() < 0.01);
    }
}
