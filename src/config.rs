//! What a design system is generated from. Kept as a user setting and, once generated, in the
//! document (on its first page) so everyone working on the file starts from the same choices.

use serde::{Deserialize, Serialize};

/// Brand and status colours, at most this many.
pub const MAX_COLORS: usize = 6;

/// A named base colour; its ramp becomes `<name>/50` … `<name>/950` in Primitives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedColor {
    pub name: String,
    pub hex: String,
}

/// Which ramp a theme role reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RampRef {
    Neutral,
    /// The `i`th of [`Config::colors`].
    Color(usize),
}

/// A semantic colour in Themes: a ramp step in Light and in Dark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub ramp: RampRef,
    pub light: u16,
    pub dark: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // Primitives
    pub colors: Vec<NamedColor>,
    /// 0…1: how much the neutral ramp leans toward the first colour's hue.
    pub neutral_tint: f64,
    /// Keep each base colour exact on its closest step.
    pub pin_base: bool,

    // Themes
    pub roles: Vec<Role>,

    // Sizes
    pub spacing_base: f64,
    pub radius_base: f64,
    /// Add a Compact mode to Sizes, scaling spacing by `compact_scale`.
    pub compact_mode: bool,
    pub compact_scale: f64,

    // Typography
    pub font_body: String,
    pub font_heading: String,
    pub font_mono: String,
    pub base_size: f64,
    pub type_scale: f64,
    pub line_height_body: f64,
    pub line_height_heading: f64,

    // Effects
    pub shadow_color: String,
    /// 0…1: the opacity of the strongest shadow layer.
    pub shadow_opacity: f64,
    pub elevations: u8,
    pub blurs: bool,
    pub focus_ring: bool,
}

fn role(name: &str, ramp: RampRef, light: u16, dark: u16) -> Role {
    Role { name: name.into(), ramp, light, dark }
}

/// The default theme roles. Colours 1–3 are danger, success and warning by default.
pub fn default_roles() -> Vec<Role> {
    use RampRef::{Color, Neutral};
    vec![
        role("bg/default", Neutral, 50, 950),
        role("bg/subtle", Neutral, 100, 900),
        role("bg/muted", Neutral, 200, 800),
        role("fg/default", Neutral, 900, 50),
        role("fg/muted", Neutral, 700, 400),
        role("fg/subtle", Neutral, 500, 500),
        role("border/default", Neutral, 200, 800),
        role("border/strong", Neutral, 500, 500),
        role("accent/default", Color(0), 600, 400),
        role("accent/hover", Color(0), 700, 300),
        role("accent/subtle", Color(0), 100, 900),
        role("accent/fg", Neutral, 50, 950),
        role("danger/default", Color(1), 600, 400),
        role("danger/subtle", Color(1), 100, 900),
        role("success/default", Color(2), 600, 400),
        role("success/subtle", Color(2), 100, 900),
        role("warning/default", Color(3), 600, 400),
        role("warning/subtle", Color(3), 100, 900),
        role("focus/ring", Color(0), 500, 400),
    ]
}

impl Default for Config {
    fn default() -> Self {
        let c = |name: &str, hex: &str| NamedColor { name: name.into(), hex: hex.into() };
        Config {
            colors: vec![c("brand", "#4F46E5"), c("red", "#DC2626"), c("green", "#16A34A"), c("amber", "#D97706")],
            neutral_tint: 0.3,
            pin_base: true,
            roles: default_roles(),
            spacing_base: 4.0,
            radius_base: 8.0,
            compact_mode: false,
            compact_scale: 0.75,
            font_body: "Inter".into(),
            font_heading: "Inter".into(),
            font_mono: "JetBrains Mono".into(),
            base_size: 16.0,
            type_scale: 1.25,
            line_height_body: 1.5,
            line_height_heading: 1.2,
            shadow_color: "#0F172A".into(),
            shadow_opacity: 0.12,
            elevations: 5,
            blurs: true,
            focus_ring: true,
        }
    }
}

/// A Primitives-safe name: lower case, words joined by `-`, no slashes.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

impl Config {
    /// Clamp numbers into range, drop invalid colours and duplicate names, and point roles that
    /// read a missing colour at the neutral ramp.
    pub fn normalized(mut self) -> Config {
        let mut seen = Vec::new();
        self.colors.retain_mut(|c| {
            c.name = slug(&c.name);
            let ok = !c.name.is_empty() && c.name != "neutral" && crate::color::Rgb::from_hex(&c.hex).is_some() && !seen.contains(&c.name);
            if ok {
                seen.push(c.name.clone());
            }
            ok
        });
        self.colors.truncate(MAX_COLORS);
        let n = self.colors.len();
        for r in &mut self.roles {
            if matches!(r.ramp, RampRef::Color(i) if i >= n) {
                r.ramp = RampRef::Neutral;
            }
            for s in [&mut r.light, &mut r.dark] {
                if crate::color::ramp::step_index(*s).is_none() {
                    *s = 500;
                }
            }
        }
        let fin = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.neutral_tint = fin(self.neutral_tint, 0.0, 1.0, 0.3);
        self.spacing_base = fin(self.spacing_base, 1.0, 32.0, 4.0);
        self.radius_base = fin(self.radius_base, 0.0, 64.0, 8.0);
        self.compact_scale = fin(self.compact_scale, 0.5, 1.0, 0.75);
        self.base_size = fin(self.base_size, 8.0, 32.0, 16.0);
        self.type_scale = fin(self.type_scale, 1.05, 1.8, 1.25);
        self.line_height_body = fin(self.line_height_body, 1.0, 2.5, 1.5);
        self.line_height_heading = fin(self.line_height_heading, 0.9, 2.0, 1.2);
        self.shadow_opacity = fin(self.shadow_opacity, 0.0, 1.0, 0.12);
        self.elevations = self.elevations.min(6);
        if crate::color::Rgb::from_hex(&self.shadow_color).is_none() {
            self.shadow_color = "#0F172A".into();
        }
        for f in [&mut self.font_body, &mut self.font_heading, &mut self.font_mono] {
            if f.trim().is_empty() {
                *f = "Inter".into();
            }
        }
        self
    }

    /// The display name of a ramp.
    pub fn ramp_name(&self, r: RampRef) -> String {
        match r {
            RampRef::Neutral => "neutral".into(),
            RampRef::Color(i) => self.colors.get(i).map_or_else(|| "neutral".into(), |c| c.name.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("  Brand Blue! "), "brand-blue");
        assert_eq!(slug("A/B"), "a-b");
        assert_eq!(slug("///"), "");
    }

    #[test]
    fn normalizing_fixes_bad_input() {
        let mut c = Config::default();
        c.colors.push(NamedColor { name: "Brand".into(), hex: "#000".into() }); // duplicate after slug
        c.colors.push(NamedColor { name: "bad".into(), hex: "nope".into() });
        c.colors.truncate(5);
        c.colors.remove(1); // red gone: danger roles now read colour 1 = green, still valid
        c.spacing_base = f64::NAN;
        c.roles.push(Role { name: "x".into(), ramp: RampRef::Color(9), light: 7, dark: 950 });
        let n = c.normalized();
        assert_eq!(n.colors.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["brand", "green", "amber"]);
        assert_eq!(n.spacing_base, 4.0);
        let x = n.roles.iter().find(|r| r.name == "x").cloned();
        assert_eq!(x.map(|r| (r.ramp, r.light)), Some((RampRef::Neutral, 500)));
    }

    #[test]
    fn config_round_trips_through_json() {
        let c = Config::default();
        let v = figcraft_plugin_sdk::serde_json::to_value(&c).unwrap_or_default();
        let back: Config = figcraft_plugin_sdk::serde_json::from_value(v).unwrap_or_default();
        assert_eq!(back, c);
        // Missing keys take defaults (an older saved config keeps working).
        let partial: Config = figcraft_plugin_sdk::serde_json::from_str(r#"{"base_size": 18}"#).unwrap_or_default();
        assert_eq!((partial.base_size, partial.spacing_base), (18.0, 4.0));
    }
}
