pub mod css;
pub mod render;
pub mod style;
pub mod text;
pub mod tree;

use std::collections::HashMap;
use std::time::Instant;

use cosmic_text::SwashCache;
use scraper::{Html, Node};
use taffy::prelude::*;

use crate::css::TextAlign;
use crate::style::{InheritedStyle, StyleIndex};
use crate::tree::{build_nodes, measure_text_node, NodeData, TextMeasure};

/// Result of running the render pipeline.
pub struct RenderResult {
    pub pixmap: tiny_skia::Pixmap,
    pub json: String,
    pub json_entry_count: usize,
    pub element_count: usize,
    pub text_count: usize,
    pub table_count: usize,
    pub inline_style_count: usize,
    pub selector_count: usize,
    pub layout_width: f32,
    pub layout_height: f32,
    pub parse_time: std::time::Duration,
    pub font_init_time: std::time::Duration,
    pub tree_build_time: std::time::Duration,
    pub layout_time: std::time::Duration,
    pub render_time: std::time::Duration,
}

/// Run the full rendering pipeline: parse HTML, build layout tree, compute layout,
/// render to pixmap, and generate element layout JSON.
pub fn render(html_str: &str) -> RenderResult {
    // Phase 1: Parse HTML
    let t0 = Instant::now();
    let document = Html::parse_document(html_str);
    let parse_time = t0.elapsed();

    let style_index = StyleIndex::from_document(&document);
    let selector_count = style_index.selector_count();

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

    // Warm up font system
    let t_font = Instant::now();
    text::FONT_SYSTEM.with(|fs| {
        let _ = fs.borrow();
    });
    let font_init_time = t_font.elapsed();

    // Phase 2+3: Build taffy layout tree
    let t1 = Instant::now();
    let mut taffy = TaffyTree::<TextMeasure>::new();
    let mut node_data: HashMap<taffy::NodeId, NodeData> = HashMap::new();
    let inherited = InheritedStyle::default();
    let root_el = document.root_element();
    let roots = build_nodes(
        root_el.id(),
        &document.tree,
        &mut taffy,
        &mut node_data,
        &inherited,
        &style_index,
        None,
        0,
        "",
    );
    let root = roots.into_iter().next().expect("No root element found");
    let tree_build_time = t1.elapsed();

    // Compute layout
    let t2 = Instant::now();
    let viewport = Size {
        width: AvailableSpace::Definite(800.0),
        height: AvailableSpace::MaxContent,
    };
    taffy
        .compute_layout_with_measure(root, viewport, measure_text_node)
        .unwrap();
    let layout_time = t2.elapsed();

    let layout = taffy.layout(root).unwrap();
    let layout_width = layout.size.width;
    let layout_height = layout.size.height;

    // Phase 4: Render to tiny-skia
    let width = layout_width.ceil() as u32;
    let height = layout_height.ceil().min(10000.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1)).unwrap();
    pixmap.fill(tiny_skia::Color::WHITE);

    let t3 = Instant::now();
    let mut swash_cache = SwashCache::new();
    render::render_node(
        &taffy,
        &node_data,
        root,
        &mut pixmap,
        &mut swash_cache,
        0.0,
        0.0,
    );
    let render_time = t3.elapsed();

    // Generate JSON layout dump
    let mut entries = Vec::new();
    dump_json(&taffy, &node_data, root, 0.0, 0.0, &mut entries);
    let json_entry_count = entries.len();
    let json = format!("[\n{}\n]", entries.join(",\n"));

    RenderResult {
        pixmap,
        json,
        json_entry_count,
        element_count,
        text_count,
        table_count,
        inline_style_count,
        selector_count,
        layout_width,
        layout_height,
        parse_time,
        font_init_time,
        tree_build_time,
        layout_time,
        render_time,
    }
}

fn dump_json(
    taffy: &TaffyTree<TextMeasure>,
    node_data: &HashMap<taffy::NodeId, NodeData>,
    id: taffy::NodeId,
    offset_x: f32,
    offset_y: f32,
    out: &mut Vec<String>,
) {
    let l = taffy.layout(id).unwrap();
    let data = node_data.get(&id);
    let x = offset_x + l.location.x;
    let y = offset_y + l.location.y;
    let w = l.size.width;
    let h = l.size.height;

    if let Some(d) = data {
        if !d.tag.is_empty() {
            let bg = if let Some((r, g, b, a)) = d.background_color {
                format!("\"rgba({r}, {g}, {b}, {a})\"")
            } else {
                "null".to_string()
            };
            let ta = match d.text_align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            };
            let id_str = d.id_attr.as_deref().unwrap_or("");
            let class_str = d.classes.as_deref().unwrap_or("");
            out.push(format!(
                concat!(
                    "{{\"path\":\"{path}\",\"tag\":\"{tag}\",\"depth\":{depth},",
                    "\"id\":\"{id}\",\"classes\":\"{classes}\",",
                    "\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h},",
                    "\"bg\":{bg},",
                    "\"color\":\"rgb({cr}, {cg}, {cb})\",",
                    "\"fontSize\":{fs},\"fontWeight\":{fw},",
                    "\"paddingTop\":{pt},\"paddingRight\":{pr},",
                    "\"paddingBottom\":{pb},\"paddingLeft\":{pl},",
                    "\"marginTop\":{mt},\"marginRight\":{mr},",
                    "\"marginBottom\":{mb},\"marginLeft\":{ml},",
                    "\"textAlign\":\"{ta}\",",
                    "\"maxWidth\":{mw}}}"
                ),
                path = d.dom_path,
                tag = d.tag,
                depth = d.depth,
                id = id_str,
                classes = class_str,
                x = (x * 10.0).round() / 10.0,
                y = (y * 10.0).round() / 10.0,
                w = (w * 10.0).round() / 10.0,
                h = (h * 10.0).round() / 10.0,
                bg = bg,
                cr = d.text_color.0,
                cg = d.text_color.1,
                cb = d.text_color.2,
                fs = d.font_size,
                fw = d.font_weight,
                pt = d.padding.0,
                pr = d.padding.1,
                pb = d.padding.2,
                pl = d.padding.3,
                mt = d.margin.0,
                mr = d.margin.1,
                mb = d.margin.2,
                ml = d.margin.3,
                ta = ta,
                mw = d.max_width_px.map_or("null".to_string(), |v| v.to_string()),
            ));
        }
    }

    for child in taffy.children(id).unwrap() {
        dump_json(taffy, node_data, child, x, y, out);
    }
}
