//! The Latte board: a widget on the canvas that shows the design system as it will be generated
//! (swatches, Light and Dark previews, a type specimen, spacing and elevation), with tabs, Generate
//! and Edit settings buttons, and editable colour names and hex values. Its state holds the
//! working config, so edits show at once and travel with the file; Generate writes them to the
//! document's variables and styles.
//!
//! Widget layers can't take fonts, effects or variables, so the board previews with plain colours
//! in the default typeface; the generated styles are the real thing.

use figcraft_plugin_sdk::{Value, json};

use crate::color::Rgb;
use crate::color::ramp::STEPS;
use crate::config::Config;
use crate::tokens::{self, StyleType, primitives, sizes, themes, typography};

/// Tabs: (state key, label).
pub const TABS: [(&str, &str); 5] = [("colours", "Colours"), ("themes", "Themes"), ("type", "Type"), ("spacing", "Spacing"), ("effects", "Effects")];

// The board's own colours: a latte.
const BG: &str = "#FBF8F4";
const CARD: &str = "#FFFFFF";
const INK: &str = "#2B2118";
const MUTED: &str = "#8C7B6B";
const LINE: &str = "#EADFD3";
const SOFT: &str = "#F3ECE4";
const SOFT_HOVER: &str = "#EBE1D6";
const COFFEE: &str = "#6F4E37";
const COFFEE_HOVER: &str = "#5A3E2B";
const CREAM: &str = "#F5E6D3";
const FOAM: &str = "#FFFDF9";

const WIDTH: f64 = 840.0;
const PAD: f64 = 28.0;
/// The content card's inner width.
const INNER: f64 = WIDTH - 2.0 * PAD - 48.0;

fn text(t: impl Into<String>, size: f64, color: &str, bold: bool) -> Value {
    let t: String = t.into();
    let mut v = json!({"type": "text", "text": if t.is_empty() { " ".to_string() } else { t }, "size": size, "color": color});
    if bold {
        v["bold"] = json!(true);
    }
    v
}

fn rect(w: f64, h: f64, fill: &str, radius: f64) -> Value {
    json!({"type": "rectangle", "width": w, "height": h, "fill": fill, "radius": radius})
}

fn frame(layout: &str, gap: u32, children: Vec<Value>) -> Value {
    json!({"type": "frame", "layout": layout, "gap": gap, "children": children})
}

fn row(gap: u32, children: Vec<Value>) -> Value {
    let mut f = frame("horizontal", gap, children);
    f["align"] = json!("center");
    f
}

/// A vertical stack, left-aligned (FigCraft centres across the axis by default).
fn col(gap: u32, children: Vec<Value>) -> Value {
    let mut f = frame("vertical", gap, children);
    f["align"] = json!("min");
    f
}

/// `v` with `extra`'s keys set.
fn with(mut v: Value, extra: Value) -> Value {
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    v
}

fn button(label: &str, target: &str, primary: bool) -> Value {
    let (fill, hover, ink) = if primary { (COFFEE, COFFEE_HOVER, FOAM) } else { (FOAM, SOFT, INK) };
    with(
        row(6, vec![text(label, 13.0, ink, true)]),
        json!({"name": label, "target": target, "padding": 10, "radius": 10, "fill": fill, "stroke": if primary { COFFEE } else { LINE }, "hover": {"fill": hover}}),
    )
}

/// `#RRGGBB` with alpha `a`.
fn alpha(hex: &str, a: f64) -> String {
    format!("{}{:02X}", Rgb::from_hex(hex).unwrap_or(Rgb::BLACK).to_hex(), (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn header(c: &Config) -> Value {
    let plan = tokens::plan(c);
    let stats = format!(
        "Design system · {} variables · {} styles",
        plan.variable_count(),
        plan.style_count(StyleType::Text) + plan.style_count(StyleType::Effect)
    );
    // A cup from above: coffee rim, latte foam.
    let logo = with(
        row(0, vec![with(json!({"type": "ellipse", "width": 24, "height": 24, "fill": CREAM}), json!({"stroke": FOAM, "strokeWeight": 2}))]),
        json!({"width": 44, "height": 44, "radius": 12, "fill": COFFEE, "justify": "center"}),
    );
    let title = row(12, vec![logo, col(2, vec![text("Latte", 22.0, INK, true), text(stats, 12.0, MUTED, false)])]);
    let buttons = with(row(8, vec![button("Edit settings", "edit", false), button("Generate", "generate", true)]), json!({"justify": "max"}));
    let w = WIDTH - 2.0 * PAD;
    row(0, vec![with(title, json!({"width": w - 260.0})), with(buttons, json!({"width": 260.0}))])
}

fn tabs(active: &str) -> Value {
    let items = TABS
        .iter()
        .map(|(key, label)| {
            let on = *key == active;
            let t = with(
                row(0, vec![text(*label, 13.0, if on { INK } else { MUTED }, on)]),
                json!({"name": label, "target": format!("tab.{key}"), "width": 104, "padding": 8, "radius": 9, "justify": "center"}),
            );
            if on { with(t, json!({"fill": CARD, "stroke": LINE})) } else { with(t, json!({"hover": {"fill": SOFT_HOVER}})) }
        })
        .collect();
    with(row(4, items), json!({"padding": 4, "radius": 12, "fill": SOFT}))
}

fn section_title(title: &str, caption: &str) -> Value {
    row(10, vec![text(title, 15.0, INK, true), text(caption, 12.0, MUTED, false)])
}

fn colours(c: &Config) -> Vec<Value> {
    let ramps = primitives::ramps(c);
    let (name_w, sw, gap) = (120.0, 52.0, 4);
    let mut steps = vec![json!({"type": "frame", "width": name_w, "height": 12})];
    steps.extend(STEPS.iter().map(|s| with(text(s.to_string(), 10.0, MUTED, false), json!({"width": sw}))));
    let mut out = vec![
        section_title("Primitives", if c.pin_base { "OKLCH ramps · base colour marked" } else { "OKLCH ramps" }),
        row(gap, steps),
    ];
    for (name, swatches) in &ramps {
        let i = c.colors.iter().position(|x| &x.name == name);
        let (label, sub) = match i {
            Some(i) => (
                with(text(name.as_str(), 13.0, INK, true), json!({"field": format!("name{i}")})),
                with(text(c.colors[i].hex.as_str(), 11.0, MUTED, false), json!({"field": format!("hex{i}")})),
            ),
            None => (text(name.as_str(), 13.0, INK, true), text(format!("tint {}%", (c.neutral_tint * 100.0).round()), 11.0, MUTED, false)),
        };
        let base = i.map(|i| c.colors[i].hex.to_ascii_uppercase());
        let mut cells = vec![with(col(2, vec![label, sub]), json!({"width": name_w}))];
        for s in swatches {
            let hex = s.color.to_hex();
            let mut r = with(rect(sw, 36.0, &hex, 6.0), json!({"name": format!("{name}/{}", s.step)}));
            if c.pin_base && base.as_deref() == Some(hex.as_str()) {
                r = with(r, json!({"stroke": INK, "strokeWeight": 2}));
            }
            cells.push(r);
        }
        out.push(row(gap, cells));
    }
    out.push(text("Double-click a colour's name or hex to change it; Edit settings for tint, roles and more.", 11.0, MUTED, false));
    out
}

fn theme_card(c: &Config, ramps: &[(String, Vec<crate::color::ramp::Swatch>)], mode: &str, dark: bool) -> Value {
    let r = |role: &str| themes::resolve(c, ramps, role, dark).map_or_else(|| "#888888".to_string(), Rgb::to_hex);
    let chip = |kind: &str, label: &str| {
        with(row(0, vec![text(label, 11.0, &r(&format!("{kind}/default")), true)]), json!({"padding": 6, "radius": 999, "fill": r(&format!("{kind}/subtle"))}))
    };
    let pill = |label: &str, fill: String, ink: String, stroke: Option<String>| {
        let mut b = with(row(0, vec![text(label, 13.0, &ink, true)]), json!({"padding": 10, "radius": 8, "fill": fill}));
        if let Some(s) = stroke {
            b = with(b, json!({"stroke": s}));
        }
        b
    };
    let w = (INNER - 16.0) / 2.0;
    with(
        col(
            14,
            vec![
                text(mode.to_uppercase(), 11.0, &r("fg/muted"), true),
                text("Good morning", 22.0, &r("fg/default"), true),
                with(text("Your design tokens, freshly brewed and ready to pour.", 13.0, &r("fg/muted"), false), json!({"width": w - 40.0})),
                with(row(0, vec![text("Search tokens…", 13.0, &r("fg/subtle"), false)]), json!({"width": w - 40.0, "padding": 10, "radius": 8, "fill": r("bg/subtle"), "stroke": r("border/default")})),
                row(
                    8,
                    vec![
                        pill("Generate", r("accent/default"), r("accent/fg"), None),
                        pill("Preview", r("accent/subtle"), r("accent/default"), None),
                        pill("Cancel", r("bg/default"), r("fg/default"), Some(r("border/strong"))),
                    ],
                ),
                row(8, vec![chip("danger", "Error"), chip("success", "Saved"), chip("warning", "Pending")]),
            ],
        ),
        json!({"name": mode, "width": w, "padding": 20, "radius": 16, "fill": r("bg/default"), "stroke": r("border/default")}),
    )
}

fn themes_tab(c: &Config) -> Vec<Value> {
    let ramps = primitives::ramps(c);
    let checks = themes::report(c);
    let fails: Vec<_> = checks.iter().filter(|x| !x.passes()).collect();
    let mut out = vec![
        section_title("Themes", &format!("{} roles · Light and Dark alias Primitives", c.roles.len())),
        row(16, vec![theme_card(c, &ramps, themes::LIGHT, false), theme_card(c, &ramps, themes::DARK, true)]),
    ];
    let (mark, color) = if fails.is_empty() { ("✓", "#15803D") } else { ("⚠", "#B45309") };
    out.push(row(
        8,
        vec![text(mark, 13.0, color, true), text(format!("{} of {} WCAG contrast checks pass", checks.len() - fails.len(), checks.len()), 13.0, INK, true)],
    ));
    for f in fails.iter().take(4) {
        out.push(text(format!("{} on {} ({}): {:.2}:1, needs {}:1", f.fg, f.bg, f.mode, f.ratio, f.needs), 11.0, MUTED, false));
    }
    if fails.len() > 4 {
        out.push(text(format!("…and {} more: see Edit settings ▸ Themes.", fails.len() - 4), 11.0, MUTED, false));
    }
    out
}

fn sample(style: &str) -> &'static str {
    match style.split('/').next().unwrap_or_default() {
        "Display" | "Heading" => "Freshly brewed",
        "Body" => "The quick brown fox jumps over the lazy dog.",
        "Label" => "Button label",
        "Caption" => "Caption text · 12 May",
        _ => "let latte = brew(espresso, milk);",
    }
}

fn type_tab(c: &Config) -> Vec<Value> {
    let mut out = vec![section_title(
        "Typography",
        &format!("{} · {} · {} · scale {}", c.font_body, c.font_heading, c.font_mono, c.type_scale),
    )];
    for s in typography::styles(c) {
        let size = s.value["fontSize"].as_f64().unwrap_or(16.0);
        let lh = s.value["lineHeight"].as_f64().unwrap_or(24.0);
        let weight = s.value["fontStyle"].as_str().unwrap_or("Regular").to_string();
        let meta = with(
            col(2, vec![text(s.name.as_str(), 12.0, INK, true), text(format!("{size}/{lh} · {weight}"), 11.0, MUTED, false)]),
            json!({"width": 150}),
        );
        let specimen = with(text(sample(&s.name), size.min(56.0), INK, weight.contains("Bold")), json!({"width": INNER - 170.0}));
        out.push(row(20, vec![meta, specimen]));
    }
    out.push(text("Previewed in the board's typeface; the text styles use your fonts.", 11.0, MUTED, false));
    out
}

fn spacing_tab(c: &Config) -> Vec<Value> {
    let plan = sizes::collection(c);
    let first = |v: &tokens::VarPlan| match v.values.first() {
        Some((_, tokens::Val::Number(n))) => *n,
        _ => 0.0,
    };
    let spacing: Vec<_> = plan.vars.iter().filter(|v| v.name.starts_with("spacing/")).collect();
    let max = spacing.iter().map(|v| first(v)).fold(1.0, f64::max);
    let scale = ((INNER - 200.0) / max).min(4.0);
    let mut out = vec![section_title("Spacing", &format!("base {}px{}", c.spacing_base, if c.compact_mode { " · Compact mode too" } else { "" }))];
    let bars = spacing
        .iter()
        .map(|v| {
            let px = first(v);
            row(
                12,
                vec![
                    with(text(v.name.as_str(), 11.0, INK, false), json!({"width": 90})),
                    rect((px * scale).max(2.0), 10.0, if px == 0.0 { LINE } else { COFFEE }, 3.0),
                    text(format!("{px}px"), 11.0, MUTED, false),
                ],
            )
        })
        .collect();
    out.push(col(6, bars));
    out.push(section_title("Radius", &format!("base {}px", c.radius_base)));
    let tiles = plan
        .vars
        .iter()
        .filter(|v| v.name.starts_with("radius/"))
        .map(|v| {
            let r = first(v).min(28.0);
            with(
                col(6, vec![with(rect(56.0, 56.0, CREAM, r), json!({"stroke": COFFEE, "strokeWeight": 1.5})), text(v.name.trim_start_matches("radius/"), 11.0, MUTED, false)]),
                json!({"align": "center"}),
            )
        })
        .collect();
    out.push(row(14, tiles));
    out.push(section_title("Borders", ""));
    let borders = plan
        .vars
        .iter()
        .filter(|v| v.name.starts_with("border/"))
        .map(|v| {
            let w = first(v);
            let tile = if w > 0.0 {
                with(json!({"type": "rectangle", "width": 64, "height": 36, "radius": 8, "fill": FOAM}), json!({"stroke": INK, "strokeWeight": w}))
            } else {
                with(rect(64.0, 36.0, SOFT, 8.0), json!({}))
            };
            with(col(6, vec![tile, text(format!("{} {w}px", v.name.trim_start_matches("border/")), 11.0, MUTED, false)]), json!({"align": "center"}))
        })
        .collect();
    out.push(row(14, borders));
    out
}

/// A card with a soft shadow faked from a few translucent layers (widgets can't take effects).
fn elevated(level: &str, spec: &str, y: f64, blur: f64, opacity: f64, shadow: &str) -> Value {
    let (w, h, m) = (88.0, 72.0, 16.0);
    // Widest and faintest first: each layer a little smaller, darker and nearer the card, so they
    // add up to a soft falloff that sits lower the higher the level.
    const STEPS: usize = 5;
    let mut layers = Vec::new();
    for k in (1..=STEPS).rev() {
        let t = k as f64 / STEPS as f64;
        let grow = blur * 0.5 * t;
        layers.push(with(
            rect(w + 2.0 * grow, h + 2.0 * grow, &alpha(shadow, opacity * 0.55 * (1.2 - t)), 12.0 + grow),
            json!({"x": m - grow, "y": m - grow + y * (0.4 + 0.6 * t)}),
        ));
    }
    layers.push(with(
        col(2, vec![text(format!("Elevation {level}"), 12.0, INK, true), text(spec, 10.0, MUTED, false)]),
        json!({"x": m, "y": m, "width": w, "height": h, "radius": 12, "fill": CARD, "padding": 12}),
    ));
    json!({"type": "frame", "name": format!("Elevation {level}"), "width": w + 2.0 * m, "height": h + 2.0 * m + y, "children": layers})
}

fn effects_tab(c: &Config) -> Vec<Value> {
    let styles = tokens::effects::styles(c);
    let mut out = vec![section_title("Effects", &format!("{} effect styles", styles.len()))];
    let elevations: Vec<Value> = styles
        .iter()
        .filter(|s| s.name.starts_with("Elevation/"))
        .map(|s| {
            let key = s.value.get(1).cloned().unwrap_or(Value::Null);
            let y = key["offset"][1].as_f64().unwrap_or(0.0);
            let blur = key["radius"].as_f64().unwrap_or(0.0);
            let spec = format!("y {y} · blur {blur}");
            elevated(s.name.trim_start_matches("Elevation/"), &spec, y.min(14.0), blur.min(32.0), c.shadow_opacity, &c.shadow_color)
        })
        .collect();
    if !elevations.is_empty() {
        out.push(with(row(0, elevations), json!({"padding": 4, "radius": 14, "fill": BG})));
    }
    let mut extras = Vec::new();
    if c.focus_ring {
        let accent = c.colors.first().map_or("#4F46E5", |x| x.hex.as_str());
        let inner = with(row(0, vec![text("Focused", 13.0, INK, true)]), json!({"padding": 10, "radius": 10, "fill": FOAM, "stroke": LINE}));
        extras.push(with(row(0, vec![inner]), json!({"padding": 3, "radius": 13, "fill": alpha(accent, 0.45)})));
    }
    if c.blurs {
        for s in styles.iter().filter(|s| s.name.starts_with("Blur/")) {
            extras.push(with(row(0, vec![text(s.name.as_str(), 12.0, INK, true)]), json!({"padding": 10, "radius": 10, "fill": alpha(FOAM, 0.7), "stroke": LINE})));
        }
    }
    if !extras.is_empty() {
        out.push(row(12, extras));
    }
    out
}

/// The tab showing, as its key.
pub fn tab_of(state: &Value) -> &str {
    state["tab"].as_str().filter(|t| TABS.iter().any(|(k, _)| k == t)).unwrap_or("colours")
}

/// The panel section matching a tab, and back.
pub fn section_for(tab: &str) -> &'static str {
    match tab {
        "themes" => "Themes",
        "type" => "Typography",
        "spacing" => "Sizes",
        "effects" => "Effects",
        _ => "Colours",
    }
}

pub fn tab_for(section: &str) -> Option<&'static str> {
    match section {
        "Colours" => Some("colours"),
        "Themes" => Some("themes"),
        "Typography" => Some("type"),
        "Sizes" => Some("spacing"),
        "Effects" => Some("effects"),
        _ => None,
    }
}

/// The whole board for `config` with tab `tab` open and `status` (the last Generate) at the foot.
pub fn render(c: &Config, tab: &str, status: &str) -> Value {
    let body = match tab {
        "themes" => themes_tab(c),
        "type" => type_tab(c),
        "spacing" => spacing_tab(c),
        "effects" => effects_tab(c),
        _ => colours(c),
    };
    let content = with(col(16, body), json!({"name": "Content", "width": WIDTH - 2.0 * PAD, "padding": 24, "radius": 16, "fill": CARD, "stroke": LINE}));
    let foot = text(if status.is_empty() { "Not generated yet: press Generate to add these variables and styles to the file." } else { status }, 11.0, MUTED, false);
    with(
        col(20, vec![header(c), tabs(tab), content, with(foot, json!({"width": WIDTH - 2.0 * PAD}))]),
        json!({"name": "Latte", "width": WIDTH, "padding": PAD, "radius": 24, "fill": BG, "stroke": LINE}),
    )
}

/// The property menu shown while the board is selected.
pub fn menu() -> Value {
    json!([{"item": "generate", "label": "Generate"}, {"item": "edit", "label": "Edit settings"}])
}

/// Layers in a description, and how deep it nests.
pub fn measure(v: &Value) -> (usize, usize) {
    let kids = v["children"].as_array().map(|a| a.iter().map(measure).collect::<Vec<_>>()).unwrap_or_default();
    (1 + kids.iter().map(|k| k.0).sum::<usize>(), 1 + kids.iter().map(|k| k.1).max().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamedColor;

    #[test]
    fn every_tab_fits_figcrafts_limits() {
        // The busiest config: six colours, compact mode, every effect.
        let mut c = Config::default();
        for (n, h) in [("teal", "#0D9488"), ("pink", "#DB2777")] {
            c.colors.push(NamedColor { name: n.into(), hex: h.into() });
        }
        c.elevations = 6;
        c.compact_mode = true;
        let c = c.normalized();
        for (tab, _) in TABS {
            let v = render(&c, tab, "Variables: 128 created.");
            let (layers, depth) = measure(&v);
            assert!(layers <= 256, "{tab}: {layers} layers");
            assert!(depth <= 16, "{tab}: depth {depth}");
            let bytes = figcraft_plugin_sdk::serde_json::to_string(&v).map_or(0, |t| t.len());
            assert!(bytes < 4 * 1024 * 1024, "{tab}");
        }
    }

    #[test]
    fn targets_and_fields() {
        let v = render(&Config::default(), "colours", "");
        let text = figcraft_plugin_sdk::serde_json::to_string(&v).unwrap_or_default();
        for t in ["\"target\":\"generate\"", "\"target\":\"edit\"", "\"target\":\"tab.themes\"", "\"field\":\"hex0\"", "\"field\":\"name3\""] {
            assert!(text.contains(t), "{t}");
        }
        assert_eq!(section_for(tab_of(&json!({"tab": "type"}))), "Typography");
        assert_eq!(tab_for("Sizes"), Some("spacing"));
        assert_eq!(tab_of(&json!({"tab": "nope"})), "colours");
    }
}
