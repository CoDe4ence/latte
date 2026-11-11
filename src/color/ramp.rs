//! Tonal ramps: a base colour becomes eleven steps (50…950) with even perceived lightness, made
//! in OKLCH. The base colour itself lands, exactly, on the step whose lightness is closest to it.

use super::oklch::{Oklch, Rgb};

/// The ramp's step names, lightest first.
pub const STEPS: [u16; 11] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

/// OKLCH lightness of each step.
const LIGHTNESS: [f64; 11] = [0.975, 0.935, 0.875, 0.800, 0.715, 0.635, 0.555, 0.475, 0.395, 0.320, 0.255];

/// How much of the base chroma each step keeps: the extremes are paler, as in hand-made ramps.
const CHROMA: [f64; 11] = [0.12, 0.25, 0.48, 0.75, 0.94, 1.0, 1.0, 0.92, 0.80, 0.66, 0.52];

/// One step of a ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swatch {
    pub step: u16,
    pub color: Rgb,
}

/// The index in [`STEPS`] of `step`, if it is one.
pub fn step_index(step: u16) -> Option<usize> {
    STEPS.iter().position(|s| *s == step)
}

/// The ramp of `base`. `pin` keeps the base colour exact on its closest step.
pub fn ramp(base: Rgb, pin: bool) -> Vec<Swatch> {
    let lch = base.to_oklch();
    let pinned = pin.then(|| {
        LIGHTNESS.iter().enumerate().min_by(|a, b| (a.1 - lch.l).abs().total_cmp(&(b.1 - lch.l).abs())).map_or(5, |(i, _)| i)
    });
    STEPS
        .iter()
        .enumerate()
        .map(|(i, &step)| {
            let color = if pinned == Some(i) {
                base
            } else {
                Oklch { l: LIGHTNESS[i], c: lch.c * CHROMA[i], h: lch.h }.to_rgb()
            };
            Swatch { step, color }
        })
        .collect()
}

/// A neutral (grey) ramp, tinted toward `hue` by `tint` (0: pure grey, 1: a noticeable tint).
pub fn neutral(hue: f64, tint: f64) -> Vec<Swatch> {
    let c = 0.03 * tint.clamp(0.0, 1.0);
    STEPS
        .iter()
        .enumerate()
        .map(|(i, &step)| Swatch { step, color: Oklch { l: LIGHTNESS[i], c: c * CHROMA[i].max(0.5), h: hue }.to_rgb() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lightness(r: &[Swatch]) -> Vec<f64> {
        r.iter().map(|s| s.color.to_oklch().l).collect()
    }

    #[test]
    fn ramps_get_darker_and_stay_in_gamut() {
        for hex in ["#4F46E5", "#DC2626", "#16A34A", "#D97706", "#0D99FF", "#FFEB3B", "#111111", "#FAFAFA"] {
            let base = Rgb::from_hex(hex).unwrap_or(Rgb::BLACK);
            let r = ramp(base, true);
            assert_eq!(r.len(), 11);
            let l = lightness(&r);
            assert!(l.windows(2).all(|w| w[0] > w[1]), "{hex}: {l:?}");
            assert!(r.iter().all(|s| s.color.in_gamut()), "{hex}");
        }
    }

    #[test]
    fn the_base_colour_is_kept() {
        let base = Rgb::from_hex("#4F46E5").unwrap_or(Rgb::BLACK);
        assert!(ramp(base, true).iter().any(|s| s.color.to_hex() == "#4F46E5"));
        assert!(!ramp(base, false).iter().any(|s| s.color.to_hex() == "#4F46E5"));
    }

    #[test]
    fn neutrals_are_nearly_grey() {
        let n = neutral(270.0, 0.0);
        assert!(n.iter().all(|s| s.color.to_oklch().c < 0.002));
        let l = lightness(&n);
        assert!(l.windows(2).all(|w| w[0] > w[1]));
        assert!(neutral(270.0, 1.0).iter().all(|s| s.color.to_oklch().c < 0.035));
    }
}
