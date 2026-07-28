use std::collections::HashMap;

use cosmic_text::{Metrics, Shaping};
use scraper::Node;
use taffy::prelude::*;

use crate::css::{
    apply_html_attrs, parse_css_value_px, parse_inline_style, ComputedStyle, TextAlign,
    VerticalAlign,
};
use crate::style::{element_style, is_inline_tag, InheritedStyle, StyleIndex};
use crate::text::{
    apply_text_transform, build_text_attrs, resolve_line_height, FONT_SYSTEM,
};

/// Per-node rendering data.
#[derive(Debug, Clone)]
pub(crate) struct NodeData {
    pub(crate) tag: String,
    pub(crate) dom_path: String,
    pub(crate) depth: usize,
    pub(crate) id_attr: Option<String>,
    pub(crate) classes: Option<String>,
    pub(crate) background_color: Option<(u8, u8, u8, u8)>,
    pub(crate) text: Option<String>,
    pub(crate) text_color: (u8, u8, u8, u8),
    pub(crate) font_size: f32,
    pub(crate) font_family: String,
    pub(crate) font_weight: u16,
    pub(crate) font_italic: bool,
    pub(crate) line_height: Option<f32>,
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) border_top: Option<f32>,
    pub(crate) border_bottom: Option<f32>,
    pub(crate) border_left: Option<f32>,
    pub(crate) border_right: Option<f32>,
    pub(crate) border_top_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_bottom_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_left_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_right_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_radius: Option<f32>,
    pub(crate) border_radius_pct: Option<f32>,
    pub(crate) is_hr: bool,
    pub(crate) is_img: bool,
    pub(crate) text_align: TextAlign,
    pub(crate) padding: (f32, f32, f32, f32),
    pub(crate) margin: (f32, f32, f32, f32),
    pub(crate) max_width_px: Option<f32>,
    pub(crate) display_inline_block: bool,
    pub(crate) vertical_align_middle: bool,
    pub(crate) rich_spans: Option<Vec<RichTextSpan>>,
}

impl Default for NodeData {
    fn default() -> Self {
        Self {
            tag: String::new(),
            dom_path: String::new(),
            depth: 0,
            id_attr: None,
            classes: None,
            background_color: None,
            text: None,
            text_color: (0, 0, 0, 255),
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            font_weight: 400,
            font_italic: false,
            line_height: None,
            letter_spacing: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            border_top_color: None,
            border_bottom_color: None,
            border_left_color: None,
            border_right_color: None,
            border_radius: None,
            border_radius_pct: None,
            is_hr: false,
            is_img: false,
            text_align: TextAlign::Left,
            padding: (0.0, 0.0, 0.0, 0.0),
            margin: (0.0, 0.0, 0.0, 0.0),
            max_width_px: None,
            display_inline_block: false,
            vertical_align_middle: false,
            rich_spans: None,
        }
    }
}

/// Context stored on taffy leaf nodes for dynamic text measurement.
#[derive(Debug, Clone)]
pub(crate) struct TextMeasure {
    pub(crate) text: String,
    pub(crate) font_size: f32,
    pub(crate) font_family: String,
    pub(crate) font_weight: u16,
    pub(crate) font_italic: bool,
    pub(crate) line_height: Option<f32>,
    pub(crate) white_space_nowrap: bool,
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) spans: Option<Vec<RichTextSpan>>,
}

/// A span of text with its own styling, used for rich text (inline elements).
#[derive(Debug, Clone)]
pub(crate) struct RichTextSpan {
    pub(crate) text: String,
    pub(crate) font_size: f32,
    pub(crate) font_family: String,
    pub(crate) font_weight: u16,
    pub(crate) font_italic: bool,
    pub(crate) color: (u8, u8, u8, u8),
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) underline: bool,
    pub(crate) line_through: bool,
}

/// Measure function called by taffy during layout to determine text node size.
pub(crate) fn measure_text_node(
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    _node_id: taffy::NodeId,
    context: Option<&mut TextMeasure>,
    _style: &Style,
) -> Size<f32> {
    let Some(ctx) = context else {
        return Size::ZERO;
    };

    let available_width = if ctx.white_space_nowrap {
        f32::MAX
    } else {
        known_dimensions
            .width
            .unwrap_or(match available_space.width {
                AvailableSpace::Definite(w) => w,
                AvailableSpace::MinContent => 1.0,
                AvailableSpace::MaxContent => f32::MAX,
            })
    };

    // Measure text wrapping at the available width
    let fs = ctx.font_size.max(1.0);

    FONT_SYSTEM.with(|font_sys| {
        let mut font_sys = font_sys.borrow_mut();
        let line_height =
            resolve_line_height(&font_sys, fs, ctx.line_height, &ctx.font_family);
        let metrics = Metrics::new(fs, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut font_sys, metrics);
        buffer.set_size(Some(available_width), Some(line_height * 100.0));

        if let Some(ref spans) = ctx.spans {
            let rich: Vec<(&str, cosmic_text::Attrs)> = spans
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let span_fs = s.font_size.max(1.0);
                    let span_lh =
                        resolve_line_height(&font_sys, span_fs, ctx.line_height, &s.font_family);
                    let mut attrs = build_text_attrs(
                        &s.font_family,
                        s.font_weight,
                        s.font_italic,
                        s.letter_spacing,
                        span_fs,
                    )
                    .metadata(i);
                    if (span_fs - fs).abs() > 0.1 {
                        attrs = attrs.metrics(Metrics::new(span_fs, span_lh));
                    }
                    (s.text.as_str(), attrs)
                })
                .collect();
            let default_attrs = build_text_attrs(
                &ctx.font_family,
                ctx.font_weight,
                ctx.font_italic,
                ctx.letter_spacing,
                fs,
            );
            buffer.set_rich_text(rich, &default_attrs, Shaping::Basic, None);
        } else {
            let attrs = build_text_attrs(
                &ctx.font_family,
                ctx.font_weight,
                ctx.font_italic,
                ctx.letter_spacing,
                fs,
            );
            buffer.set_text(&ctx.text, &attrs, Shaping::Basic, None);
        }
        buffer.shape_until_scroll(&mut font_sys, false);

        let num_lines = buffer.layout_runs().count().max(1) as f32;
        let text_width = buffer
            .layout_runs()
            .map(|r| r.line_w)
            .fold(0.0_f32, f32::max);

        Size {
            width: known_dimensions
                .width
                .unwrap_or(text_width.min(available_width)),
            height: known_dimensions
                .height
                .unwrap_or(line_height * num_lines),
        }
    })
}

#[allow(dead_code)]
pub(crate) fn measure_text_width(
    text: &str,
    font_size: f32,
    family: &str,
    weight: u16,
    italic: bool,
) -> f32 {
    let font_size = font_size.max(1.0);
    FONT_SYSTEM.with(|fs| {
        let mut fs = fs.borrow_mut();
        let line_height = (font_size * 1.4).ceil().max(1.0);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        buffer.set_size(Some(f32::MAX), Some(line_height));

        let attrs = build_text_attrs(family, weight, italic, None, font_size);
        buffer.set_text(text, &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(&mut fs, false);

        buffer.layout_runs().map(|run| run.line_w).sum::<f32>()
    })
}

/// Recursively collect text from inline children into rich text spans.
/// Returns None if any non-inline content is found (images, nested blocks, etc.)
/// signaling fallback to the current flex approach.
pub(crate) fn collect_inline_text(
    node_ref: ego_tree::NodeRef<'_, Node>,
    inherited: &InheritedStyle,
    style_index: &StyleIndex,
    spans: &mut Vec<RichTextSpan>,
) -> bool {
    for child in node_ref.children() {
        match child.value() {
            Node::Text(text) => {
                // Strip zero-width characters (U+200B ZWSP, U+FEFF): they are
                // not Unicode whitespace so they survive collapsing, and Ahem
                // has no glyph for them, so Basic shaping gives them a visible
                // fallback width - "vidar\u{200B}braut" wrapped onto an extra
                // line in the Steam account cells (gmail_steam_purchase).
                // Chrome renders them at zero width.
                let source: String = text
                    .text
                    .chars()
                    .filter(|c| !matches!(c, '\u{200B}' | '\u{FEFF}'))
                    .collect();
                // Normalize whitespace: collapse runs of whitespace to single spaces,
                // but preserve a single leading/trailing space if the original had one.
                let has_leading_space = source.starts_with(|c: char| c.is_whitespace());
                let has_trailing_space = source.ends_with(|c: char| c.is_whitespace());
                let mut collapsed: String =
                    source.split_whitespace().collect::<Vec<_>>().join(" ");
                // Whitespace after a <br> collapses away like whitespace at
                // block start: it is the leading space of the new line. The
                // indentation between "<br></strong>" and "</td>" otherwise
                // became a " " span after the "\n" span, adding a phantom
                // line to every Steam receipt title (gmail_steam_purchase)
                let after_break = spans.last().is_some_and(|s| s.text.ends_with('\n'));
                if collapsed.is_empty() {
                    if has_leading_space && !spans.is_empty() && !after_break {
                        collapsed = " ".to_string();
                    } else {
                        continue;
                    }
                } else {
                    // Only preserve leading space between spans, not at block start.
                    // Leading whitespace from HTML indentation must be stripped for the
                    // first text in a block, matching browser behavior. (text_decoration_test)
                    if has_leading_space && !spans.is_empty() && !after_break {
                        collapsed.insert(0, ' ');
                    }
                    if has_trailing_space {
                        collapsed.push(' ');
                    }
                }
                if collapsed.is_empty() {
                    continue;
                }
                let collapsed = apply_text_transform(&collapsed, inherited.text_transform);
                spans.push(RichTextSpan {
                    text: collapsed,
                    font_size: inherited.font_size,
                    font_family: inherited.font_family.clone(),
                    font_weight: inherited.font_weight,
                    font_italic: inherited.font_italic,
                    color: inherited.color,
                    letter_spacing: inherited.letter_spacing,
                    underline: inherited.text_decoration_underline,
                    line_through: inherited.text_decoration_line_through,
                });
            }
            Node::Element(el) => {
                let tag = el.name();
                if tag == "br" {
                    spans.push(RichTextSpan {
                        text: "\n".to_string(),
                        font_size: inherited.font_size,
                        font_family: inherited.font_family.clone(),
                        font_weight: inherited.font_weight,
                        font_italic: inherited.font_italic,
                        color: inherited.color,
                        letter_spacing: inherited.letter_spacing,
                        underline: false,
                        line_through: false,
                    });
                    continue;
                }
                if !is_inline_tag(tag) {
                    return false;
                }
                // Compute style overrides for this inline element
                let el_ref = scraper::ElementRef::wrap(child).unwrap();
                let rule_css = style_index.match_element_ref(&el_ref);
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
                let computed = apply_html_attrs(computed, el);
                // Bail if this inline element has visual styling that needs its own box
                if computed.background_color.is_some()
                    || computed.padding_top.is_some()
                    || computed.padding_bottom.is_some()
                    || computed.padding_left.is_some()
                    || computed.padding_right.is_some()
                    || computed.border_top.is_some()
                    || computed.border_bottom.is_some()
                    || computed.border_left.is_some()
                    || computed.border_right.is_some()
                    || computed.display_none
                    || computed.display_inline_block
                {
                    return false;
                }
                let child_inherited = inherited.with_overrides(&computed, tag);
                if !collect_inline_text(child, &child_inherited, style_index, spans) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Build taffy nodes for a DOM node. Returns a Vec because inline elements
/// are flattened - their children become direct children of the parent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_nodes(
    node_id: ego_tree::NodeId,
    tree: &ego_tree::Tree<Node>,
    taffy: &mut TaffyTree<TextMeasure>,
    node_data: &mut HashMap<taffy::NodeId, NodeData>,
    inherited: &InheritedStyle,
    style_index: &StyleIndex,
    cellpadding: Option<f32>,
    depth: usize,
    parent_path: &str,
) -> Vec<taffy::NodeId> {
    let node_ref = tree.get(node_id).unwrap();

    match node_ref.value() {
        Node::Element(el) => {
            let tag = el.name();

            // Compute sibling index (same-tag siblings before this one)
            let sib_idx = {
                let mut idx = 0;
                let mut sib = node_ref.prev_sibling();
                while let Some(s) = sib {
                    if let Node::Element(sib_el) = s.value()
                        && sib_el.name() == tag
                    {
                        idx += 1;
                    }
                    sib = s.prev_sibling();
                }
                idx
            };
            let node_path = if parent_path.is_empty() {
                tag.to_string()
            } else {
                format!("{parent_path}>{tag}[{sib_idx}]")
            };

            // Merge style block rules + inline styles (inline wins via cascade order)
            let el_ref = scraper::ElementRef::wrap(node_ref).unwrap();
            let rule_css = style_index.match_element_ref(&el_ref);
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

            let mut child_inherited = inherited.with_overrides(&computed, tag);

            let mut style = element_style(tag, el, &computed);

            // <br> height uses inherited font size for correct line spacing
            if tag == "br" {
                let br_fs = child_inherited.font_size.max(1.0);
                let br_lh = FONT_SYSTEM.with(|fs| {
                    let fs = fs.borrow();
                    resolve_line_height(
                        &fs,
                        br_fs,
                        child_inherited.line_height,
                        &child_inherited.font_family,
                    )
                });
                style.size.height = length(br_lh);
            }

            // Center block children with fixed pixel widths when parent has
            // align="center". In CSS, text-align:center only centers inline content;
            // block elements need margin:0 auto. HTML align="center" on <td> should
            // do both. Only apply when child has a fixed px width (not percentage or
            // auto, which would fill the container) and no explicit margins.
            // (header_test)
            if inherited.text_align == TextAlign::Center
                && style.display == Display::Block
                && !computed.margin_left_auto
                && !computed.margin_right_auto
                && computed.margin_left.is_none()
                && computed.margin_right.is_none()
                && computed.width_px.is_some() // fixed pixel width, not auto
                && computed.width_pct.is_none() // not percentage (would fill parent)
            {
                style.margin.left = LengthPercentageAuto::auto();
                style.margin.right = LengthPercentageAuto::auto();
            }

            // Check align attr for text-align on cells/blocks (common in email HTML).
            // On <table>, align="center" means center the table itself (handled by
            // apply_align_attr on the taffy style), NOT text-align for content.
            // (text_decoration_test)
            let text_align = if computed.text_align.is_some() {
                child_inherited.text_align
            } else if tag != "table" {
                if let Some(align) = el.attr("align") {
                    match align.to_lowercase().as_str() {
                        "center" => TextAlign::Center,
                        "right" => TextAlign::Right,
                        _ => child_inherited.text_align,
                    }
                } else {
                    child_inherited.text_align
                }
            } else {
                child_inherited.text_align
            };
            // Propagate text-align to children so direct text nodes get it
            child_inherited.text_align = text_align;

            let data = NodeData {
                tag: tag.to_string(),
                dom_path: node_path.clone(),
                depth,
                id_attr: el.attr("id").map(std::string::ToString::to_string),
                classes: el.attr("class").map(std::string::ToString::to_string),
                background_color: computed.background_color,
                text: None,
                text_color: child_inherited.color,
                font_size: child_inherited.font_size,
                font_family: child_inherited.font_family.clone(),
                font_weight: child_inherited.font_weight,
                font_italic: child_inherited.font_italic,
                line_height: child_inherited.line_height,
                letter_spacing: child_inherited.letter_spacing,
                border_top: computed.border_top,
                border_bottom: computed.border_bottom,
                border_left: computed.border_left,
                border_right: computed.border_right,
                border_top_color: computed.border_top_color,
                border_bottom_color: computed.border_bottom_color,
                border_left_color: computed.border_left_color,
                border_right_color: computed.border_right_color,
                border_radius: computed.border_radius,
                border_radius_pct: computed.border_radius_pct,
                is_hr: tag == "hr",
                is_img: tag == "img",
                text_align,
                padding: (
                    computed.padding_top.unwrap_or(0.0),
                    computed.padding_right.unwrap_or(0.0),
                    computed.padding_bottom.unwrap_or(0.0),
                    computed.padding_left.unwrap_or(0.0),
                ),
                margin: (
                    computed.margin_top.unwrap_or(0.0),
                    computed.margin_right.unwrap_or(0.0),
                    computed.margin_bottom.unwrap_or(0.0),
                    computed.margin_left.unwrap_or(0.0),
                ),
                max_width_px: computed.max_width_px,
                display_inline_block: computed.display_inline_block,
                vertical_align_middle: computed.vertical_align == Some(VerticalAlign::Middle),
                rich_spans: None,
            };

            // If this is a <table> with cellpadding, propagate to child cells
            let child_cellpadding = if tag == "table" {
                el.attr("cellpadding")
                    .and_then(parse_css_value_px)
                    .or(cellpadding)
            } else {
                cellpadding
            };

            // Apply cellpadding to td/th cells - only override sides without explicit CSS padding
            let mut style =
                if let Some(cp) = child_cellpadding.filter(|_| tag == "td" || tag == "th") {
                    let mut s = style;
                    if computed.padding_top.is_none() {
                        s.padding.top = length(cp);
                    }
                    if computed.padding_bottom.is_none() {
                        s.padding.bottom = length(cp);
                    }
                    if computed.padding_left.is_none() {
                        s.padding.left = length(cp);
                    }
                    if computed.padding_right.is_none() {
                        s.padding.right = length(cp);
                    }
                    s
                } else {
                    style
                };

            // Try rich text collection for block/cell elements with inline children
            if matches!(style.display, Display::Block | Display::TableCell) {
                let has_inline_children = node_ref.children().any(|child| match child.value() {
                    Node::Element(child_el) => {
                        is_inline_tag(child_el.name()) || child_el.name() == "br"
                    }
                    Node::Text(_) => false,
                    _ => false,
                });
                if has_inline_children {
                    let mut spans = Vec::new();
                    if collect_inline_text(
                        node_ref,
                        &child_inherited,
                        style_index,
                        &mut spans,
                    ) && !spans.is_empty()
                    {
                        // A trailing <br> must not add a phantom line: Chrome
                        // renders "text<br>" as one line ("text<br><br>" as
                        // two, "<br>" alone as one), but our trailing "\n"
                        // span makes cosmic-text emit an empty final line that
                        // measurement counts. Each <br> is its own span, so
                        // drop the last span iff it is exactly "\n" and not
                        // the block's only content. Steam receipt titles are
                        // all "<strong>Title<br></strong>", which made every
                        // line item one line-height too tall
                        // (gmail_steam_purchase)
                        if spans.len() > 1 && spans.last().is_some_and(|s| s.text == "\n") {
                            spans.pop();
                        }
                        // Success: create a single rich text leaf node
                        let full_text: String =
                            spans.iter().map(|s| s.text.as_str()).collect();
                        let text_ctx = TextMeasure {
                            text: full_text.clone(),
                            font_size: child_inherited.font_size,
                            font_family: child_inherited.font_family.clone(),
                            font_weight: child_inherited.font_weight,
                            font_italic: child_inherited.font_italic,
                            line_height: child_inherited.line_height,
                            white_space_nowrap: child_inherited.white_space_nowrap,
                            letter_spacing: child_inherited.letter_spacing,
                            spans: Some(spans.clone()),
                        };
                        let leaf_style = Style {
                            size: Size {
                                width: percent(1.0),
                                height: auto(),
                            },
                            ..Default::default()
                        };
                        let leaf_id =
                            taffy.new_leaf_with_context(leaf_style, text_ctx).unwrap();
                        let leaf_data = NodeData {
                            text: Some(full_text),
                            text_color: child_inherited.color,
                            font_size: child_inherited.font_size,
                            font_family: child_inherited.font_family.clone(),
                            font_weight: child_inherited.font_weight,
                            font_italic: child_inherited.font_italic,
                            line_height: child_inherited.line_height,
                            text_align: data.text_align,
                            rich_spans: Some(spans),
                            ..Default::default()
                        };
                        node_data.insert(leaf_id, leaf_data);

                        let id = taffy.new_with_children(style, &[leaf_id]).unwrap();
                        node_data.insert(id, data);
                        return vec![id];
                    }
                }
            }

            let children: Vec<taffy::NodeId> = node_ref
                .children()
                .flat_map(|child| {
                    build_nodes(
                        child.id(),
                        tree,
                        taffy,
                        node_data,
                        &child_inherited,
                        style_index,
                        child_cellpadding,
                        depth + 1,
                        &node_path,
                    )
                })
                .collect();

            // Fallback: if block has inline or inline-block children but rich text
            // collection failed, switch to flex-wrap so children flow horizontally.
            // Inline-block children (e.g. MJML mj-column-per-50 divs) need to sit
            // side-by-side; flex-wrap gives the correct wrapping behavior.
            // (creatine_products)
            if style.display == Display::Block {
                let has_inline_children = node_ref.children().any(|child| {
                    if let Node::Element(child_el) = child.value() {
                        is_inline_tag(child_el.name())
                    } else {
                        false
                    }
                });
                let has_inline_block_children = children
                    .iter()
                    .any(|id| node_data.get(id).is_some_and(|d| d.display_inline_block));
                if has_inline_children || has_inline_block_children {
                    style.display = Display::Flex;
                    style.flex_wrap = FlexWrap::Wrap;
                    // Taffy's default align-items:stretch inflates auto-height
                    // inline-blocks to the tallest sibling (nav separator links
                    // measured 32px vs Chrome's 22px). Inline formatting never
                    // stretches atomic inlines: they size to content and sit on
                    // the baseline, which START approximates since stretch and
                    // baseline both start items at the line top for same
                    // line-height content. (gmail_creatine_week, creatine_header)
                    style.align_items = Some(AlignItems::START);
                    // Translate text-align to justify-content so inline/inline-block
                    // children are centered or right-aligned in the flex container,
                    // matching CSS inline formatting behavior. (creatine_products)
                    match text_align {
                        TextAlign::Center => {
                            style.justify_content = Some(JustifyContent::CENTER);
                        }
                        TextAlign::Right => {
                            style.justify_content = Some(JustifyContent::END);
                        }
                        _ => {}
                    }
                    // Translate vertical-align:middle on inline-block children to
                    // align-items:center on the flex parent, matching CSS inline
                    // formatting where middle-aligned atomic inlines are vertically
                    // centered relative to each other. (creatine_products)
                    let any_valign_middle = children.iter().any(|id| {
                        node_data.get(id).is_some_and(|d| d.vertical_align_middle)
                    });
                    if any_valign_middle {
                        style.align_items = Some(AlignItems::CENTER);
                    }
                }
            }

            // Table cells with inline-block/inline-table children (MJML social
            // icons: <table style="display:inline-table"> x3 in one td) get an
            // anonymous flex-wrap container so the children flow horizontally
            // like an inline formatting context. The cell itself must stay
            // TableCell for the table algorithm, so the flex container is
            // inserted as its single child. (gmail_creatine_week)
            let children = if style.display == Display::TableCell
                && children
                    .iter()
                    .any(|id| node_data.get(id).is_some_and(|d| d.display_inline_block))
            {
                let mut wrap_style = Style {
                    display: Display::Flex,
                    flex_wrap: FlexWrap::Wrap,
                    // Same no-stretch rule as the block fallback above:
                    // atomic inlines size to content (gmail_creatine_week)
                    align_items: Some(AlignItems::START),
                    size: Size {
                        width: percent(1.0),
                        height: auto(),
                    },
                    ..Default::default()
                };
                match text_align {
                    TextAlign::Center => {
                        wrap_style.justify_content = Some(JustifyContent::CENTER);
                    }
                    TextAlign::Right => {
                        wrap_style.justify_content = Some(JustifyContent::END);
                    }
                    _ => {}
                }
                let wrapper = taffy.new_with_children(wrap_style, &children).unwrap();
                vec![wrapper]
            } else {
                children
            };

            let id = taffy.new_with_children(style, &children).unwrap();
            node_data.insert(id, data);
            vec![id]
        }
        Node::Text(text) => {
            let text_str = text.text.trim();
            if text_str.is_empty() {
                // Preserve inter-element whitespace as a single space between
                // inline-block siblings. Without this, trimmed whitespace lets
                // atomic inlines pack tighter than browsers render, preventing
                // flex-wrap from breaking them onto separate lines.
                // (creatine_products)
                if !text.text.contains(char::is_whitespace) {
                    return vec![];
                }
                let node_ref = tree.get(node_id).unwrap();
                let sibling_is_inline_block = |sib: ego_tree::NodeRef<Node>| -> bool {
                    if let Node::Element(el) = sib.value() {
                        let el_ref = match scraper::ElementRef::wrap(sib) {
                            Some(r) => r,
                            None => return false,
                        };
                        let rule_css = style_index.match_element_ref(&el_ref);
                        let inline_css = el.attr("style").unwrap_or("");
                        let merged = if !rule_css.is_empty() && !inline_css.is_empty() {
                            format!("{rule_css};{inline_css}")
                        } else if !rule_css.is_empty() {
                            rule_css
                        } else {
                            inline_css.to_string()
                        };
                        if merged.is_empty() {
                            return false;
                        }
                        parse_inline_style(&merged).display_inline_block
                    } else {
                        false
                    }
                };
                let prev_ib = node_ref
                    .prev_sibling()
                    .is_some_and(sibling_is_inline_block);
                let next_ib = node_ref
                    .next_sibling()
                    .is_some_and(sibling_is_inline_block);
                if !(prev_ib && next_ib) {
                    return vec![];
                }
                // Keep as a single space
                let text_str = " ".to_string();
                let text_ctx = TextMeasure {
                    text: text_str.clone(),
                    font_size: inherited.font_size,
                    font_family: inherited.font_family.clone(),
                    font_weight: inherited.font_weight,
                    font_italic: inherited.font_italic,
                    line_height: inherited.line_height,
                    white_space_nowrap: inherited.white_space_nowrap,
                    letter_spacing: inherited.letter_spacing,
                    spans: None,
                };
                let style = Style {
                    ..Default::default()
                };
                let data = NodeData {
                    text: Some(text_str),
                    text_color: inherited.color,
                    font_size: inherited.font_size,
                    font_family: inherited.font_family.clone(),
                    font_weight: inherited.font_weight,
                    font_italic: inherited.font_italic,
                    line_height: inherited.line_height,
                    letter_spacing: inherited.letter_spacing,
                    text_align: inherited.text_align,
                    ..Default::default()
                };
                let id = taffy.new_leaf_with_context(style, text_ctx).unwrap();
                node_data.insert(id, data);
                return vec![id];
            }
            let text_str = apply_text_transform(text_str, inherited.text_transform);

            let text_ctx = TextMeasure {
                text: text_str.clone(),
                font_size: inherited.font_size,
                font_family: inherited.font_family.clone(),
                font_weight: inherited.font_weight,
                font_italic: inherited.font_italic,
                line_height: inherited.line_height,
                white_space_nowrap: inherited.white_space_nowrap,
                letter_spacing: inherited.letter_spacing,
                spans: None,
            };

            let style = Style {
                ..Default::default()
            };
            let data = NodeData {
                text: Some(text_str),
                text_color: inherited.color,
                font_size: inherited.font_size,
                font_family: inherited.font_family.clone(),
                font_weight: inherited.font_weight,
                font_italic: inherited.font_italic,
                line_height: inherited.line_height,
                letter_spacing: inherited.letter_spacing,
                text_align: inherited.text_align,
                ..Default::default()
            };
            let id = taffy.new_leaf_with_context(style, text_ctx).unwrap();
            node_data.insert(id, data);
            vec![id]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use taffy::prelude::*;

    fn length(v: f32) -> Dimension {
        Dimension::length(v)
    }
    fn percent(v: f32) -> Dimension {
        Dimension::percent(v)
    }
    fn auto() -> Dimension {
        Dimension::auto()
    }

    /// Pure taffy test: 3-column table with explicit pixel widths.
    /// Verifies taffy distributes column widths correctly.
    #[test]
    fn taffy_table_pixel_widths() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Simulate text children inside cells (like the pipeline does)
        let text1 = taffy
            .new_leaf(Style {
                size: Size {
                    width: length(40.0),
                    height: length(20.0),
                },
                ..Default::default()
            })
            .unwrap();
        let text2 = taffy
            .new_leaf(Style {
                size: Size {
                    width: length(40.0),
                    height: length(20.0),
                },
                ..Default::default()
            })
            .unwrap();
        let text3 = taffy
            .new_leaf(Style {
                size: Size {
                    width: length(40.0),
                    height: length(20.0),
                },
                ..Default::default()
            })
            .unwrap();

        // 3 cells with widths 100, 300, 200 (sum = 600 = table width)
        let cell1 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size {
                        width: length(100.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[text1],
            )
            .unwrap();
        let cell2 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size {
                        width: length(300.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[text2],
            )
            .unwrap();
        let cell3 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size {
                        width: length(200.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[text3],
            )
            .unwrap();
        let row = taffy
            .new_with_children(
                Style {
                    display: Display::TableRow,
                    ..Default::default()
                },
                &[cell1, cell2, cell3],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size {
                        width: length(600.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let l1 = taffy.layout(cell1).unwrap();
        let l2 = taffy.layout(cell2).unwrap();
        let l3 = taffy.layout(cell3).unwrap();

        println!("Cell1: x={}, w={}", l1.location.x, l1.size.width);
        println!("Cell2: x={}, w={}", l2.location.x, l2.size.width);
        println!("Cell3: x={}, w={}", l3.location.x, l3.size.width);

        assert!(
            l1.size.width < l2.size.width,
            "Cell1 (100px) should be narrower than Cell2 (300px): {} vs {}",
            l1.size.width,
            l2.size.width
        );
        assert!(
            l3.size.width < l2.size.width,
            "Cell3 (200px) should be narrower than Cell2 (300px): {} vs {}",
            l3.size.width,
            l2.size.width
        );
    }

    /// Pure taffy test: table row should have enough height for content.
    #[test]
    fn taffy_table_row_height() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        // Cell with a fixed-height child (simulating text)
        let text = taffy
            .new_leaf(Style {
                size: Size {
                    width: length(100.0),
                    height: length(20.0),
                },
                ..Default::default()
            })
            .unwrap();
        let cell = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    ..Default::default()
                },
                &[text],
            )
            .unwrap();
        let row = taffy
            .new_with_children(
                Style {
                    display: Display::TableRow,
                    ..Default::default()
                },
                &[cell],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size {
                        width: length(600.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let row_layout = taffy.layout(row).unwrap();
        let table_layout = taffy.layout(table).unwrap();

        println!("Row height: {}", row_layout.size.height);
        println!("Table height: {}", table_layout.size.height);

        assert!(
            row_layout.size.height >= 20.0,
            "Row should be at least as tall as content (20px), got {}",
            row_layout.size.height
        );
    }

    /// Pure taffy test: percentage widths on cells.
    #[test]
    fn taffy_table_percentage_widths() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let cell1 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size {
                    width: percent(0.2),
                    height: auto(),
                },
                ..Default::default()
            })
            .unwrap();
        let cell2 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size {
                    width: percent(0.5),
                    height: auto(),
                },
                ..Default::default()
            })
            .unwrap();
        let cell3 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size {
                    width: percent(0.3),
                    height: auto(),
                },
                ..Default::default()
            })
            .unwrap();
        let row = taffy
            .new_with_children(
                Style {
                    display: Display::TableRow,
                    ..Default::default()
                },
                &[cell1, cell2, cell3],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size {
                        width: length(600.0),
                        height: auto(),
                    },
                    ..Default::default()
                },
                &[row],
            )
            .unwrap();

        taffy.compute_layout(table, Size::MAX_CONTENT).unwrap();

        let l1 = taffy.layout(cell1).unwrap();
        let l2 = taffy.layout(cell2).unwrap();
        let l3 = taffy.layout(cell3).unwrap();

        println!("Cell1 (20%): x={}, w={}", l1.location.x, l1.size.width);
        println!("Cell2 (50%): x={}, w={}", l2.location.x, l2.size.width);
        println!("Cell3 (30%): x={}, w={}", l3.location.x, l3.size.width);

        // 20% of 600 = 120, 50% = 300, 30% = 180
        assert!(
            (l1.size.width - 120.0).abs() < 5.0,
            "Cell1 should be ~120px (20% of 600), got {}",
            l1.size.width
        );
        assert!(
            (l2.size.width - 300.0).abs() < 5.0,
            "Cell2 should be ~300px (50% of 600), got {}",
            l2.size.width
        );
        assert!(
            (l3.size.width - 180.0).abs() < 5.0,
            "Cell3 should be ~180px (30% of 600), got {}",
            l3.size.width
        );
    }
}
