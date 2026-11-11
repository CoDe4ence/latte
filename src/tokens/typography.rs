//! Typography: font families, a modular size scale, line heights and weights as variables, and
//! text styles bound to them.

use figcraft_plugin_sdk::json;

use super::{CollectionPlan, Kind, StylePlan, StyleType, TYPOGRAPHY, Val, VarPlan, VarRef, single};
use crate::config::Config;

pub const MODE: &str = "Base";

/// Size steps and their power of the scale ratio from the base size.
pub const SIZES: [(&str, i32); 9] = [("xs", -2), ("sm", -1), ("base", 0), ("lg", 1), ("xl", 2), ("2xl", 3), ("3xl", 4), ("4xl", 5), ("5xl", 6)];

pub const WEIGHTS: [(&str, f64); 4] = [("regular", 400.0), ("medium", 500.0), ("semibold", 600.0), ("bold", 700.0)];

/// The font size of step `power`, on whole pixels.
pub fn size(config: &Config, power: i32) -> f64 {
    (config.base_size * config.type_scale.powi(power)).round()
}

/// Headings (xl and up) use the tighter line height; line heights land on even pixels.
pub fn line_height(config: &Config, power: i32) -> f64 {
    let ratio = if power >= 2 { config.line_height_heading } else { config.line_height_body };
    let lh = size(config, power) * ratio;
    (lh / 2.0).round() * 2.0
}

pub fn collection(config: &Config) -> CollectionPlan {
    let v = |name: String, kind, val, scopes: Vec<&'static str>| VarPlan { name, kind, values: single(MODE, val), scopes: Some(scopes), description: String::new() };
    let mut vars = vec![
        v("family/body".into(), Kind::String, Val::Str(config.font_body.clone()), vec!["FONT_FAMILY"]),
        v("family/heading".into(), Kind::String, Val::Str(config.font_heading.clone()), vec!["FONT_FAMILY"]),
        v("family/mono".into(), Kind::String, Val::Str(config.font_mono.clone()), vec!["FONT_FAMILY"]),
    ];
    for (name, p) in SIZES {
        vars.push(v(format!("size/{name}"), Kind::Number, Val::Number(size(config, p)), vec!["FONT_SIZE"]));
    }
    for (name, p) in SIZES {
        vars.push(v(format!("line-height/{name}"), Kind::Number, Val::Number(line_height(config, p)), vec!["LINE_HEIGHT"]));
    }
    for (name, w) in WEIGHTS {
        vars.push(v(format!("weight/{name}"), Kind::Number, Val::Number(w), vec!["FONT_WEIGHT"]));
    }
    CollectionPlan { name: TYPOGRAPHY, modes: vec![MODE.into()], vars }
}

/// (style name, family, size step, font style, letter spacing %)
const STYLES: [(&str, &str, &str, &str, f64); 13] = [
    ("Display/Large", "heading", "5xl", "Bold", -2.0),
    ("Display/Small", "heading", "4xl", "Bold", -2.0),
    ("Heading/H1", "heading", "3xl", "Bold", -1.0),
    ("Heading/H2", "heading", "2xl", "Semi Bold", -1.0),
    ("Heading/H3", "heading", "xl", "Semi Bold", 0.0),
    ("Heading/H4", "heading", "lg", "Semi Bold", 0.0),
    ("Body/Large", "body", "lg", "Regular", 0.0),
    ("Body/Default", "body", "base", "Regular", 0.0),
    ("Body/Small", "body", "sm", "Regular", 0.0),
    ("Label/Default", "body", "sm", "Medium", 0.0),
    ("Label/Small", "body", "xs", "Medium", 1.0),
    ("Caption", "body", "xs", "Regular", 0.0),
    ("Code", "mono", "sm", "Regular", 0.0),
];

pub fn styles(config: &Config) -> Vec<StylePlan> {
    let var = |name: String| VarRef { collection: TYPOGRAPHY, name };
    STYLES
        .iter()
        .map(|&(name, family, step, font_style, tracking)| {
            let power = SIZES.iter().find(|(n, _)| *n == step).map_or(0, |(_, p)| *p);
            let font = match family {
                "heading" => &config.font_heading,
                "mono" => &config.font_mono,
                _ => &config.font_body,
            };
            StylePlan {
                kind: StyleType::Text,
                name: name.to_string(),
                value: json!({
                    "fontFamily": font,
                    "fontStyle": font_style,
                    "fontSize": size(config, power),
                    "lineHeight": line_height(config, power),
                    "letterSpacing": format!("{tracking}%"),
                }),
                binds: vec![
                    ("fontFamily", var(format!("family/{family}"))),
                    ("fontSize", var(format!("size/{step}"))),
                    ("lineHeight", var(format!("line-height/{step}"))),
                ],
                description: format!("{font} {font_style}, size/{step}"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale() {
        let c = Config::default();
        assert_eq!(size(&c, 0), 16.0);
        assert_eq!(size(&c, 1), 20.0);
        assert_eq!(size(&c, -1), 13.0);
        assert_eq!(line_height(&c, 0), 24.0);
        // 3xl: 16 × 1.25⁴ = 39.06 → 39; × 1.2 = 46.8 → 46.
        assert_eq!(size(&c, 4), 39.0);
        assert_eq!(line_height(&c, 4), 46.0);
        let sizes: Vec<f64> = SIZES.iter().map(|(_, p)| size(&c, *p)).collect();
        assert!(sizes.windows(2).all(|w| w[0] < w[1]));
    }
}
