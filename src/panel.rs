//! Latte's panel: one FigCraft panel whose `section` menu switches between views. Every field
//! change is saved to the config at once and the view is drawn again with fresh previews, so the
//! panel itself keeps no state: the config (a setting) and the event's values are all there is.

use figcraft_plugin_sdk::{Value, json};

use crate::color::ramp::STEPS;
use crate::color::{png, Rgb};
use crate::config::{Config, MAX_COLORS, NamedColor, RampRef, default_roles};
use crate::tokens::{self, StyleType, primitives, sizes, themes, typography};

pub const SECTIONS: [&str; 6] = ["Overview", "Colours", "Themes", "Sizes", "Typography", "Effects"];

/// Type scale presets: (ratio, label).
pub const SCALES: [(f64, &str); 6] =
    [(1.125, "1.125 Major second"), (1.2, "1.2 Minor third"), (1.25, "1.25 Major third"), (1.333, "1.333 Perfect fourth"), (1.414, "1.414 Augmented fourth"), (1.5, "1.5 Perfect fifth")];

fn scale_label(ratio: f64) -> &'static str {
    SCALES.iter().min_by(|a, b| (a.0 - ratio).abs().total_cmp(&(b.0 - ratio).abs())).map_or(SCALES[2].1, |s| s.1)
}

fn section_of(values: &Value) -> &str {
    values["section"].as_str().filter(|s| SECTIONS.contains(s)).unwrap_or("Overview")
}

fn note(label: impl Into<String>) -> Value {
    // FigCraft refuses control characters (line breaks included) in labels.
    let mut l: String = label.into().chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if l.chars().count() > 1000 {
        l = l.chars().take(997).collect::<String>() + "…";
    }
    json!({"type": "note", "label": l})
}

fn step_options() -> Vec<String> {
    STEPS.iter().map(u16::to_string).collect()
}

fn ramp_options(c: &Config) -> Vec<String> {
    std::iter::once("neutral".to_string()).chain(c.colors.iter().map(|c| c.name.clone())).collect()
}

/// A PNG of every ramp, one row each (neutral first), as base64.
fn ramps_image(c: &Config) -> String {
    let ramps = primitives::ramps(c);
    let (cell_w, cell_h) = (24u32, 18u32);
    let (w, h) = (cell_w * STEPS.len() as u32, cell_h * ramps.len() as u32);
    let mut px = vec![0u8; (w * h * 3) as usize];
    for (row, (_, r)) in ramps.iter().enumerate() {
        for (col, s) in r.iter().enumerate() {
            let rgb = s.color.bytes();
            for y in 0..cell_h {
                for x in 0..cell_w {
                    let (gx, gy) = (col as u32 * cell_w + x, row as u32 * cell_h + y);
                    let i = ((gy * w + gx) * 3) as usize;
                    px[i..i + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    png::base64(&png::encode(w, h, &px))
}

/// The controls of `section`.
fn controls(c: &Config, section: &str, values: &Value, message: Option<&str>) -> Vec<Value> {
    let mut v = vec![json!({"type": "select", "name": "section", "label": "Section", "options": SECTIONS, "value": section})];
    match section {
        "Colours" => {
            for i in 0..MAX_COLORS {
                let col = c.colors.get(i);
                v.push(json!({"type": "text", "name": format!("name{i}"), "label": format!("Colour {} name", i + 1), "value": col.map_or("", |c| c.name.as_str())}));
                v.push(json!({"type": "color", "name": format!("hex{i}"), "label": format!("Colour {}", i + 1), "value": col.map_or("#888888", |c| c.hex.as_str())}));
            }
            v.push(note("Clear a name to drop that colour. The first colour is the accent; by default 2–4 are danger, success and warning."));
            v.push(json!({"type": "slider", "name": "tint", "label": "Neutral tint", "min": 0, "max": 100, "value": (c.neutral_tint * 100.0).round()}));
            v.push(json!({"type": "boolean", "name": "pin", "label": "Keep each base colour exact on its closest step", "value": c.pin_base}));
            v.push(json!({"type": "image", "label": "Ramp preview", "data": ramps_image(c)}));
            v.push(note(format!("Rows: {}. Steps 50 → 950.", ramp_options(c).join(", "))));
        }
        "Themes" => {
            let names: Vec<&str> = c.roles.iter().map(|r| r.name.as_str()).collect();
            let picked = values["role"].as_str().filter(|r| names.contains(r)).or(names.first().copied()).unwrap_or_default();
            v.push(note(themes::report_text(c)));
            v.push(json!({"type": "list", "name": "role", "label": "Role", "options": names, "value": picked}));
            if let Some(r) = c.roles.iter().find(|r| r.name == picked) {
                v.push(json!({"type": "select", "name": "ramp", "label": "Ramp", "options": ramp_options(c), "value": c.ramp_name(r.ramp)}));
                v.push(json!({"type": "select", "name": "light", "label": "Light step", "options": step_options(), "value": r.light.to_string()}));
                v.push(json!({"type": "select", "name": "dark", "label": "Dark step", "options": step_options(), "value": r.dark.to_string()}));
                let ramps = primitives::ramps(c);
                let hex = |dark| themes::resolve(c, &ramps, picked, dark).map_or("?".into(), Rgb::to_hex);
                v.push(note(format!("{picked}: {} in Light, {} in Dark.", hex(false), hex(true))));
            }
        }
        "Sizes" => {
            v.push(json!({"type": "number", "name": "spacing", "label": "Spacing base unit (px)", "min": 1, "max": 32, "value": c.spacing_base}));
            v.push(json!({"type": "number", "name": "radius", "label": "Base corner radius (px)", "min": 0, "max": 64, "value": c.radius_base}));
            v.push(json!({"type": "boolean", "name": "compact", "label": "Add a Compact mode", "value": c.compact_mode}));
            v.push(json!({"type": "slider", "name": "compactScale", "label": "Compact spacing (%)", "min": 50, "max": 100, "value": (c.compact_scale * 100.0).round()}));
            let plan = sizes::collection(c);
            let list = |prefix: &str| {
                plan.vars
                    .iter()
                    .filter(|v| v.name.starts_with(prefix))
                    .filter_map(|v| match v.values.first() {
                        Some((_, tokens::Val::Number(n))) => Some(format!("{} {}", v.name.trim_start_matches(prefix), n)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            v.push(note(format!("Spacing: {}", list("spacing/"))));
            v.push(note(format!("Radius: {}", list("radius/"))));
        }
        "Typography" => {
            v.push(json!({"type": "text", "name": "fontBody", "label": "Body font", "value": c.font_body}));
            v.push(json!({"type": "text", "name": "fontHeading", "label": "Heading font", "value": c.font_heading}));
            v.push(json!({"type": "text", "name": "fontMono", "label": "Monospace font", "value": c.font_mono}));
            v.push(json!({"type": "number", "name": "baseSize", "label": "Base size (px)", "min": 8, "max": 32, "value": c.base_size}));
            v.push(json!({"type": "select", "name": "scale", "label": "Type scale", "options": SCALES.iter().map(|s| s.1).collect::<Vec<_>>(), "value": scale_label(c.type_scale)}));
            v.push(json!({"type": "slider", "name": "lhBody", "label": "Body line height (%)", "min": 100, "max": 250, "value": (c.line_height_body * 100.0).round()}));
            v.push(json!({"type": "slider", "name": "lhHeading", "label": "Heading line height (%)", "min": 90, "max": 200, "value": (c.line_height_heading * 100.0).round()}));
            let sizes: Vec<String> =
                typography::SIZES.iter().map(|(n, p)| format!("{n} {}/{}", typography::size(c, *p), typography::line_height(c, *p))).collect();
            v.push(note(format!("Size/line height: {}", sizes.join(" · "))));
        }
        "Effects" => {
            v.push(json!({"type": "color", "name": "shadowColor", "label": "Shadow colour", "value": c.shadow_color}));
            v.push(json!({"type": "slider", "name": "shadowOpacity", "label": "Shadow opacity (%)", "min": 0, "max": 60, "value": (c.shadow_opacity * 100.0).round()}));
            v.push(json!({"type": "number", "name": "elevations", "label": "Elevation levels", "min": 0, "max": 6, "value": c.elevations}));
            v.push(json!({"type": "boolean", "name": "focusRing", "label": "Focus ring", "value": c.focus_ring}));
            v.push(json!({"type": "boolean", "name": "blurs", "label": "Background blurs", "value": c.blurs}));
            let names: Vec<String> = tokens::effects::styles(c).into_iter().map(|s| s.name).collect();
            v.push(note(format!("Effect styles: {}", if names.is_empty() { "none".into() } else { names.join(", ") })));
        }
        _ => {
            let plan = tokens::plan(c);
            let per: Vec<String> = plan.collections.iter().map(|k| format!("{} {}", k.name, k.vars.len())).collect();
            v.push(note(format!(
                "Latte will keep {} variables ({}), {} text styles and {} effect styles in this file. Generate creates what's missing and updates what changed; it never deletes.",
                plan.variable_count(),
                per.join(", "),
                plan.style_count(StyleType::Text),
                plan.style_count(StyleType::Effect)
            )));
            v.push(note(themes::report_text(c)));
        }
    }
    if let Some(m) = message {
        v.push(note(m));
    }
    v.push(json!({"type": "button", "name": "generate", "label": "Generate"}));
    if section != "Overview" {
        v.push(json!({"type": "button", "name": "reset", "label": "Reset section"}));
    }
    v.push(json!({"type": "button", "name": "close", "label": "Close"}));
    v
}

/// The panel spec for `section`.
pub fn spec(c: &Config, section: &str, values: &Value, message: Option<&str>) -> Value {
    json!({"title": "Latte", "controls": controls(c, section, values, message)})
}

/// `values` from the panel applied to the config, for the section showing. `control` is what
/// changed (picking another role only changes the view).
pub fn merge(mut c: Config, control: &str, values: &Value) -> Config {
    let num = |k: &str| values[k].as_f64().filter(|v| v.is_finite());
    let text = |k: &str| values[k].as_str().map(str::to_string);
    let flag = |k: &str| values[k].as_bool();
    match section_of(values) {
        "Colours" => {
            let mut colors = Vec::new();
            for i in 0..MAX_COLORS {
                let (name, hex) = (text(&format!("name{i}")).unwrap_or_default(), text(&format!("hex{i}")).unwrap_or_default());
                if !name.trim().is_empty() {
                    colors.push(NamedColor { name, hex: Rgb::from_hex(&hex).map_or(hex, Rgb::to_hex) });
                }
            }
            // A colour removed from the middle shifts the ones after it; roles follow their colour.
            let old: Vec<String> = c.colors.iter().map(|x| crate::config::slug(&x.name)).collect();
            let new: Vec<String> = colors.iter().map(|x| crate::config::slug(&x.name)).collect();
            for r in &mut c.roles {
                if let RampRef::Color(i) = r.ramp {
                    // Renaming a colour in place keeps its slot; a colour that's gone leaves its
                    // roles on the neutral ramp.
                    let renamed = old.get(i).is_some_and(|o| !new.contains(o)) && new.get(i).is_some_and(|n| !old.contains(n));
                    r.ramp = match old.get(i).and_then(|name| new.iter().position(|x| x == name)) {
                        Some(n) => RampRef::Color(n),
                        None if renamed => RampRef::Color(i),
                        None => RampRef::Neutral,
                    };
                }
            }
            c.colors = colors;
            c.neutral_tint = num("tint").map_or(c.neutral_tint, |t| t / 100.0);
            c.pin_base = flag("pin").unwrap_or(c.pin_base);
        }
        "Themes" if control != "role" => {
            let ramp = text("ramp").map(|r| if r == "neutral" { RampRef::Neutral } else { c.colors.iter().position(|x| x.name == r).map_or(RampRef::Neutral, RampRef::Color) });
            let step = |k: &str| text(k).and_then(|s| s.parse::<u16>().ok());
            if let Some(role) = text("role")
                && let Some(r) = c.roles.iter_mut().find(|r| r.name == role)
            {
                r.ramp = ramp.unwrap_or(r.ramp);
                r.light = step("light").unwrap_or(r.light);
                r.dark = step("dark").unwrap_or(r.dark);
            }
        }
        "Sizes" => {
            c.spacing_base = num("spacing").unwrap_or(c.spacing_base);
            c.radius_base = num("radius").unwrap_or(c.radius_base);
            c.compact_mode = flag("compact").unwrap_or(c.compact_mode);
            c.compact_scale = num("compactScale").map_or(c.compact_scale, |v| v / 100.0);
        }
        "Typography" => {
            c.font_body = text("fontBody").unwrap_or(c.font_body);
            c.font_heading = text("fontHeading").unwrap_or(c.font_heading);
            c.font_mono = text("fontMono").unwrap_or(c.font_mono);
            c.base_size = num("baseSize").unwrap_or(c.base_size);
            c.type_scale = text("scale").and_then(|l| SCALES.iter().find(|s| s.1 == l)).map_or(c.type_scale, |s| s.0);
            c.line_height_body = num("lhBody").map_or(c.line_height_body, |v| v / 100.0);
            c.line_height_heading = num("lhHeading").map_or(c.line_height_heading, |v| v / 100.0);
        }
        "Effects" => {
            c.shadow_color = text("shadowColor").and_then(|h| Rgb::from_hex(&h)).map_or(c.shadow_color, Rgb::to_hex);
            c.shadow_opacity = num("shadowOpacity").map_or(c.shadow_opacity, |v| v / 100.0);
            c.elevations = num("elevations").map_or(c.elevations, |v| v.clamp(0.0, 6.0) as u8);
            c.focus_ring = flag("focusRing").unwrap_or(c.focus_ring);
            c.blurs = flag("blurs").unwrap_or(c.blurs);
        }
        _ => {}
    }
    c.normalized()
}

/// The config with `section`'s settings back to their defaults.
pub fn reset(mut c: Config, section: &str) -> Config {
    let d = Config::default();
    match section {
        "Colours" => {
            c.colors = d.colors;
            c.neutral_tint = d.neutral_tint;
            c.pin_base = d.pin_base;
            c.roles = default_roles();
        }
        "Themes" => c.roles = default_roles(),
        "Sizes" => {
            c.spacing_base = d.spacing_base;
            c.radius_base = d.radius_base;
            c.compact_mode = d.compact_mode;
            c.compact_scale = d.compact_scale;
        }
        "Typography" => {
            c.font_body = d.font_body;
            c.font_heading = d.font_heading;
            c.font_mono = d.font_mono;
            c.base_size = d.base_size;
            c.type_scale = d.type_scale;
            c.line_height_body = d.line_height_body;
            c.line_height_heading = d.line_height_heading;
        }
        "Effects" => {
            c.shadow_color = d.shadow_color;
            c.shadow_opacity = d.shadow_opacity;
            c.elevations = d.elevations;
            c.focus_ring = d.focus_ring;
            c.blurs = d.blurs;
        }
        _ => {}
    }
    c.normalized()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel's values, as FigCraft would send them back for `spec`.
    fn values_of(spec: &Value) -> Value {
        let mut out = json!({});
        for c in spec["controls"].as_array().cloned().unwrap_or_default() {
            if let (Some(n), Some(v)) = (c["name"].as_str(), c.get("value")) {
                out[n] = v.clone();
            }
        }
        out
    }

    #[test]
    fn every_section_fits_a_panel() {
        let c = Config::default();
        for s in SECTIONS {
            let spec = spec(&c, s, &Value::Null, Some("Done."));
            let controls = spec["controls"].as_array().map_or(0, Vec::len);
            assert!(controls <= 64, "{s}: {controls}");
            assert!(figcraft_plugin_sdk::serde_json::to_string(&spec).map_or(0, |t| t.len()) < 1_000_000, "{s}");
            let labels = spec["controls"].as_array().into_iter().flatten().filter_map(|c| c["label"].as_str());
            assert!(labels.clone().all(|l| !l.chars().any(char::is_control)), "{s}: a label has control characters");
            let names: Vec<&str> = spec["controls"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str()).collect();
            let mut unique = names.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), names.len(), "{s}");
        }
    }

    #[test]
    fn unchanged_values_give_the_same_config() {
        let c = Config::default();
        for s in SECTIONS {
            let values = values_of(&spec(&c, s, &Value::Null, None));
            assert_eq!(merge(c.clone(), "x", &values), c.clone().normalized(), "{s}");
        }
    }

    #[test]
    fn editing_a_role_and_a_colour() {
        let c = Config::default();
        let mut values = values_of(&spec(&c, "Themes", &json!({"role": "fg/muted"}), None));
        values["light"] = json!("800");
        let c = merge(c, "light", &values);
        assert_eq!(c.roles.iter().find(|r| r.name == "fg/muted").map(|r| r.light), Some(800));
        // Dropping colour 2 (red) keeps the warning roles on amber.
        let mut values = values_of(&spec(&c, "Colours", &Value::Null, None));
        values["name1"] = json!("");
        let c = merge(c, "name1", &values);
        assert_eq!(c.colors.len(), 3);
        let warning = c.roles.iter().find(|r| r.name == "warning/default").map(|r| c.ramp_name(r.ramp));
        assert_eq!(warning.as_deref(), Some("amber"));
        let danger = c.roles.iter().find(|r| r.name == "danger/default").map(|r| c.ramp_name(r.ramp));
        assert_eq!(danger.as_deref(), Some("neutral"));
        // Renaming a colour keeps its roles.
        let mut values = values_of(&spec(&c, "Colours", &Value::Null, None));
        values["name0"] = json!("Indigo");
        let c = merge(c, "name0", &values);
        let accent = c.roles.iter().find(|r| r.name == "accent/default").map(|r| c.ramp_name(r.ramp));
        assert_eq!(accent.as_deref(), Some("indigo"));
    }
}
