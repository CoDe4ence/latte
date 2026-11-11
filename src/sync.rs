//! Make the document match a [`TokenPlan`]: create what's missing, update values that differ,
//! leave the rest. Matching is by collection, variable and style name, so running it again
//! changes nothing, and editing the config and running it again updates values in place (layers
//! bound to the variables follow). Latte never deletes: variables and styles it no longer plans,
//! and anything the user made, stay as they are.

use std::collections::BTreeMap;

use figcraft_plugin_sdk::{Result, Value, json};

use crate::color::Rgb;
use crate::tokens::{StylePlan, StyleType, TokenPlan, Val, VarRef};

/// FigCraft, or a stand-in in tests.
pub trait Host {
    fn call(&mut self, command: &str, params: &Value) -> Result<Value>;
}

/// The real document, through the plugin SDK.
pub struct Figcraft;

impl Host for Figcraft {
    fn call(&mut self, command: &str, params: &Value) -> Result<Value> {
        figcraft_plugin_sdk::call(command, params)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub collections_created: usize,
    pub modes_added: usize,
    pub variables_created: usize,
    pub variables_updated: usize,
    pub variables_unchanged: usize,
    pub styles_created: usize,
    pub styles_updated: usize,
    pub styles_unchanged: usize,
    /// Things left alone, with why (a name taken by a variable of another type…).
    pub skipped: Vec<String>,
}

impl Summary {
    pub fn changed(&self) -> bool {
        self.collections_created + self.modes_added + self.variables_created + self.variables_updated + self.styles_created + self.styles_updated > 0
    }

    pub fn text(&self) -> String {
        if !self.changed() && self.skipped.is_empty() {
            return format!(
                "Already in sync: {} variables and {} styles unchanged.",
                self.variables_unchanged, self.styles_unchanged
            );
        }
        let mut s = format!(
            "Variables: {} created, {} updated, {} unchanged. Styles: {} created, {} updated, {} unchanged.",
            self.variables_created, self.variables_updated, self.variables_unchanged, self.styles_created, self.styles_updated, self.styles_unchanged
        );
        if self.collections_created > 0 {
            s.push_str(&format!(" {} new collections.", self.collections_created));
        }
        if self.modes_added > 0 {
            s.push_str(&format!(" {} new modes.", self.modes_added));
        }
        if !self.skipped.is_empty() {
            s.push_str(&format!(" Skipped {}: {}", self.skipped.len(), self.skipped.join("; ")));
        }
        s
    }
}

/// A collection as `variable.list` reports it.
#[derive(Clone, Debug)]
struct Coll {
    id: String,
    name: String,
    /// (mode id, mode name)
    modes: Vec<(String, String)>,
}

/// A variable as `variable.list` reports it.
#[derive(Clone, Debug)]
struct Var {
    id: String,
    name: String,
    collection: String,
    kind: String,
    /// Value per mode id, as listed (`{"type": "color", "value": …}`).
    values: BTreeMap<String, Value>,
}

#[derive(Default)]
struct State {
    collections: Vec<Coll>,
    vars: Vec<Var>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn as_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| i.as_str().map(str::to_string).or_else(|| i.as_u64().map(|n| n.to_string())))
}

impl State {
    fn load(host: &mut dyn Host) -> Result<State> {
        let list = host.call("variable.list", &json!({}))?;
        let collections = list["collections"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|c| Coll {
                        id: s(c, "id"),
                        name: s(c, "name"),
                        modes: c["modes"].as_array().map(|m| m.iter().map(|m| (s(m, "id"), s(m, "name"))).collect()).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let vars = list["variables"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| Var {
                        id: s(v, "id"),
                        name: s(v, "name"),
                        collection: s(v, "collection"),
                        kind: s(v, "resolvedType"),
                        values: v["values"].as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(State { collections, vars })
    }

    fn collection(&self, name: &str) -> Option<&Coll> {
        self.collections.iter().find(|c| c.name == name)
    }

    fn var(&self, collection_id: &str, name: &str) -> Option<&Var> {
        self.vars.iter().find(|v| v.collection == collection_id && v.name == name)
    }

    /// The id of the variable a reference names.
    fn resolve(&self, r: &VarRef) -> Option<String> {
        let c = self.collection(r.collection)?;
        self.var(&c.id, &r.name).map(|v| v.id.clone())
    }
}

/// The value as `variable.create` / `variable.set` take it.
fn param(state: &State, v: &Val) -> Result<Value> {
    Ok(match v {
        Val::Color(hex) => json!(hex),
        Val::Number(n) => json!(n),
        Val::Str(t) => json!(t),
        Val::Alias(r) => json!({"alias": state.resolve(r).ok_or_else(|| format!("{}/{} doesn't exist", r.collection, r.name))?}),
    })
}

/// A listed colour (`{r, g, b, a}` in 0..1, an array or a hex string) as `#RRGGBB`, if it's opaque.
fn listed_hex(c: &Value) -> Option<String> {
    if let Some(t) = c.as_str() {
        return Rgb::from_hex(t).map(Rgb::to_hex);
    }
    let get = |k: &str, i: usize| c.get(k).or_else(|| c.get(i)).and_then(Value::as_f64);
    let a = get("a", 3).unwrap_or(1.0);
    ((a - 1.0).abs() < 1e-3).then_some(())?;
    Some(Rgb { r: get("r", 0)?, g: get("g", 1)?, b: get("b", 2)? }.to_hex())
}

/// Whether a listed value is already the planned one.
fn same(state: &State, listed: Option<&Value>, want: &Val) -> bool {
    let Some(l) = listed else { return false };
    let (t, v) = (l["type"].as_str().unwrap_or_default(), &l["value"]);
    match want {
        Val::Color(hex) => t == "color" && listed_hex(v).as_deref() == Some(hex.as_str()),
        Val::Number(n) => t == "number" && v.as_f64().is_some_and(|x| (x - n).abs() < 1e-6),
        Val::Str(text) => t == "string" && v.as_str() == Some(text.as_str()),
        Val::Alias(r) => t == "alias" && state.resolve(r).is_some_and(|id| v.as_str() == Some(id.as_str())),
    }
}

/// Bring the document in line with `plan`. Everything happens in one plugin run, so it's one undo
/// step, and a failure part way rolls all of it back.
pub fn apply(host: &mut dyn Host, plan: &TokenPlan) -> Result<Summary> {
    let mut sum = Summary::default();
    let mut st = State::load(host)?;

    for coll in &plan.collections {
        // The collection, with every planned mode.
        if st.collection(coll.name).is_none() {
            host.call("variable.createCollection", &json!({"name": coll.name, "modes": coll.modes}))?;
            sum.collections_created += 1;
            st = State::load(host)?;
        }
        let Some(c) = st.collection(coll.name).cloned() else { return Err(format!("couldn't make the {} collection", coll.name)) };
        let mut added = false;
        for m in &coll.modes {
            if !c.modes.iter().any(|(_, n)| n == m) {
                host.call("variable.addMode", &json!({"collection": c.id, "name": m}))?;
                sum.modes_added += 1;
                added = true;
            }
        }
        if added {
            st = State::load(host)?;
        }
        let Some(c) = st.collection(coll.name).cloned() else { return Err(format!("the {} collection went missing", coll.name)) };
        let mode_id = |name: &str| c.modes.iter().find(|(_, n)| n == name).map(|(id, _)| id.clone());

        for v in &coll.vars {
            match st.var(&c.id, &v.name).cloned() {
                None => {
                    let mut values = serde_json_map();
                    for (m, val) in &v.values {
                        values.insert(m.clone(), param(&st, val)?);
                    }
                    let mut p = json!({"collection": c.id, "name": v.name, "type": v.kind.id(), "values": values});
                    if !v.description.is_empty() {
                        p["description"] = json!(v.description);
                    }
                    let r = host.call("variable.create", &p)?;
                    let id = as_id(&r).ok_or_else(|| format!("variable.create gave no id for {}", v.name))?;
                    if let Some(scopes) = &v.scopes {
                        host.call("variable.update", &json!({"variable": id, "scopes": scopes}))?;
                    }
                    // Known from now on, so later aliases can name it.
                    let mut listed = BTreeMap::new();
                    for (m, val) in &v.values {
                        if let Some(mid) = mode_id(m) {
                            listed.insert(mid, listed_form(&st, val));
                        }
                    }
                    st.vars.push(Var { id, name: v.name.clone(), collection: c.id.clone(), kind: v.kind.id().into(), values: listed });
                    sum.variables_created += 1;
                }
                Some(e) if e.kind != v.kind.id() => {
                    sum.skipped.push(format!("{}/{} is a {} variable, not {}", coll.name, v.name, e.kind, v.kind.id()));
                }
                Some(e) => {
                    let mut changed = Vec::new();
                    for (m, val) in &v.values {
                        let Some(mid) = mode_id(m) else { continue };
                        if !same(&st, e.values.get(&mid), val) {
                            host.call("variable.set", &json!({"variable": e.id, "mode": mid, "value": param(&st, val)?}))?;
                            changed.push((mid, listed_form(&st, val)));
                        }
                    }
                    if !changed.is_empty() {
                        if let Some(x) = st.vars.iter_mut().find(|x| x.id == e.id) {
                            x.values.extend(changed);
                        }
                        sum.variables_updated += 1;
                    } else {
                        sum.variables_unchanged += 1;
                    }
                }
            }
        }
    }

    sync_styles(host, &st, &plan.styles, &mut sum)?;
    Ok(sum)
}

fn serde_json_map() -> figcraft_plugin_sdk::serde_json::Map<String, Value> {
    figcraft_plugin_sdk::serde_json::Map::new()
}

/// How `variable.list` would show a value just set (only used for aliases made in this run).
fn listed_form(st: &State, v: &Val) -> Value {
    match v {
        Val::Color(hex) => json!({"type": "color", "value": hex}),
        Val::Number(n) => json!({"type": "number", "value": n}),
        Val::Str(t) => json!({"type": "string", "value": t}),
        Val::Alias(r) => json!({"type": "alias", "value": st.resolve(r)}),
    }
}

fn close(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-3,
        _ => false,
    }
}

/// A listed `{unit, value}` length (line height, letter spacing) as (unit, value).
fn length(v: &Value) -> (String, Option<f64>) {
    (s(v, "unit").to_ascii_lowercase(), v.get("value").and_then(Value::as_f64))
}

/// Whether a listed text style already has the planned value.
fn same_text(listed: &Value, want: &Value) -> bool {
    let t = &listed["style"];
    let field = |snake: &str, camel: &str| t.get(snake).or_else(|| t.get(camel)).cloned().unwrap_or(Value::Null);
    let pct = want["letterSpacing"].as_str().and_then(|p| p.trim_end_matches('%').parse::<f64>().ok());
    let (lh_unit, lh) = length(&field("line_height", "lineHeight"));
    let (ls_unit, ls) = length(&field("letter_spacing", "letterSpacing"));
    field("font_family", "fontFamily").as_str() == want["fontFamily"].as_str()
        && field("font_style", "fontStyle").as_str() == want["fontStyle"].as_str()
        && close(field("font_size", "fontSize").as_f64(), want["fontSize"].as_f64())
        && lh_unit.starts_with("pixel")
        && close(lh, want["lineHeight"].as_f64())
        && (ls_unit.starts_with("percent") && close(ls, pct) || pct == Some(0.0) && ls.unwrap_or(0.0) == 0.0)
}

/// A listed or planned colour as `#RRGGBBAA`.
fn hex8(c: &Value) -> Option<String> {
    if let Some(t) = c.as_str() {
        let h = t.trim_start_matches('#').to_ascii_uppercase();
        return Some(if h.len() == 6 { format!("#{h}FF") } else { format!("#{h}") });
    }
    let get = |k: &str, i: usize| c.get(k).or_else(|| c.get(i)).and_then(Value::as_f64);
    let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(format!("#{:02X}{:02X}{:02X}{:02X}", q(get("r", 0)?), q(get("g", 1)?), q(get("b", 2)?), q(get("a", 3).unwrap_or(1.0))))
}

/// Whether a listed effect style already has the planned effects.
fn same_effects(listed: &Value, want: &Value) -> bool {
    let (Some(have), Some(want)) = (listed["effects"].as_array(), want.as_array()) else { return false };
    have.len() == want.len()
        && have.iter().zip(want).all(|(h, w)| {
            let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64);
            let off = |v: &Value, i: usize| v.get("offset").and_then(|o| o.get(i).or_else(|| o.get(["x", "y"][i]))).and_then(Value::as_f64);
            s(h, "type") == s(w, "type")
                && close(num(h, "radius"), num(w, "radius"))
                && (w.get("color").is_none() || hex8(&h["color"]) == hex8(&w["color"]))
                && (w.get("spread").is_none() || close(num(h, "spread"), num(w, "spread")))
                && (w.get("offset").is_none() || (close(off(h, 0), off(w, 0)) && close(off(h, 1), off(w, 1))))
        })
}

fn sync_styles(host: &mut dyn Host, st: &State, styles: &[StylePlan], sum: &mut Summary) -> Result<()> {
    let listed = host.call("style.list", &json!({}))?;
    let listed = listed.as_array().cloned().unwrap_or_default();
    for p in styles {
        let binds: Vec<(&str, String)> = p.binds.iter().filter_map(|(f, r)| st.resolve(r).map(|id| (*f, id))).collect();
        let bind_all = |host: &mut dyn Host, sid: &str, have: &Value| -> Result<()> {
            for (field, var) in &binds {
                if have.get(*field).and_then(Value::as_str) != Some(var.as_str()) {
                    host.call("style.bind", &json!({"style": sid, "field": field, "variable": var}))?;
                }
            }
            Ok(())
        };
        match listed.iter().find(|x| s(x, "name") == p.name) {
            None => {
                let mut params = json!({"type": p.kind.id(), "name": p.name, "value": p.value});
                if !p.description.is_empty() {
                    params["description"] = json!(p.description);
                }
                let r = host.call("style.create", &params)?;
                let sid = as_id(&r).ok_or_else(|| format!("style.create gave no id for {}", p.name))?;
                bind_all(host, &sid, &Value::Null)?;
                sum.styles_created += 1;
            }
            Some(e) if s(e, "type") != p.kind.id() => {
                sum.skipped.push(format!("style {} is a {} style, not {}", p.name, s(e, "type"), p.kind.id()));
            }
            Some(e) => {
                let sid = s(e, "id");
                let same = match p.kind {
                    StyleType::Text => same_text(e, &p.value),
                    StyleType::Effect => same_effects(e, &p.value),
                };
                let bound = e.get("bound").cloned().unwrap_or(Value::Null);
                let rebind = binds.iter().any(|(f, v)| bound.get(*f).and_then(Value::as_str) != Some(v.as_str()));
                if same && !rebind {
                    sum.styles_unchanged += 1;
                    continue;
                }
                if !same {
                    host.call("style.update", &json!({"style": sid, "value": p.value}))?;
                }
                // A new value can drop bindings; bind again whatever isn't bound now.
                let now = if same { bound } else { Value::Null };
                bind_all(host, &sid, &now)?;
                sum.styles_updated += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub mod fake;

#[cfg(test)]
mod tests {
    use super::fake::Fake;
    use super::*;
    use crate::config::Config;
    use crate::tokens;

    #[test]
    fn generating_twice_changes_nothing_the_second_time() {
        let plan = tokens::plan(&Config::default());
        let mut doc = Fake::default();
        let first = apply(&mut doc, &plan).unwrap_or_default();
        assert_eq!(first.collections_created, 4);
        assert_eq!(first.variables_created, plan.variable_count());
        assert_eq!(first.styles_created, plan.styles.len());
        assert!(first.skipped.is_empty(), "{:?}", first.skipped);
        let calls = doc.calls.len();

        let second = apply(&mut doc, &plan).unwrap_or_default();
        assert!(!second.changed(), "{second:?}");
        assert_eq!(second.variables_unchanged, plan.variable_count());
        assert_eq!(second.styles_unchanged, plan.styles.len());
        // Only the two lists were read.
        assert_eq!(doc.calls[calls..], ["variable.list".to_string(), "style.list".to_string()]);
    }

    #[test]
    fn themes_alias_primitives_and_text_styles_are_bound() {
        let mut doc = Fake::default();
        let _ = apply(&mut doc, &tokens::plan(&Config::default()));
        let prim = doc.var_id("Primitives", "brand/600").unwrap_or_default();
        let accent = doc.var("Themes", "accent/default").cloned().unwrap_or(Value::Null);
        let light = doc.mode_id("Themes", "Light").unwrap_or_default();
        assert_eq!(accent["values"][&light], json!({"type": "alias", "value": prim}));
        assert_eq!(accent["scopes"], json!(["FRAME_FILL", "SHAPE_FILL", "TEXT_FILL", "STROKE_COLOR", "EFFECT_COLOR"]));
        let body = doc.style("Body/Default").cloned().unwrap_or(Value::Null);
        assert_eq!(body["bound"]["fontSize"], json!(doc.var_id("Typography", "size/base")));
    }

    #[test]
    fn a_config_change_updates_in_place() {
        let mut config = Config::default();
        let mut doc = Fake::default();
        let _ = apply(&mut doc, &tokens::plan(&config));
        let before = doc.var_id("Primitives", "brand/500");

        config.colors[0].hex = "#0D99FF".into();
        config.base_size = 18.0;
        config.compact_mode = true;
        let s = apply(&mut doc, &tokens::plan(&config)).unwrap_or_default();
        assert_eq!(s.collections_created, 0);
        assert_eq!(s.modes_added, 1);
        assert_eq!(s.variables_created, 0);
        assert!(s.variables_updated > 11, "{s:?}");
        assert!(s.styles_updated > 0);
        // Same variable, new value.
        assert_eq!(doc.var_id("Primitives", "brand/500"), before);
        // And now it's in sync again.
        assert!(!apply(&mut doc, &tokens::plan(&config)).unwrap_or_default().changed());
    }

    #[test]
    fn a_users_variable_of_another_type_is_left_alone() {
        let mut doc = Fake::default();
        let _ = doc.call("variable.createCollection", &json!({"name": "Sizes", "modes": ["Default"]}));
        let _ = doc.call("variable.create", &json!({"collection": "Sizes", "name": "spacing/1", "type": "string", "value": "mine"}));
        let s = apply(&mut doc, &tokens::plan(&Config::default())).unwrap_or_default();
        assert_eq!(s.collections_created, 3);
        assert_eq!(s.skipped.len(), 1, "{:?}", s.skipped);
        assert_eq!(doc.var("Sizes", "spacing/1").map(|v| v["values"].clone()).unwrap_or_default().as_object().map(|o| o.len()), Some(1));
    }
}
