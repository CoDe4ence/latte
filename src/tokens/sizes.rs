//! Sizes: spacing, corner radii and border widths as number variables, with an optional Compact
//! mode that tightens spacing.

use super::{CollectionPlan, Kind, SIZES, Val, VarPlan, round2};
use crate::config::Config;

pub const DEFAULT: &str = "Default";
pub const COMPACT: &str = "Compact";

/// Spacing steps as multiples of the base unit; the name is the multiple.
pub const SPACING: [f64; 15] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0];

/// Radii as multiples of the base radius (`full` is a pill).
pub const RADII: [(&str, f64); 7] = [("none", 0.0), ("xs", 0.25), ("sm", 0.5), ("md", 1.0), ("lg", 1.5), ("xl", 2.0), ("2xl", 3.0)];

pub const BORDERS: [(&str, f64); 4] = [("none", 0.0), ("thin", 1.0), ("medium", 2.0), ("thick", 4.0)];

/// `0.5` → `0-5`, `2` → `2`: a step name without a dot.
fn step_name(m: f64) -> String {
    let s = format!("{}", round2(m));
    s.replace('.', "-")
}

pub fn collection(config: &Config) -> CollectionPlan {
    let modes: Vec<String> = if config.compact_mode { vec![DEFAULT.into(), COMPACT.into()] } else { vec![DEFAULT.into()] };
    let num = |name: String, default: f64, compact: f64, scopes: Vec<&'static str>, description: String| {
        let mut values = vec![(DEFAULT.to_string(), Val::Number(round2(default)))];
        if config.compact_mode {
            values.push((COMPACT.into(), Val::Number(round2(compact))));
        }
        VarPlan { name, kind: Kind::Number, values, scopes: Some(scopes), description }
    };
    let mut vars = Vec::new();
    for m in SPACING {
        let px = config.spacing_base * m;
        // Compact spacing stays on whole pixels.
        let compact = (px * config.compact_scale).round();
        vars.push(num(format!("spacing/{}", step_name(m)), px, compact, vec!["GAP", "WIDTH_HEIGHT"], format!("{} × base", round2(m))));
    }
    for (name, m) in RADII {
        let px = (config.radius_base * m).round();
        vars.push(num(format!("radius/{name}"), px, px, vec!["CORNER_RADIUS"], String::new()));
    }
    vars.push(num("radius/full".into(), 9999.0, 9999.0, vec!["CORNER_RADIUS"], "Pill".into()));
    for (name, px) in BORDERS {
        vars.push(num(format!("border/{name}"), px, px, vec!["STROKE_FLOAT"], String::new()));
    }
    CollectionPlan { name: SIZES, modes, vars }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_follows_the_base() {
        let c = Config { spacing_base: 8.0, compact_mode: true, ..Config::default() };
        let p = collection(&c);
        assert_eq!(p.modes, [DEFAULT, COMPACT]);
        let v = p.vars.iter().find(|v| v.name == "spacing/0-5").map(|v| v.values.clone());
        assert_eq!(v, Some(vec![(DEFAULT.into(), Val::Number(4.0)), (COMPACT.into(), Val::Number(3.0))]));
        assert!(p.vars.iter().any(|v| v.name == "radius/full"));
    }
}
