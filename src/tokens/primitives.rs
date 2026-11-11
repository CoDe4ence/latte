//! Primitives: a tonal ramp per base colour, a neutral ramp, white and black.

use super::{CollectionPlan, Kind, PRIMITIVES, Val, VarPlan, single};
use crate::color::ramp::{self, Swatch};
use crate::color::Rgb;
use crate::config::Config;

pub const MODE: &str = "Value";

/// The ramps, neutral first, then each colour: (name, swatches).
pub fn ramps(config: &Config) -> Vec<(String, Vec<Swatch>)> {
    let hue = config.colors.first().and_then(|c| Rgb::from_hex(&c.hex)).map_or(270.0, |c| c.to_oklch().h);
    let mut out = vec![("neutral".to_string(), ramp::neutral(hue, config.neutral_tint))];
    for c in &config.colors {
        if let Some(base) = Rgb::from_hex(&c.hex) {
            out.push((c.name.clone(), ramp::ramp(base, config.pin_base)));
        }
    }
    out
}

/// The colour at `step` of ramp `name`.
pub fn lookup(ramps: &[(String, Vec<Swatch>)], name: &str, step: u16) -> Option<Rgb> {
    ramps.iter().find(|(n, _)| n == name).and_then(|(_, r)| r.iter().find(|s| s.step == step)).map(|s| s.color)
}

pub fn var_name(ramp: &str, step: u16) -> String {
    format!("{ramp}/{step}")
}

const COLOR_SCOPES: [&str; 5] = ["FRAME_FILL", "SHAPE_FILL", "TEXT_FILL", "STROKE_COLOR", "EFFECT_COLOR"];

pub fn collection(ramps: &[(String, Vec<Swatch>)]) -> CollectionPlan {
    let color = |name: String, c: Rgb| VarPlan {
        name,
        kind: Kind::Color,
        values: single(MODE, Val::Color(c.to_hex())),
        // Primitives feed Themes; designers pick semantic colours, so keep raw ones out of pickers.
        scopes: Some(vec![]),
        description: String::new(),
    };
    let mut vars: Vec<VarPlan> = ramps.iter().flat_map(|(name, r)| r.iter().map(move |s| (name, s))).map(|(n, s)| color(var_name(n, s.step), s.color)).collect();
    vars.push(color("base/white".into(), Rgb::WHITE));
    vars.push(color("base/black".into(), Rgb::BLACK));
    CollectionPlan { name: PRIMITIVES, modes: vec![MODE.into()], vars }
}

/// Scopes for semantic colours.
pub fn color_scopes() -> Vec<&'static str> {
    COLOR_SCOPES.to_vec()
}
