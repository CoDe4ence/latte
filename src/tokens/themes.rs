//! Themes: semantic colours with a Light and a Dark mode, each an alias of a Primitives step, and
//! a contrast report on the pairs that carry text and UI.

use super::{CollectionPlan, Kind, PRIMITIVES, THEMES, Val, VarPlan, VarRef, primitives};
use crate::color::{contrast, Rgb};
use crate::config::Config;

pub const LIGHT: &str = "Light";
pub const DARK: &str = "Dark";

pub fn collection(config: &Config) -> CollectionPlan {
    let vars = config
        .roles
        .iter()
        .map(|r| {
            let ramp = config.ramp_name(r.ramp);
            let alias = |step| Val::Alias(VarRef { collection: PRIMITIVES, name: primitives::var_name(&ramp, step) });
            VarPlan {
                name: r.name.clone(),
                kind: Kind::Color,
                values: vec![(LIGHT.into(), alias(r.light)), (DARK.into(), alias(r.dark))],
                scopes: Some(scopes_for(&r.name)),
                description: format!("{ramp} {} (light) / {} (dark)", r.light, r.dark),
            }
        })
        .collect();
    CollectionPlan { name: THEMES, modes: vec![LIGHT.into(), DARK.into()], vars }
}

/// Backgrounds fill frames and shapes, foregrounds colour text, borders stroke; the rest go
/// anywhere a colour does.
fn scopes_for(role: &str) -> Vec<&'static str> {
    match role.split('/').next().unwrap_or_default() {
        "bg" => vec!["FRAME_FILL", "SHAPE_FILL"],
        "fg" => vec!["TEXT_FILL", "SHAPE_FILL"],
        "border" => vec!["STROKE_COLOR"],
        _ => primitives::color_scopes(),
    }
}

/// A pair of roles whose contrast matters, and the ratio it needs.
pub const CHECKS: [(&str, &str, f64); 8] = [
    ("fg/default", "bg/default", 4.5),
    ("fg/muted", "bg/default", 4.5),
    ("fg/default", "bg/subtle", 4.5),
    ("accent/fg", "accent/default", 4.5),
    ("accent/default", "bg/default", 3.0),
    ("border/strong", "bg/default", 3.0),
    ("danger/default", "bg/default", 3.0),
    ("success/default", "bg/default", 3.0),
];

/// One line of the contrast report.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub fg: &'static str,
    pub bg: &'static str,
    pub mode: &'static str,
    pub ratio: f64,
    pub needs: f64,
}

impl Check {
    pub fn passes(&self) -> bool {
        self.ratio + 1e-9 >= self.needs
    }
}

/// The resolved colour of `role` in `mode`.
pub fn resolve(config: &Config, ramps: &[(String, Vec<crate::color::ramp::Swatch>)], role: &str, dark: bool) -> Option<Rgb> {
    let r = config.roles.iter().find(|r| r.name == role)?;
    primitives::lookup(ramps, &config.ramp_name(r.ramp), if dark { r.dark } else { r.light })
}

/// Contrast of every checked pair whose roles exist, in both modes.
pub fn report(config: &Config) -> Vec<Check> {
    let ramps = primitives::ramps(config);
    let mut out = Vec::new();
    for (mode, dark) in [(LIGHT, false), (DARK, true)] {
        for (fg, bg, needs) in CHECKS {
            if let (Some(a), Some(b)) = (resolve(config, &ramps, fg, dark), resolve(config, &ramps, bg, dark)) {
                out.push(Check { fg, bg, mode, ratio: contrast::ratio(a, b), needs });
            }
        }
    }
    out
}

/// The report as text for the panel: failures first, then a count of the passes.
pub fn report_text(config: &Config) -> String {
    let checks = report(config);
    let fails: Vec<_> = checks.iter().filter(|c| !c.passes()).collect();
    // One line: FigCraft's panel notes take no line breaks.
    let mut parts: Vec<String> = fails.iter().map(|c| format!("⚠ {} on {} ({}): {:.2}:1, needs {}:1", c.fg, c.bg, c.mode, c.ratio, c.needs)).collect();
    parts.push(format!("{} of {} contrast checks pass.", checks.len() - fails.len(), checks.len()));
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_pass_every_check() {
        let c = Config::default();
        let r = report(&c);
        assert_eq!(r.len(), CHECKS.len() * 2);
        let fails: Vec<_> = r.iter().filter(|c| !c.passes()).collect();
        assert!(fails.is_empty(), "{fails:#?}");
    }

    #[test]
    fn a_bad_pairing_is_reported() {
        let mut c = Config::default();
        if let Some(r) = c.roles.iter_mut().find(|r| r.name == "fg/muted") {
            r.light = 200;
            r.dark = 800;
        }
        let text = report_text(&c);
        assert!(text.contains("fg/muted on bg/default (Light)"), "{text}");
        assert!(text.contains("fg/muted on bg/default (Dark)"), "{text}");
        // Panel notes take no line breaks.
        assert!(!text.chars().any(char::is_control), "{text}");
    }
}
