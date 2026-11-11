//! Latte: design-system generation and management for FigCraft.
//!
//! Latte turns a handful of choices (brand colours, a spacing unit, fonts and a type scale,
//! shadows) into native variables and styles, organised as four collections: **Primitives**
//! (tonal ramps), **Themes** (semantic colours, Light and Dark, aliasing Primitives), **Sizes**
//! (spacing, radii, borders) and **Typography** (families, sizes, line heights, weights), plus
//! text styles bound to Typography and effect styles. Running it again re-syncs: what's missing is
//! made and what changed is updated in place.
//!
//! Two ways in: the **panel** (Plugins ▸ Latte ▸ Open Latte…) and the **Latte board** widget
//! (Plugins ▸ Widgets ▸ Latte), a live preview on the canvas whose Edit settings button opens the
//! same panel and redraws the board with every change.

pub mod color;
pub mod config;
pub mod panel;
pub mod sync;
pub mod tokens;
pub mod widget;

use figcraft_plugin_sdk::{Event, Input, Result, Value, Widget, call, close_ui, entry, json, notify, render_widget, serde_json, set_setting, setting, show_ui};

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

fn to_value(c: &Config) -> Result<Value> {
    serde_json::to_value(c).map_err(|e| e.to_string())
}

fn save_setting(c: &Config) -> Result<()> {
    set_setting(CONFIG_KEY, &to_value(c)?)
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
    notify(&summary.text());
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

/// What a panel event leads to.
struct Outcome {
    config: Config,
    /// The panel to show next (None: it was closed).
    panel: Option<Value>,
    section: String,
    generated: Option<sync::Summary>,
}

/// A button or field of the panel, against `config`.
fn panel_event(config: Config, e: &Event) -> Result<Outcome> {
    let section = e.values["section"].as_str().unwrap_or("Overview").to_string();
    let show = |c: &Config, msg: Option<&str>| panel::spec(c, &section, &e.values, msg);
    Ok(match e.control.as_str() {
        "close" => Outcome { panel: None, config, section, generated: None },
        "generate" => {
            // Take whatever is in the fields now, in case the last edit hasn't been seen yet.
            let config = panel::merge(config, "generate", &e.values);
            let s = generate(&config)?;
            Outcome { panel: Some(show(&config, Some(&s.text()))), config, section, generated: Some(s) }
        }
        "reset" => {
            let config = panel::reset(config, &section);
            Outcome { panel: Some(show(&config, None)), config, section, generated: None }
        }
        "section" | "role" => Outcome { panel: Some(show(&config, None)), config, section, generated: None },
        control => {
            let config = panel::merge(config, control, &e.values);
            Outcome { panel: Some(show(&config, None)), config, section, generated: None }
        }
    })
}

fn show_or_close(panel: Option<&Value>) -> Result<()> {
    match panel {
        Some(p) => show_ui(p),
        None => {
            close_ui();
            Ok(())
        }
    }
}

/// The panel opened from the menu: its working config is the user setting.
fn on_menu_panel(e: &Event) -> Result<Value> {
    let saved = setting(CONFIG_KEY).and_then(|v| serde_json::from_value::<Config>(v).ok()).map(Config::normalized);
    let o = panel_event(saved.unwrap_or_else(load_config), e)?;
    save_setting(&o.config)?;
    show_or_close(o.panel.as_ref())?;
    Ok(o.generated.as_ref().map_or(Value::Null, summary_json))
}

/// The board's working config: kept in its state (so edits show before they're generated and
/// travel with the file), else the document's.
fn board_config(state: &Value) -> Config {
    serde_json::from_value::<Config>(state["config"].clone()).map(Config::normalized).unwrap_or_else(|_| load_config())
}

fn draw_board(config: &Config, tab: &str, last: &str) -> Result<()> {
    render_widget(&json!({
        "state": {"tab": tab, "config": to_value(config)?, "last": last},
        "root": widget::render(config, tab, last),
        "menu": widget::menu(),
    }))
}

/// A callback of the panel the board opened: edit the board's config and redraw it.
fn on_board_panel(w: &Widget, e: &Event) -> Result<Value> {
    let o = panel_event(board_config(&w.state), e)?;
    let tab = widget::tab_for(&o.section).unwrap_or_else(|| widget::tab_of(&w.state));
    let last = o.generated.as_ref().map_or_else(|| w.state["last"].as_str().unwrap_or_default().to_string(), sync::Summary::text);
    draw_board(&o.config, tab, &last)?;
    show_or_close(o.panel.as_ref())?;
    Ok(o.generated.as_ref().map_or(Value::Null, summary_json))
}

/// The board inserted, clicked, edited or given a menu item.
fn on_board(w: &Widget) -> Result<Value> {
    let mut config = board_config(&w.state);
    let mut tab = widget::tab_of(&w.state).to_string();
    let mut last = w.state["last"].as_str().unwrap_or_default().to_string();
    let mut output = Value::Null;
    let action = w.target.as_deref().or(w.menu.as_deref()).unwrap_or_default();
    if let Some(t) = action.strip_prefix("tab.") {
        tab = t.to_string();
    } else if action == "generate" {
        let s = generate(&config)?;
        last = s.text();
        output = summary_json(&s);
    } else if action == "edit" {
        show_ui(&panel::spec(&config, widget::section_for(&tab), &Value::Null, None))?;
    } else if let Some(field) = &w.field {
        // A colour's name or hex edited on the board: the same rules as the panel's Colours.
        let mut values = json!({"section": "Colours"});
        for (i, c) in config.colors.iter().enumerate() {
            values[format!("name{i}")] = json!(c.name);
            values[format!("hex{i}")] = json!(c.hex);
        }
        values["tint"] = json!(config.neutral_tint * 100.0);
        values["pin"] = json!(config.pin_base);
        let typed = w.value.as_str().unwrap_or_default().trim().to_string();
        if field.starts_with("hex") && color::Rgb::from_hex(&typed).is_none() {
            notify(&format!("“{typed}” isn't a hex colour (like #4F46E5)"));
        } else {
            values[field.as_str()] = json!(typed);
            config = panel::merge(config, field, &values);
        }
    }
    draw_board(&config, &tab, &last)?;
    Ok(output)
}

fn run(input: Input) -> Result<Value> {
    match (&input.widget, &input.event) {
        (Some(w), Some(e)) => return on_board_panel(w, e),
        (Some(w), None) => return on_board(w),
        (None, Some(e)) => return on_menu_panel(e),
        (None, None) => {}
    }
    let config = load_config();
    match input.command.as_str() {
        "generate" => Ok(summary_json(&generate(&config)?)),
        _ => {
            // Panel edits start from the document's config.
            save_setting(&config)?;
            show_ui(&panel::spec(&config, "Overview", &Value::Null, None))?;
            Ok(Value::Null)
        }
    }
}

entry!(run);
