//! Latte: design-system generation and management for FigCraft.
//!
//! Latte turns a handful of choices (brand colours, a spacing unit, fonts and a type scale,
//! shadows) into native variables and styles, organised as four collections: **Primitives**
//! (tonal ramps), **Themes** (semantic colours, Light and Dark, aliasing Primitives), **Sizes**
//! (spacing, radii, borders) and **Typography** (families, sizes, line heights, weights), plus
//! text styles bound to Typography and effect styles. Running it again re-syncs: what's missing is
//! made and what changed is updated in place.

pub mod color;
pub mod config;
pub mod panel;
pub mod sync;
pub mod tokens;

use figcraft_plugin_sdk::{Event, Input, Result, Value, call, close_ui, entry, json, notify, serde_json, set_setting, setting, show_ui};

use config::Config;

/// Where the config lives: a user setting and, once generated, the document's first page.
const CONFIG_KEY: &str = "config";
const DOC_KEY: &str = "latte.config";

fn first_page() -> Option<u64> {
    call("page.list", &json!({})).ok()?.as_array()?.first()?.get("id")?.as_u64()
}

/// The config: the document's (so a file keeps its own design system), else the user's last one,
/// else the defaults.
fn load_config() -> Config {
    let from_doc = first_page()
        .and_then(|p| call("node.get", &json!({"id": p})).ok())
        .and_then(|n| n["pluginData"][DOC_KEY].as_str().and_then(|t| serde_json::from_str::<Config>(t).ok()));
    from_doc
        .or_else(|| setting(CONFIG_KEY).and_then(|v| serde_json::from_value(v).ok()))
        .unwrap_or_default()
        .normalized()
}

fn save_setting(c: &Config) -> Result<()> {
    set_setting(CONFIG_KEY, &serde_json::to_value(c).map_err(|e| e.to_string())?)
}

/// Keep the config in the document, beside what it generated.
fn save_to_document(c: &Config) -> Result<()> {
    let page = first_page().ok_or("the document has no page")?;
    let text = serde_json::to_string(c).map_err(|e| e.to_string())?;
    call("node.set", &json!({"ids": [page], "props": {"pluginData": {DOC_KEY: text}}}))?;
    Ok(())
}

/// Generate (or re-sync) the design system for `c`.
fn generate(c: &Config) -> Result<sync::Summary> {
    let summary = sync::apply(&mut sync::Figcraft, &tokens::plan(c))?;
    save_to_document(c)?;
    save_setting(c)?;
    Ok(summary)
}

fn summary_json(s: &sync::Summary) -> Value {
    json!({
        "collectionsCreated": s.collections_created,
        "modesAdded": s.modes_added,
        "variables": {"created": s.variables_created, "updated": s.variables_updated, "unchanged": s.variables_unchanged},
        "styles": {"created": s.styles_created, "updated": s.styles_updated, "unchanged": s.styles_unchanged},
        "skipped": s.skipped,
        "message": s.text(),
    })
}

fn on_event(e: Event) -> Result<Value> {
    let saved = setting(CONFIG_KEY).and_then(|v| serde_json::from_value::<Config>(v).ok()).map(Config::normalized);
    let config = saved.unwrap_or_else(load_config);
    let section = e.values["section"].as_str().unwrap_or("Overview").to_string();
    match e.control.as_str() {
        "close" => {
            close_ui();
            Ok(Value::Null)
        }
        "generate" => {
            // Take whatever is in the fields now, in case the last edit hasn't been seen yet.
            let config = panel::merge(config, "generate", &e.values);
            let s = generate(&config)?;
            notify(&s.text());
            show_ui(&panel::spec(&config, &section, &e.values, Some(&s.text())))?;
            Ok(summary_json(&s))
        }
        "reset" => {
            let config = panel::reset(config, &section);
            save_setting(&config)?;
            show_ui(&panel::spec(&config, &section, &e.values, None))?;
            Ok(Value::Null)
        }
        "section" | "role" => {
            show_ui(&panel::spec(&config, &section, &e.values, None))?;
            Ok(Value::Null)
        }
        control => {
            let config = panel::merge(config, control, &e.values);
            save_setting(&config)?;
            show_ui(&panel::spec(&config, &section, &e.values, None))?;
            Ok(Value::Null)
        }
    }
}

fn run(input: Input) -> Result<Value> {
    if let Some(e) = input.event {
        return on_event(e);
    }
    let config = load_config();
    match input.command.as_str() {
        "generate" => {
            let s = generate(&config)?;
            notify(&s.text());
            Ok(summary_json(&s))
        }
        _ => {
            // Panel edits start from the document's config.
            save_setting(&config)?;
            show_ui(&panel::spec(&config, "Overview", &Value::Null, None))?;
            Ok(Value::Null)
        }
    }
}

entry!(run);
