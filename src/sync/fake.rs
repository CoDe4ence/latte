//! A small in-memory stand-in for FigCraft's variable and style commands, answering in the shapes
//! FigCraft's own `variable.list` and `style.list` use, so sync can be tested on the host.

use figcraft_plugin_sdk::{Result, Value, json};

use super::Host;
use crate::color::Rgb;

#[derive(Default)]
pub struct Fake {
    collections: Vec<Value>,
    variables: Vec<Value>,
    styles: Vec<Value>,
    next: u64,
    /// Every command called, in order.
    pub calls: Vec<String>,
}

fn color(v: &Value) -> Value {
    let t = v.as_str().unwrap_or_default();
    let c = Rgb::from_hex(t).unwrap_or(Rgb::BLACK);
    let h = t.trim_start_matches('#');
    let a = if h.len() == 8 { u8::from_str_radix(&h[6..], 16).map_or(1.0, |a| f64::from(a) / 255.0) } else { 1.0 };
    json!({"r": c.r, "g": c.g, "b": c.b, "a": a})
}

impl Fake {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}:{}", self.next)
    }

    fn coll_index(&self, key: &str) -> Option<usize> {
        self.collections.iter().position(|c| c["id"] == key || c["name"] == key)
    }

    fn var_index(&self, key: &str) -> Option<usize> {
        self.variables.iter().position(|v| v["id"] == key).or_else(|| self.variables.iter().position(|v| v["name"] == key))
    }

    pub fn mode_id(&self, collection: &str, mode: &str) -> Option<String> {
        let c = &self.collections[self.coll_index(collection)?];
        c["modes"].as_array()?.iter().find(|m| m["name"] == mode || m["id"] == mode).and_then(|m| m["id"].as_str()).map(str::to_string)
    }

    pub fn var(&self, collection: &str, name: &str) -> Option<&Value> {
        let cid = self.collections[self.coll_index(collection)?]["id"].clone();
        self.variables.iter().find(|v| v["collection"] == cid && v["name"] == name)
    }

    pub fn var_id(&self, collection: &str, name: &str) -> Option<String> {
        self.var(collection, name).and_then(|v| v["id"].as_str()).map(str::to_string)
    }

    pub fn style(&self, name: &str) -> Option<&Value> {
        self.styles.iter().find(|s| s["name"] == name)
    }

    fn value(&self, kind: &str, v: &Value) -> Result<Value> {
        if let Some(a) = v.get("alias") {
            let i = self.var_index(a.as_str().unwrap_or_default()).ok_or("no variable to alias")?;
            return Ok(json!({"type": "alias", "value": self.variables[i]["id"]}));
        }
        Ok(match kind {
            "color" => json!({"type": "color", "value": color(v)}),
            k => json!({"type": k, "value": v}),
        })
    }

    fn text_style(v: &Value) -> Value {
        let pct = v["letterSpacing"].as_str().and_then(|p| p.trim_end_matches('%').parse::<f64>().ok()).unwrap_or(0.0);
        json!({
            "font_family": v["fontFamily"], "font_style": v["fontStyle"], "font_size": v["fontSize"],
            "line_height": {"unit": "pixels", "value": v["lineHeight"]},
            "letter_spacing": {"unit": "percent", "value": pct},
        })
    }

    fn effects(v: &Value) -> Value {
        Value::Array(
            v.as_array()
                .map(|a| {
                    a.iter()
                        .map(|e| {
                            let mut e = e.clone();
                            if e.get("color").is_some() {
                                e["color"] = color(&e["color"]);
                            }
                            e
                        })
                        .collect()
                })
                .unwrap_or_default(),
        )
    }
}

impl Host for Fake {
    fn call(&mut self, command: &str, p: &Value) -> Result<Value> {
        self.calls.push(command.to_string());
        let st = |k: &str| p[k].as_str().unwrap_or_default().to_string();
        match command {
            "variable.list" => Ok(json!({"collections": self.collections, "variables": self.variables})),
            "variable.createCollection" => {
                if self.coll_index(&st("name")).is_some() {
                    return Err("exists".into());
                }
                let id = self.id("C");
                let modes: Vec<Value> =
                    p["modes"].as_array().cloned().unwrap_or_default().iter().enumerate().map(|(i, m)| json!({"id": format!("{id}/mode{}", i + 1), "name": m})).collect();
                self.collections.push(json!({"id": id, "name": st("name"), "modes": modes}));
                Ok(json!({"id": id}))
            }
            "variable.addMode" => {
                let ci = self.coll_index(&st("collection")).ok_or("no collection")?;
                let cid = self.collections[ci]["id"].clone();
                let first = self.collections[ci]["modes"][0]["id"].as_str().unwrap_or_default().to_string();
                let mid = self.id("M");
                if let Some(m) = self.collections[ci]["modes"].as_array_mut() {
                    m.push(json!({"id": mid, "name": st("name")}));
                }
                for v in self.variables.iter_mut().filter(|v| v["collection"] == cid) {
                    let copy = v["values"][&first].clone();
                    v["values"][&mid] = copy;
                }
                Ok(json!({"id": mid}))
            }
            "variable.create" => {
                let ci = self.coll_index(&st("collection")).ok_or("no collection")?;
                let (cid, kind) = (self.collections[ci]["id"].clone(), st("type"));
                let mut values = figcraft_plugin_sdk::serde_json::Map::new();
                for (m, v) in p["values"].as_object().cloned().unwrap_or_default() {
                    let mid = self.mode_id(&st("collection"), &m).ok_or("no mode")?;
                    values.insert(mid, self.value(&kind, &v)?);
                }
                if p.get("value").is_some() {
                    for m in self.collections[ci]["modes"].as_array().cloned().unwrap_or_default() {
                        values.insert(m["id"].as_str().unwrap_or_default().to_string(), self.value(&kind, &p["value"])?);
                    }
                }
                let id = self.id("V");
                self.variables.push(json!({"id": id, "name": st("name"), "collection": cid, "resolvedType": kind, "values": values, "description": p["description"]}));
                Ok(json!({"id": id}))
            }
            "variable.set" => {
                let i = self.var_index(&st("variable")).ok_or("no variable")?;
                let kind = self.variables[i]["resolvedType"].as_str().unwrap_or_default().to_string();
                let val = self.value(&kind, &p["value"])?;
                self.variables[i]["values"][st("mode")] = val;
                Ok(json!({}))
            }
            "variable.update" => {
                let i = self.var_index(&st("variable")).ok_or("no variable")?;
                if p.get("scopes").is_some() {
                    self.variables[i]["scopes"] = p["scopes"].clone();
                }
                Ok(json!({}))
            }
            "style.list" => Ok(Value::Array(self.styles.clone())),
            "style.create" => {
                let id = self.id("S");
                let mut s = json!({"id": id, "name": st("name"), "description": p["description"], "type": st("type")});
                match st("type").as_str() {
                    "text" => s["style"] = Fake::text_style(&p["value"]),
                    _ => s["effects"] = Fake::effects(&p["value"]),
                }
                self.styles.push(s);
                Ok(json!({"id": id}))
            }
            "style.update" => {
                let i = self.styles.iter().position(|s| s["id"] == p["style"]).ok_or("no style")?;
                match self.styles[i]["type"].as_str() {
                    Some("text") => self.styles[i]["style"] = Fake::text_style(&p["value"]),
                    _ => self.styles[i]["effects"] = Fake::effects(&p["value"]),
                }
                // A new value keeps no bindings in this stand-in (the worst case for sync).
                if let Some(o) = self.styles[i].as_object_mut() {
                    o.remove("bound");
                }
                Ok(json!({}))
            }
            "style.bind" => {
                let i = self.styles.iter().position(|s| s["id"] == p["style"]).ok_or("no style")?;
                if self.styles[i].get("bound").is_none() {
                    self.styles[i]["bound"] = json!({});
                }
                self.styles[i]["bound"][st("field")] = p["variable"].clone();
                Ok(json!({}))
            }
            other => Err(format!("the fake doesn't do {other}")),
        }
    }
}
