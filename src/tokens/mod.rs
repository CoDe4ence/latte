//! A design system as plain data: the collections, variables and styles Latte wants in the
//! document. Building it touches nothing; [`crate::sync`] makes the document match it.

pub mod effects;
pub mod primitives;
pub mod sizes;
pub mod themes;
pub mod typography;

use figcraft_plugin_sdk::Value;

use crate::config::Config;

pub const PRIMITIVES: &str = "Primitives";
pub const THEMES: &str = "Themes";
pub const SIZES: &str = "Sizes";
pub const TYPOGRAPHY: &str = "Typography";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Color,
    Number,
    String,
}

impl Kind {
    /// The type name FigCraft's variable commands use.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Color => "color",
            Kind::Number => "number",
            Kind::String => "string",
        }
    }
}

/// A variable in another (or the same) collection, by name.
#[derive(Clone, Debug, PartialEq)]
pub struct VarRef {
    pub collection: &'static str,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// `#RRGGBB`.
    Color(String),
    Number(f64),
    Str(String),
    Alias(VarRef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarPlan {
    pub name: String,
    pub kind: Kind,
    /// Value per mode name, in the collection's mode order.
    pub values: Vec<(String, Val)>,
    /// Scope ids pickers offer the variable for (None: every property; empty: hidden from
    /// pickers). Set when the variable is made; later edits by hand are kept.
    pub scopes: Option<Vec<&'static str>>,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollectionPlan {
    pub name: &'static str,
    pub modes: Vec<String>,
    pub vars: Vec<VarPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleType {
    Text,
    Effect,
}

impl StyleType {
    pub fn id(self) -> &'static str {
        match self {
            StyleType::Text => "text",
            StyleType::Effect => "effect",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StylePlan {
    pub kind: StyleType,
    pub name: String,
    /// The style's value, as `style.create` takes it.
    pub value: Value,
    /// Fields of the value bound to variables (`fontSize`, `fontFamily`, `lineHeight`…).
    pub binds: Vec<(&'static str, VarRef)>,
    pub description: String,
}

/// Everything Latte wants in the document, in the order it must be made (Themes alias
/// Primitives, text styles bind Typography).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenPlan {
    pub collections: Vec<CollectionPlan>,
    pub styles: Vec<StylePlan>,
}

impl TokenPlan {
    pub fn variable_count(&self) -> usize {
        self.collections.iter().map(|c| c.vars.len()).sum()
    }

    pub fn style_count(&self, kind: StyleType) -> usize {
        self.styles.iter().filter(|s| s.kind == kind).count()
    }
}

/// The whole design system for `config`.
pub fn plan(config: &Config) -> TokenPlan {
    let ramps = primitives::ramps(config);
    TokenPlan {
        collections: vec![
            primitives::collection(&ramps),
            themes::collection(config),
            sizes::collection(config),
            typography::collection(config),
        ],
        styles: typography::styles(config).into_iter().chain(effects::styles(config)).collect(),
    }
}

/// A one-mode value list.
pub(crate) fn single(mode: &str, v: Val) -> Vec<(String, Val)> {
    vec![(mode.to_string(), v)]
}

/// A number with at most two decimals (no float noise in variable values or names).
pub(crate) fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_plan() {
        let p = plan(&Config::default());
        let names: Vec<_> = p.collections.iter().map(|c| c.name).collect();
        assert_eq!(names, [PRIMITIVES, THEMES, SIZES, TYPOGRAPHY]);
        // 4 colours + neutral, 11 steps each, plus white and black.
        assert_eq!(p.collections[0].vars.len(), 5 * 11 + 2);
        assert_eq!(p.collections[1].modes, ["Light", "Dark"]);
        assert!(p.style_count(StyleType::Text) >= 10);
        assert!(p.style_count(StyleType::Effect) >= 5);
        // Names are unique within each collection and among styles.
        for c in &p.collections {
            let mut n: Vec<_> = c.vars.iter().map(|v| &v.name).collect();
            n.sort();
            n.dedup();
            assert_eq!(n.len(), c.vars.len(), "{}", c.name);
        }
        let mut s: Vec<_> = p.styles.iter().map(|s| &s.name).collect();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), p.styles.len());
    }

    #[test]
    fn every_alias_and_binding_points_at_a_planned_variable() {
        let p = plan(&Config::default());
        let exists = |r: &VarRef| p.collections.iter().any(|c| c.name == r.collection && c.vars.iter().any(|v| v.name == r.name));
        for c in &p.collections {
            for v in &c.vars {
                for (_, val) in &v.values {
                    if let Val::Alias(r) = val {
                        assert!(exists(r), "{} → {r:?}", v.name);
                    }
                }
            }
        }
        for s in &p.styles {
            for (_, r) in &s.binds {
                assert!(exists(r), "{} → {r:?}", s.name);
            }
        }
    }
}
