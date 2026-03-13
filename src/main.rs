use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use cosmic_text::{Attrs, Family, FontSystem, Metrics, Shaping, SwashCache};
use scraper::{Html, Node};
use taffy::prelude::*;

/// Computed style for a node, extracted from inline CSS.
#[derive(Debug, Clone, Default)]
struct ComputedStyle {
    display_none: bool,
    width_px: Option<f32>,
    height_px: Option<f32>,
    width_pct: Option<f32>,
    height_pct: Option<f32>,
    max_width_px: Option<f32>,
    padding_top: Option<f32>,
    padding_bottom: Option<f32>,
    padding_left: Option<f32>,
    padding_right: Option<f32>,
    margin_top: Option<f32>,
    margin_bottom: Option<f32>,
    margin_left: Option<f32>,
    margin_right: Option<f32>,
    background_color: Option<(u8, u8, u8, u8)>,
    color: Option<(u8, u8, u8, u8)>,
    font_size: Option<f32>,
    font_family: Option<String>,
    font_weight: Option<u16>,
    font_style_italic: bool,
    border_top: Option<f32>,
    border_bottom: Option<f32>,
    line_height: Option<f32>,
    text_align: Option<TextAlign>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TextAlign {
    Left,
    Center,
    Right,
}

fn parse_css_value_px(val: &str) -> Option<f32> {
    let val = val.trim();
    if val == "0" {
        return Some(0.0);
    }
    if let Some(px) = val.strip_suffix("px") {
        return px.trim().parse().ok();
    }
    if let Some(pt) = val.strip_suffix("pt") {
        return pt.trim().parse::<f32>().ok().map(|v| v * 4.0 / 3.0);
    }
    if let Some(em) = val.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    val.parse().ok()
}

fn parse_css_value_pct(val: &str) -> Option<f32> {
    val.trim()
        .strip_suffix('%')
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map(|v| v / 100.0)
}

fn parse_css_color(val: &str) -> Option<(u8, u8, u8, u8)> {
    let val = val.trim();
    if let Some(hex) = val.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b, 255));
        }
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some((r, g, b, 255));
        }
        return None;
    }
    if val.starts_with("rgb") {
        let inner = val
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            let a = if parts.len() > 3 {
                (parts[3].trim().parse::<f32>().unwrap_or(1.0) * 255.0) as u8
            } else {
                255
            };
            return Some((r, g, b, a));
        }
    }
    match val.to_lowercase().as_str() {
        "white" => Some((255, 255, 255, 255)),
        "black" => Some((0, 0, 0, 255)),
        "red" => Some((255, 0, 0, 255)),
        "green" => Some((0, 128, 0, 255)),
        "blue" => Some((0, 0, 255, 255)),
        "gray" | "grey" => Some((128, 128, 128, 255)),
        "transparent" => Some((0, 0, 0, 0)),
        _ => None,
    }
}

fn parse_inline_style(css: &str) -> ComputedStyle {
    let mut style = ComputedStyle::default();

    for decl in css.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let Some((prop, val)) = decl.split_once(':') else {
            continue;
        };
        let prop = prop.trim().to_lowercase();
        let val = val.trim().trim_end_matches("!important").trim();

        match prop.as_str() {
            "display" => {
                if val.eq_ignore_ascii_case("none") {
                    style.display_none = true;
                }
            }
            "width" => {
                if let Some(pct) = parse_css_value_pct(val) {
                    style.width_pct = Some(pct);
                } else if let Some(px) = parse_css_value_px(val) {
                    style.width_px = Some(px);
                }
            }
            "height" => {
                if let Some(pct) = parse_css_value_pct(val) {
                    style.height_pct = Some(pct);
                } else if let Some(px) = parse_css_value_px(val) {
                    style.height_px = Some(px);
                }
            }
            "max-width" => {
                style.max_width_px = parse_css_value_px(val);
            }
            "padding" => {
                let parts: Vec<&str> = val.split_whitespace().collect();
                match parts.len() {
                    1 => {
                        let v = parse_css_value_px(parts[0]);
                        style.padding_top = v;
                        style.padding_bottom = v;
                        style.padding_left = v;
                        style.padding_right = v;
                    }
                    2 => {
                        let v = parse_css_value_px(parts[0]);
                        let h = parse_css_value_px(parts[1]);
                        style.padding_top = v;
                        style.padding_bottom = v;
                        style.padding_left = h;
                        style.padding_right = h;
                    }
                    3 => {
                        style.padding_top = parse_css_value_px(parts[0]);
                        style.padding_left = parse_css_value_px(parts[1]);
                        style.padding_right = parse_css_value_px(parts[1]);
                        style.padding_bottom = parse_css_value_px(parts[2]);
                    }
                    4 => {
                        style.padding_top = parse_css_value_px(parts[0]);
                        style.padding_right = parse_css_value_px(parts[1]);
                        style.padding_bottom = parse_css_value_px(parts[2]);
                        style.padding_left = parse_css_value_px(parts[3]);
                    }
                    _ => {}
                }
            }
            "padding-top" => style.padding_top = parse_css_value_px(val),
            "padding-bottom" => style.padding_bottom = parse_css_value_px(val),
            "padding-left" => style.padding_left = parse_css_value_px(val),
            "padding-right" => style.padding_right = parse_css_value_px(val),
            "margin" => {
                let parts: Vec<&str> = val.split_whitespace().collect();
                match parts.len() {
                    1 => {
                        let v = parse_css_value_px(parts[0]);
                        style.margin_top = v;
                        style.margin_bottom = v;
                        style.margin_left = v;
                        style.margin_right = v;
                    }
                    2 => {
                        let v = parse_css_value_px(parts[0]);
                        let h = parse_css_value_px(parts[1]);
                        style.margin_top = v;
                        style.margin_bottom = v;
                        style.margin_left = h;
                        style.margin_right = h;
                    }
                    4 => {
                        style.margin_top = parse_css_value_px(parts[0]);
                        style.margin_right = parse_css_value_px(parts[1]);
                        style.margin_bottom = parse_css_value_px(parts[2]);
                        style.margin_left = parse_css_value_px(parts[3]);
                    }
                    _ => {}
                }
            }
            "margin-top" => style.margin_top = parse_css_value_px(val),
            "margin-bottom" => style.margin_bottom = parse_css_value_px(val),
            "margin-left" => style.margin_left = parse_css_value_px(val),
            "margin-right" => style.margin_right = parse_css_value_px(val),
            "background-color" | "background" => {
                style.background_color = parse_css_color(val);
            }
            "color" => {
                style.color = parse_css_color(val);
            }
            "font-size" => {
                style.font_size = parse_css_value_px(val);
            }
            "font-family" => {
                // Take the first family, strip quotes
                if let Some(first) = val.split(',').next() {
                    let family = first.trim().trim_matches(|c| c == '\'' || c == '"');
                    style.font_family = Some(family.to_string());
                }
            }
            "font-weight" => {
                style.font_weight = match val.to_lowercase().as_str() {
                    "bold" => Some(700),
                    "normal" => Some(400),
                    "lighter" => Some(300),
                    "bolder" => Some(800),
                    _ => val.parse().ok(),
                };
            }
            "font-style" => {
                if val.eq_ignore_ascii_case("italic") || val.eq_ignore_ascii_case("oblique") {
                    style.font_style_italic = true;
                }
            }
            "line-height" => {
                style.line_height = parse_css_value_px(val);
            }
            "text-align" => {
                style.text_align = match val.to_lowercase().as_str() {
                    "center" => Some(TextAlign::Center),
                    "right" => Some(TextAlign::Right),
                    "left" | "start" => Some(TextAlign::Left),
                    _ => None,
                };
            }
            "border-top" => {
                if val == "0" || val == "none" {
                    style.border_top = Some(0.0);
                } else if let Some(first) = val.split_whitespace().next() {
                    style.border_top = parse_css_value_px(first);
                }
            }
            "border-bottom" => {
                if val == "0" || val == "none" {
                    style.border_bottom = Some(0.0);
                } else if let Some(first) = val.split_whitespace().next() {
                    style.border_bottom = parse_css_value_px(first);
                }
            }
            _ => {}
        }
    }

    style
}

/// Apply HTML attributes commonly used in email HTML (bgcolor, width, align, color).
fn apply_html_attrs(mut style: ComputedStyle, el: &scraper::node::Element) -> ComputedStyle {
    if let Some(bgcolor) = el.attr("bgcolor") {
        if style.background_color.is_none() {
            style.background_color = parse_css_color(bgcolor);
        }
    }
    if let Some(color) = el.attr("color") {
        if style.color.is_none() {
            style.color = parse_css_color(color);
        }
    }
    if let Some(width) = el.attr("width") {
        if style.width_px.is_none() && style.width_pct.is_none() {
            if let Some(pct) = parse_css_value_pct(width) {
                style.width_pct = Some(pct);
            } else if let Some(px) = parse_css_value_px(width) {
                style.width_px = Some(px);
            }
        }
    }
    if let Some(height) = el.attr("height") {
        if style.height_px.is_none() && style.height_pct.is_none() {
            style.height_px = parse_css_value_px(height);
        }
    }
    // cellpadding on table elements
    if let Some(cellpadding) = el.attr("cellpadding") {
        if style.padding_top.is_none() {
            if let Some(v) = parse_css_value_px(cellpadding) {
                style.padding_top = Some(v);
                style.padding_bottom = Some(v);
                style.padding_left = Some(v);
                style.padding_right = Some(v);
            }
        }
    }
    style
}

/// Per-node rendering data.
#[derive(Debug, Clone)]
struct NodeData {
    background_color: Option<(u8, u8, u8, u8)>,
    text: Option<String>,
    text_color: (u8, u8, u8, u8),
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_italic: bool,
    border_top: Option<f32>,
    border_bottom: Option<f32>,
    is_hr: bool,
    text_align: TextAlign,
}

impl Default for NodeData {
    fn default() -> Self {
        Self {
            background_color: None,
            text: None,
            text_color: (0, 0, 0, 255),
            font_size: 14.0,
            font_family: "sans-serif".to_string(),
            font_weight: 400,
            font_italic: false,
            border_top: None,
            border_bottom: None,
            is_hr: false,
            text_align: TextAlign::Left,
        }
    }
}

// Shared font system for text measurement during tree building.
thread_local! {
    static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
}

fn measure_text_width(text: &str, font_size: f32, family: &str, weight: u16, italic: bool) -> f32 {
    let font_size = font_size.max(1.0);
    FONT_SYSTEM.with(|fs| {
        let mut fs = fs.borrow_mut();
        let line_height = (font_size * 1.4).ceil().max(1.0);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        buffer.set_size(&mut fs, Some(f32::MAX), Some(line_height));

        let cosmic_family = match family {
            "serif" => Family::Serif,
            "sans-serif" | "sans serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };
        let cosmic_weight = cosmic_text::Weight(weight);
        let cosmic_style = if italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        };

        let attrs = Attrs::new()
            .family(cosmic_family)
            .weight(cosmic_weight)
            .style(cosmic_style);
        buffer.set_text(&mut fs, text, &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(&mut fs, false);

        buffer.layout_runs().map(|run| run.line_w).sum::<f32>()
    })
}

/// Extract CSS rules from <style> blocks. Returns a map of selector -> declarations.
fn extract_style_rules(document: &Html) -> HashMap<String, String> {
    use scraper::Selector;
    let mut rules = HashMap::new();
    let style_sel = Selector::parse("style").unwrap();
    for style_el in document.select(&style_sel) {
        let css_text = style_el.text().collect::<String>();
        parse_css_rules(&css_text, &mut rules);
    }
    rules
}

fn parse_css_rules(css: &str, rules: &mut HashMap<String, String>) {
    let mut rest = css.trim();
    while !rest.is_empty() {
        // Skip whitespace and comments
        rest = rest.trim_start();
        if rest.starts_with("/*") {
            if let Some(end) = rest.find("*/") {
                rest = &rest[end + 2..];
                continue;
            }
            break;
        }
        // Handle @media blocks — parse the inner rules
        if rest.starts_with("@media") {
            if let Some(brace) = rest.find('{') {
                let inner = &rest[brace + 1..];
                // Find matching closing brace (handle nesting)
                let mut depth = 1;
                let mut end = 0;
                for (i, ch) in inner.char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth == 0 {
                    // Parse inner rules (media queries are mostly for mobile — include them all)
                    parse_css_rules(&inner[..end], rules);
                    rest = &inner[end + 1..];
                } else {
                    break;
                }
                continue;
            }
            break;
        }
        // Skip other @-rules (charset, import, etc.)
        if rest.starts_with('@') {
            if let Some(semi) = rest.find(';') {
                rest = &rest[semi + 1..];
                continue;
            }
            if let Some(brace) = rest.find('{') {
                let inner = &rest[brace + 1..];
                if let Some(close) = inner.find('}') {
                    rest = &inner[close + 1..];
                    continue;
                }
            }
            break;
        }
        // Regular rule: selector { declarations }
        if let Some(brace_start) = rest.find('{') {
            let selector = rest[..brace_start].trim();
            let after_brace = &rest[brace_start + 1..];
            if let Some(brace_end) = after_brace.find('}') {
                let declarations = after_brace[..brace_end].trim();
                for sel in selector.split(',') {
                    let sel = sel.trim();
                    if !sel.is_empty() {
                        rules
                            .entry(sel.to_string())
                            .and_modify(|existing: &mut String| {
                                existing.push(';');
                                existing.push_str(declarations);
                            })
                            .or_insert_with(|| declarations.to_string());
                    }
                }
                rest = &after_brace[brace_end + 1..];
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

/// Check if a simple selector (no spaces) matches an element.
fn selector_part_matches(sel: &str, el: &scraper::node::Element) -> bool {
    let sel = sel.trim();
    if sel == "*" {
        return true;
    }

    // Split compound selector like "table.main" or "td.content"
    // into tag + class/id parts
    let mut rest = sel;
    let mut required_tag = None;
    let mut required_classes = Vec::new();
    let mut required_id = None;

    // Extract tag name (anything before first . or #)
    if let Some(pos) = rest.find(['.', '#']) {
        let tag = &rest[..pos];
        if !tag.is_empty() {
            required_tag = Some(tag);
        }
        rest = &rest[pos..];
    } else if !rest.starts_with('.') && !rest.starts_with('#') {
        required_tag = Some(rest);
        rest = "";
    }

    // Extract classes and ids
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix('.') {
            let end = stripped.find(['.', '#']).unwrap_or(stripped.len());
            required_classes.push(&stripped[..end]);
            rest = &stripped[end..];
        } else if let Some(stripped) = rest.strip_prefix('#') {
            let end = stripped.find(['.', '#']).unwrap_or(stripped.len());
            required_id = Some(&stripped[..end]);
            rest = &stripped[end..];
        } else {
            break;
        }
    }

    if let Some(tag) = required_tag {
        if el.name() != tag {
            return false;
        }
    }
    if let Some(id) = required_id {
        if el.attr("id") != Some(id) {
            return false;
        }
    }
    if !required_classes.is_empty() {
        let el_classes: Vec<&str> = el
            .attr("class")
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();
        for cls in &required_classes {
            if !el_classes.contains(cls) {
                return false;
            }
        }
    }

    true
}

/// Pre-indexed style rules for fast matching.
struct StyleIndex {
    /// Rules keyed by tag name
    by_tag: HashMap<String, String>,
    /// Rules keyed by class name
    by_class: HashMap<String, String>,
    /// Rules keyed by id
    by_id: HashMap<String, String>,
    /// Compound selectors (tag.class, .class1.class2, etc.)
    compound: Vec<(String, String)>,
}

impl StyleIndex {
    fn from_rules(rules: &HashMap<String, String>) -> Self {
        let mut by_tag = HashMap::new();
        let mut by_class = HashMap::new();
        let mut by_id = HashMap::new();
        let mut compound = Vec::new();

        for (selector, decls) in rules {
            // Only match single-part selectors
            if selector.contains(char::is_whitespace)
                || selector.contains(':')
                || selector.contains('[')
                || selector.contains('>')
            {
                continue;
            }

            let sel = selector.trim();
            if sel == "*" {
                // Skip universal selector
                continue;
            }

            // Simple selectors: just a tag, just a class, just an id
            if !sel.contains('.') && !sel.contains('#') {
                by_tag
                    .entry(sel.to_string())
                    .and_modify(|e: &mut String| {
                        e.push(';');
                        e.push_str(decls);
                    })
                    .or_insert_with(|| decls.clone());
            } else if sel.starts_with('.') && !sel[1..].contains('.') && !sel.contains('#') {
                by_class
                    .entry(sel[1..].to_string())
                    .and_modify(|e: &mut String| {
                        e.push(';');
                        e.push_str(decls);
                    })
                    .or_insert_with(|| decls.clone());
            } else if sel.starts_with('#') && !sel.contains('.') {
                by_id
                    .entry(sel[1..].to_string())
                    .and_modify(|e: &mut String| {
                        e.push(';');
                        e.push_str(decls);
                    })
                    .or_insert_with(|| decls.clone());
            } else {
                compound.push((sel.to_string(), decls.clone()));
            }
        }

        Self {
            by_tag,
            by_class,
            by_id,
            compound,
        }
    }

    fn match_element(&self, el: &scraper::node::Element) -> String {
        let mut matched = String::new();

        // Tag match
        if let Some(decls) = self.by_tag.get(el.name()) {
            matched.push_str(decls);
            matched.push(';');
        }

        // Class matches
        if let Some(classes) = el.attr("class") {
            for class in classes.split_whitespace() {
                if let Some(decls) = self.by_class.get(class) {
                    matched.push_str(decls);
                    matched.push(';');
                }
            }
        }

        // Id match
        if let Some(id) = el.attr("id") {
            if let Some(decls) = self.by_id.get(id) {
                matched.push_str(decls);
                matched.push(';');
            }
        }

        // Compound selectors (slow path, but typically few)
        for (sel, decls) in &self.compound {
            if selector_part_matches(sel, el) {
                matched.push_str(decls);
                matched.push(';');
            }
        }

        matched
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: rt-pipeline <html-file>");
        std::process::exit(1);
    });
    let html_str = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });

    // Phase 1: Parse HTML
    let t0 = Instant::now();
    let document = Html::parse_document(&html_str);
    let parse_time = t0.elapsed();

    // Extract <style> block rules and build index
    let style_rules = extract_style_rules(&document);
    let style_index = StyleIndex::from_rules(&style_rules);
    if !style_rules.is_empty() {
        println!("Style rules extracted: {} selectors", style_rules.len());
    }

    // Count
    let mut element_count = 0;
    let mut text_count = 0;
    let mut table_count = 0;
    let mut inline_style_count = 0;
    for node in document.tree.nodes() {
        match node.value() {
            Node::Element(el) => {
                element_count += 1;
                if el.name() == "table" {
                    table_count += 1;
                }
                if el.attr("style").is_some() {
                    inline_style_count += 1;
                }
            }
            Node::Text(_) => text_count += 1,
            _ => {}
        }
    }

    println!("Parsed: {element_count} elements, {text_count} text nodes, {table_count} tables");
    println!("Inline styles: {inline_style_count}");

    // Warm up font system (one-time cost, amortized across emails in production)
    let t_font = Instant::now();
    FONT_SYSTEM.with(|fs| {
        let _ = fs.borrow();
    });
    let font_init_time = t_font.elapsed();

    // Phase 2+3: Build taffy layout tree with CSS + text measurement
    let t1 = Instant::now();
    let mut taffy = TaffyTree::<()>::new();
    let mut node_data: HashMap<taffy::NodeId, NodeData> = HashMap::new();
    let inherited = InheritedStyle::default();
    let root_el = document.root_element();
    let root = build_node(
        root_el.id(),
        &document.tree,
        &mut taffy,
        &mut node_data,
        &inherited,
        &style_index,
    );
    let tree_build_time = t1.elapsed();

    // Compute layout
    let t2 = Instant::now();
    let viewport = Size {
        width: AvailableSpace::Definite(800.0),
        height: AvailableSpace::MaxContent,
    };
    taffy.compute_layout(root, viewport).unwrap();
    let layout_time = t2.elapsed();

    let layout = taffy.layout(root).unwrap();
    println!(
        "Layout: {}x{} (root)",
        layout.size.width, layout.size.height
    );

    // Phase 4: Render to tiny-skia with actual text
    let width = layout.size.width.ceil() as u32;
    let height = layout.size.height.ceil().min(10000.0) as u32;
    if width == 0 || height == 0 {
        eprintln!("Layout produced zero-size output");
        return;
    }

    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1)).unwrap();
    pixmap.fill(tiny_skia::Color::WHITE);

    let t3 = Instant::now();
    let mut swash_cache = SwashCache::new();
    render_node(
        &taffy,
        &node_data,
        root,
        &mut pixmap,
        &mut swash_cache,
        0.0,
        0.0,
    );
    let render_time = t3.elapsed();

    let t4 = Instant::now();
    let out_path = path.replace(".html", "_pipeline.png");
    pixmap.save_png(&out_path).unwrap();
    let png_time = t4.elapsed();

    println!("Saved to {out_path}");
    println!("--- Timing ---");
    println!("  Font init:    {:?} (one-time)", font_init_time);
    println!("  HTML parse:   {:?}", parse_time);
    println!("  Tree+CSS:     {:?}", tree_build_time);
    println!("  Layout:       {:?}", layout_time);
    println!("  Render:       {:?}", render_time);
    println!("  PNG encode:   {:?}", png_time);
    let pipeline_total = parse_time + tree_build_time + layout_time + render_time;
    println!("  Total (no PNG): {:?}", pipeline_total);
    println!(
        "  Total (warm):   {:?} (excludes font init)",
        pipeline_total
    );
}

/// Inherited CSS properties that cascade down the tree.
#[derive(Debug, Clone)]
struct InheritedStyle {
    color: (u8, u8, u8, u8),
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_italic: bool,
    text_align: TextAlign,
}

impl Default for InheritedStyle {
    fn default() -> Self {
        Self {
            color: (0, 0, 0, 255),
            font_size: 14.0,
            font_family: "sans-serif".to_string(),
            font_weight: 400,
            font_italic: false,
            text_align: TextAlign::Left,
        }
    }
}

impl InheritedStyle {
    fn with_overrides(&self, css: &ComputedStyle, tag: &str) -> Self {
        let mut out = self.clone();
        if let Some(c) = css.color {
            out.color = c;
        }
        if let Some(fs) = css.font_size {
            out.font_size = fs;
        }
        if let Some(ref ff) = css.font_family {
            out.font_family = ff.clone();
        }
        if let Some(fw) = css.font_weight {
            out.font_weight = fw;
        }
        if css.font_style_italic {
            out.font_italic = true;
        }
        if let Some(ta) = css.text_align {
            out.text_align = ta;
        }
        // Tag-based defaults
        match tag {
            "a" => {
                if out.color == (0, 0, 0, 255) {
                    out.color = (0, 102, 204, 255); // link blue
                }
            }
            "b" | "strong" => out.font_weight = out.font_weight.max(700),
            "i" | "em" => out.font_italic = true,
            "h1" => {
                out.font_size = out.font_size.max(32.0);
                out.font_weight = out.font_weight.max(700);
            }
            "h2" => {
                out.font_size = out.font_size.max(24.0);
                out.font_weight = out.font_weight.max(700);
            }
            "h3" => {
                out.font_size = out.font_size.max(18.0);
                out.font_weight = out.font_weight.max(700);
            }
            "h4" => {
                out.font_size = out.font_size.max(16.0);
                out.font_weight = out.font_weight.max(700);
            }
            "small" => out.font_size = (out.font_size * 0.83).max(10.0),
            "code" | "pre" | "kbd" | "samp" => {
                out.font_family = "monospace".to_string();
            }
            _ => {}
        }
        out
    }
}

fn build_node(
    node_id: ego_tree::NodeId,
    tree: &ego_tree::Tree<Node>,
    taffy: &mut TaffyTree<()>,
    node_data: &mut HashMap<taffy::NodeId, NodeData>,
    inherited: &InheritedStyle,
    style_index: &StyleIndex,
) -> taffy::NodeId {
    let node_ref = tree.get(node_id).unwrap();

    match node_ref.value() {
        Node::Element(el) => {
            let tag = el.name();
            // Merge style block rules + inline styles (inline wins via cascade order)
            let rule_css = style_index.match_element(el);
            let inline_css = el.attr("style").unwrap_or("");
            let merged = if !rule_css.is_empty() && !inline_css.is_empty() {
                format!("{rule_css};{inline_css}")
            } else if !rule_css.is_empty() {
                rule_css
            } else {
                inline_css.to_string()
            };
            let computed = if !merged.is_empty() {
                parse_inline_style(&merged)
            } else {
                ComputedStyle::default()
            };

            // Also check HTML attributes for bgcolor, color, width (common in email HTML)
            let computed = apply_html_attrs(computed, el);

            let child_inherited = inherited.with_overrides(&computed, tag);
            let style = element_style(tag, el, &computed);
            // Check align attr for text-align (common in email HTML)
            let text_align = if computed.text_align.is_some() {
                child_inherited.text_align
            } else if let Some(align) = el.attr("align") {
                match align.to_lowercase().as_str() {
                    "center" => TextAlign::Center,
                    "right" => TextAlign::Right,
                    _ => child_inherited.text_align,
                }
            } else {
                child_inherited.text_align
            };

            let data = NodeData {
                background_color: computed.background_color,
                text: None,
                text_color: child_inherited.color,
                font_size: child_inherited.font_size,
                font_family: child_inherited.font_family.clone(),
                font_weight: child_inherited.font_weight,
                font_italic: child_inherited.font_italic,
                border_top: computed.border_top,
                border_bottom: computed.border_bottom,
                is_hr: tag == "hr",
                text_align,
            };

            let children: Vec<taffy::NodeId> = node_ref
                .children()
                .map(|child| {
                    build_node(child.id(), tree, taffy, node_data, &child_inherited, style_index)
                })
                .collect();

            let id = taffy.new_with_children(style, &children).unwrap();
            node_data.insert(id, data);
            id
        }
        Node::Text(text) => {
            let text_str = text.text.trim();
            if text_str.is_empty() {
                let id = taffy.new_leaf(Style::default()).unwrap();
                node_data.insert(id, NodeData::default());
                return id;
            }

            // Measure text with cosmic-text for accurate layout
            let fs = inherited.font_size.max(1.0);
            let text_width = measure_text_width(
                text_str,
                fs,
                &inherited.font_family,
                inherited.font_weight,
                inherited.font_italic,
            );
            let line_height = (fs * 1.4).ceil();
            let num_lines = (text_width / 780.0).ceil().max(1.0);

            let style = Style {
                size: Size {
                    width: percent(1.0),
                    height: length(line_height * num_lines),
                },
                ..Default::default()
            };
            let data = NodeData {
                text: Some(text_str.to_string()),
                text_color: inherited.color,
                font_size: inherited.font_size,
                font_family: inherited.font_family.clone(),
                font_weight: inherited.font_weight,
                font_italic: inherited.font_italic,
                text_align: inherited.text_align,
                ..Default::default()
            };
            let id = taffy.new_leaf(style).unwrap();
            node_data.insert(id, data);
            id
        }
        _ => {
            let id = taffy.new_leaf(Style::default()).unwrap();
            node_data.insert(id, NodeData::default());
            id
        }
    }
}

fn element_style(tag: &str, el: &scraper::node::Element, css: &ComputedStyle) -> Style {
    let base = match tag {
        "html" | "body" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav"
        | "aside" | "form" | "fieldset" | "blockquote" | "pre" | "address" | "details"
        | "summary" | "figure" | "figcaption" | "dialog" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        },

        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            margin: Rect {
                top: length(8.0),
                bottom: length(8.0),
                left: auto(),
                right: auto(),
            },
            ..Default::default()
        },

        "p" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            margin: Rect {
                top: length(8.0),
                bottom: length(8.0),
                left: auto(),
                right: auto(),
            },
            ..Default::default()
        },

        "table" => Style {
            display: Display::Block,
            size: Size {
                width: percent_width_from_attr(el).unwrap_or(percent(1.0)),
                height: auto(),
            },
            ..Default::default()
        },
        "tr" => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        },
        "td" | "th" => Style {
            display: Display::Block,
            flex_grow: 1.0,
            padding: Rect {
                top: length(2.0),
                bottom: length(2.0),
                left: length(4.0),
                right: length(4.0),
            },
            ..Default::default()
        },
        "thead" | "tbody" | "tfoot" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        },

        "span" | "a" | "strong" | "em" | "b" | "i" | "u" | "small" | "big" | "sub" | "sup"
        | "code" | "kbd" | "samp" | "var" | "cite" | "abbr" | "mark" | "del" | "ins" | "s"
        | "q" | "dfn" | "ruby" | "rt" | "rp" | "bdi" | "bdo" | "wbr" | "time" | "data"
        | "output" | "font" | "center" => Style {
            display: Display::Block,
            ..Default::default()
        },

        "img" => {
            let w = el
                .attr("width")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(100.0);
            let h = el
                .attr("height")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(100.0);
            Style {
                size: Size {
                    width: length(w),
                    height: length(h),
                },
                ..Default::default()
            }
        }

        "ul" | "ol" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            padding: Rect {
                left: length(20.0),
                ..Rect::zero()
            },
            ..Default::default()
        },
        "li" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        },

        "br" => Style {
            size: Size {
                width: percent(1.0),
                height: length(16.0),
            },
            ..Default::default()
        },

        "hr" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: length(2.0),
            },
            margin: Rect {
                top: length(8.0),
                bottom: length(8.0),
                left: auto(),
                right: auto(),
            },
            ..Default::default()
        },

        "head" | "script" | "style" | "meta" | "link" | "title" | "noscript" => Style {
            display: Display::None,
            ..Default::default()
        },

        _ => Style {
            display: Display::Block,
            ..Default::default()
        },
    };

    let styled = apply_css_overrides(base, css);
    apply_align_attr(styled, el)
}

fn apply_align_attr(mut style: Style, el: &scraper::node::Element) -> Style {
    if let Some(align) = el.attr("align") {
        match align.to_lowercase().as_str() {
            "center" => {
                style.justify_content = Some(JustifyContent::Center);
                style.align_items = Some(AlignItems::Center);
            }
            "right" => {
                style.justify_content = Some(JustifyContent::End);
            }
            _ => {}
        }
    }
    if let Some(valign) = el.attr("valign") {
        match valign.to_lowercase().as_str() {
            "middle" => style.align_items = Some(AlignItems::Center),
            "bottom" => style.align_items = Some(AlignItems::End),
            "top" => style.align_items = Some(AlignItems::Start),
            _ => {}
        }
    }
    style
}

fn apply_css_overrides(mut style: Style, css: &ComputedStyle) -> Style {
    if css.display_none {
        style.display = Display::None;
        return style;
    }

    if let Some(w) = css.width_px {
        style.size.width = length(w);
    } else if let Some(pct) = css.width_pct {
        style.size.width = percent(pct);
    }

    if let Some(h) = css.height_px {
        style.size.height = length(h);
    } else if let Some(pct) = css.height_pct {
        style.size.height = percent(pct);
    }

    if let Some(mw) = css.max_width_px {
        style.max_size.width = length(mw);
    }

    if let Some(v) = css.padding_top {
        style.padding.top = LengthPercentage::length(v);
    }
    if let Some(v) = css.padding_bottom {
        style.padding.bottom = LengthPercentage::length(v);
    }
    if let Some(v) = css.padding_left {
        style.padding.left = LengthPercentage::length(v);
    }
    if let Some(v) = css.padding_right {
        style.padding.right = LengthPercentage::length(v);
    }

    if let Some(v) = css.margin_top {
        style.margin.top = LengthPercentageAuto::length(v);
    }
    if let Some(v) = css.margin_bottom {
        style.margin.bottom = LengthPercentageAuto::length(v);
    }
    if let Some(v) = css.margin_left {
        style.margin.left = LengthPercentageAuto::length(v);
    }
    if let Some(v) = css.margin_right {
        style.margin.right = LengthPercentageAuto::length(v);
    }

    if let Some(v) = css.border_top {
        style.border.top = LengthPercentage::length(v);
    }
    if let Some(v) = css.border_bottom {
        style.border.bottom = LengthPercentage::length(v);
    }

    style
}

fn percent_width_from_attr(el: &scraper::node::Element) -> Option<Dimension> {
    let w = el.attr("width")?;
    if let Some(pct) = w.strip_suffix('%') {
        pct.trim()
            .parse::<f32>()
            .ok()
            .map(|v| Dimension::percent(v / 100.0))
    } else {
        w.trim().parse::<f32>().ok().map(Dimension::length)
    }
}

fn render_node(
    taffy: &TaffyTree<()>,
    node_data: &HashMap<taffy::NodeId, NodeData>,
    node_id: taffy::NodeId,
    pixmap: &mut tiny_skia::Pixmap,
    swash_cache: &mut SwashCache,
    offset_x: f32,
    offset_y: f32,
) {
    let layout = taffy.layout(node_id).unwrap();
    let default_data = NodeData::default();
    let data = node_data.get(&node_id).unwrap_or(&default_data);
    let x = offset_x + layout.location.x;
    let y = offset_y + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;
    let pix_h = pixmap.height() as f32;

    if w > 0.0 && h > 0.0 && y < pix_h && y + h > 0.0 {
        // Draw background
        if let Some((r, g, b, a)) = data.background_color {
            if a > 0 {
                if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                    let paint = tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(
                            tiny_skia::Color::from_rgba8(r, g, b, a),
                        ),
                        anti_alias: false,
                        ..Default::default()
                    };
                    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                }
            }
        }

        // Draw <hr> as a gray line
        if data.is_hr {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, 1.0) {
                let paint = tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(
                        tiny_skia::Color::from_rgba8(180, 180, 180, 255),
                    ),
                    anti_alias: false,
                    ..Default::default()
                };
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }
        }

        // Draw borders (top and bottom for now)
        if let Some(border_w) = data.border_top {
            if border_w > 0.0 {
                if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, border_w) {
                    let paint = tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(
                            tiny_skia::Color::from_rgba8(200, 200, 200, 255),
                        ),
                        anti_alias: false,
                        ..Default::default()
                    };
                    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                }
            }
        }
        if let Some(border_w) = data.border_bottom {
            if border_w > 0.0 {
                if let Some(rect) = tiny_skia::Rect::from_xywh(x, y + h - border_w, w, border_w) {
                    let paint = tiny_skia::Paint {
                        shader: tiny_skia::Shader::SolidColor(
                            tiny_skia::Color::from_rgba8(200, 200, 200, 255),
                        ),
                        anti_alias: false,
                        ..Default::default()
                    };
                    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                }
            }
        }

        // Draw text
        if let Some(ref text) = data.text {
            draw_text(pixmap, swash_cache, text, x, y, w, data);
        }
    }

    for child_id in taffy.children(node_id).unwrap() {
        render_node(taffy, node_data, child_id, pixmap, swash_cache, x, y);
    }
}

fn draw_text(
    pixmap: &mut tiny_skia::Pixmap,
    swash_cache: &mut SwashCache,
    text: &str,
    x: f32,
    y: f32,
    container_width: f32,
    data: &NodeData,
) {
    FONT_SYSTEM.with(|fs| {
        let mut fs = fs.borrow_mut();
        let font_size = data.font_size.max(1.0);
        let line_height = (font_size * 1.4).ceil().max(1.0);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        // Use available width for text wrapping
        let available_width = (pixmap.width() as f32 - x).max(100.0);
        buffer.set_size(&mut fs, Some(available_width), Some(line_height * 20.0));

        let cosmic_family = match data.font_family.as_str() {
            "serif" => Family::Serif,
            "sans-serif" | "sans serif" => Family::SansSerif,
            "monospace" => Family::Monospace,
            name => Family::Name(name),
        };
        let attrs = Attrs::new()
            .family(cosmic_family)
            .weight(cosmic_text::Weight(data.font_weight))
            .style(if data.font_italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            });
        buffer.set_text(&mut fs, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fs, false);

        let (cr, cg, cb, _ca) = data.text_color;
        let pix_w = pixmap.width() as i32;
        let pix_h = pixmap.height() as i32;
        let draw_y = y as i32;

        for run in buffer.layout_runs() {
            // Calculate alignment offset
            let align_offset = match data.text_align {
                TextAlign::Center => ((container_width - run.line_w) / 2.0).max(0.0),
                TextAlign::Right => (container_width - run.line_w).max(0.0),
                TextAlign::Left => 0.0,
            };
            let draw_x = (x + align_offset) as i32;
            let baseline_y = run.line_y as i32;
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(image) = swash_cache.get_image_uncached(&mut fs, physical.cache_key) {
                    let gx = draw_x + physical.x + image.placement.left;
                    let gy = draw_y + baseline_y + physical.y - image.placement.top;

                    match image.content {
                        cosmic_text::SwashContent::Mask => {
                            let mut i = 0;
                            for off_y in 0..image.placement.height as i32 {
                                for off_x in 0..image.placement.width as i32 {
                                    let px = gx + off_x;
                                    let py = gy + off_y;
                                    if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                                        let alpha = image.data[i];
                                        if alpha > 0 {
                                            blend_pixel(
                                                pixmap.data_mut(),
                                                pix_w as u32,
                                                px as u32,
                                                py as u32,
                                                cr,
                                                cg,
                                                cb,
                                                alpha,
                                            );
                                        }
                                    }
                                    i += 1;
                                }
                            }
                        }
                        cosmic_text::SwashContent::Color => {
                            let mut i = 0;
                            for off_y in 0..image.placement.height as i32 {
                                for off_x in 0..image.placement.width as i32 {
                                    let px = gx + off_x;
                                    let py = gy + off_y;
                                    if px >= 0 && px < pix_w && py >= 0 && py < pix_h {
                                        let r = image.data[i];
                                        let g = image.data[i + 1];
                                        let b = image.data[i + 2];
                                        let a = image.data[i + 3];
                                        if a > 0 {
                                            blend_pixel(
                                                pixmap.data_mut(),
                                                pix_w as u32,
                                                px as u32,
                                                py as u32,
                                                r,
                                                g,
                                                b,
                                                a,
                                            );
                                        }
                                    }
                                    i += 4;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

fn blend_pixel(data: &mut [u8], width: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 {
        return;
    }
    let idx = (y as usize * width as usize + x as usize) * 4;
    if idx + 3 >= data.len() {
        return;
    }

    let sa = a as u32;
    let sr = (r as u32 * sa + 127) / 255;
    let sg = (g as u32 * sa + 127) / 255;
    let sb = (b as u32 * sa + 127) / 255;

    let dr = data[idx] as u32;
    let dg = data[idx + 1] as u32;
    let db = data[idx + 2] as u32;
    let da = data[idx + 3] as u32;

    let inv_sa = 255 - sa;
    data[idx] = (sr + (dr * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 1] = (sg + (dg * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 2] = (sb + (db * inv_sa + 127) / 255).min(255) as u8;
    data[idx + 3] = (sa + (da * inv_sa + 127) / 255).min(255) as u8;
}
