//! Effect styles: elevation shadows, a focus ring and background blurs.

use figcraft_plugin_sdk::{Value, json};

use super::{StylePlan, StyleType};
use crate::color::Rgb;
use crate::config::Config;

/// Per elevation level: (key offset y, key blur, ambient offset y, ambient blur, spread).
const LEVELS: [(f64, f64, f64, f64, f64); 6] =
    [(1.0, 2.0, 1.0, 3.0, 0.0), (2.0, 4.0, 1.0, 6.0, -1.0), (4.0, 8.0, 2.0, 12.0, -2.0), (8.0, 16.0, 4.0, 24.0, -4.0), (12.0, 24.0, 6.0, 36.0, -6.0), (20.0, 40.0, 8.0, 56.0, -8.0)];

const BLURS: [(&str, f64); 3] = [("sm", 4.0), ("md", 12.0), ("lg", 24.0)];

/// `#RRGGBB` with alpha `a` as `#RRGGBBAA`.
fn with_alpha(hex: &str, a: f64) -> String {
    let c = Rgb::from_hex(hex).unwrap_or(Rgb::BLACK).to_hex();
    format!("{c}{:02X}", (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn shadow(color: String, y: f64, blur: f64, spread: f64) -> Value {
    json!({"type": "dropShadow", "color": color, "offset": [0.0, y], "radius": blur, "spread": spread, "visible": true})
}

pub fn styles(config: &Config) -> Vec<StylePlan> {
    let mut out = Vec::new();
    let o = config.shadow_opacity;
    for (i, &(ky, kb, ay, ab, spread)) in LEVELS.iter().take(usize::from(config.elevations)).enumerate() {
        out.push(StylePlan {
            kind: StyleType::Effect,
            name: format!("Elevation/{}", i + 1),
            // Bottom first: a soft ambient shadow, then the sharper key shadow.
            value: json!([shadow(with_alpha(&config.shadow_color, o * 0.6), ay, ab, spread), shadow(with_alpha(&config.shadow_color, o), ky, kb, spread)]),
            binds: vec![],
            description: format!("Shadow level {}", i + 1),
        });
    }
    if config.focus_ring {
        let accent = config.colors.first().map_or("#4F46E5", |c| c.hex.as_str());
        out.push(StylePlan {
            kind: StyleType::Effect,
            name: "Focus/Ring".into(),
            value: json!([shadow(with_alpha(accent, 0.45), 0.0, 0.0, 3.0)]),
            binds: vec![],
            description: "A 3px ring in the first colour".into(),
        });
    }
    if config.blurs {
        for (name, r) in BLURS {
            out.push(StylePlan {
                kind: StyleType::Effect,
                name: format!("Blur/{name}"),
                value: json!([{"type": "backgroundBlur", "radius": r, "visible": true}]),
                binds: vec![],
                description: format!("Background blur {r}px"),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_follow_the_config() {
        let mut c = Config::default();
        assert_eq!(styles(&c).len(), 5 + 1 + 3);
        c.elevations = 2;
        c.blurs = false;
        c.focus_ring = false;
        let s = styles(&c);
        assert_eq!(s.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Elevation/1", "Elevation/2"]);
        assert_eq!(with_alpha("#0F172A", 0.12), "#0F172A1F");
    }
}
