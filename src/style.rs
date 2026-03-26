use lightningcss::media_query::{
    MediaCondition, MediaFeatureId, MediaFeatureValue, MediaQuery, MediaType, Operator,
    QueryFeature, Qualifier,
};
use lightningcss::stylesheet::ParserOptions;
use lightningcss::values::length::Length;
use scraper::Html;
use taffy::prelude::*;

use crate::css::{ComputedStyle, TextAlign, TextTransform};

/// Style rules extracted from <style> blocks.
/// Each rule is a (compiled selector, CSS declaration text) pair.
/// Uses scraper's selector matching for full CSS selector support
/// (descendant, child, compound selectors, etc.).
pub(crate) struct StyleIndex {
    rules: Vec<(scraper::Selector, String)>,
}

impl StyleIndex {
    /// Build style index from document, evaluating @media queries against viewport_width.
    /// Also respects `<style media="...">` attributes (gmail_creatine_week).
    pub(crate) fn from_document(document: &Html, viewport_width: f32) -> Self {
        use lightningcss::stylesheet::StyleSheet;
        use scraper::Selector;
        let mut rules = Vec::new();

        let style_sel = Selector::parse("style").unwrap();
        for style_el in document.select(&style_sel) {
            // Check <style media="..."> attribute — skip if media doesn't match
            // (gmail_creatine_week uses <style media="screen and (min-width:480px)">)
            if let Some(media_attr) = style_el.value().attr("media") {
                let mut input = cssparser::ParserInput::new(media_attr);
                let mut parser = cssparser::Parser::new(&mut input);
                if let Ok(media_list) = lightningcss::media_query::MediaList::parse(
                    &mut parser,
                    &ParserOptions::default(),
                ) && !eval_media_list(&media_list, viewport_width) {
                    continue;
                }
            }

            let css_text = style_el.text().collect::<String>();
            let Ok(sheet) = StyleSheet::parse(&css_text, ParserOptions::default()) else {
                continue;
            };
            Self::collect_rules(&sheet.rules.0, viewport_width, &mut rules);
        }

        Self { rules }
    }

    fn collect_rules(
        rules: &[lightningcss::rules::CssRule],
        viewport_width: f32,
        out: &mut Vec<(scraper::Selector, String)>,
    ) {
        use lightningcss::rules::CssRule;
        use lightningcss::stylesheet::PrinterOptions;
        use lightningcss::traits::ToCss;

        for rule in rules {
            match rule {
                CssRule::Style(style_rule) => {
                    let mut decl_text = String::new();
                    for prop in &style_rule.declarations.declarations {
                        if let Ok(css) = prop.to_css_string(false, PrinterOptions::default()) {
                            decl_text.push_str(&css);
                            decl_text.push(';');
                        }
                    }
                    for prop in &style_rule.declarations.important_declarations {
                        if let Ok(css) = prop.to_css_string(false, PrinterOptions::default()) {
                            decl_text.push_str(&css);
                            decl_text.push(';');
                        }
                    }

                    if decl_text.is_empty() {
                        continue;
                    }

                    let sel_text = style_rule
                        .selectors
                        .to_css_string(PrinterOptions::default())
                        .unwrap_or_default();

                    // Try to compile each comma-separated selector with scraper
                    for sel_str in sel_text.split(',') {
                        let sel_str = sel_str.trim();
                        // Skip pseudo-classes/elements (scraper doesn't support them)
                        if sel_str.contains(':') {
                            continue;
                        }
                        if let Ok(selector) = scraper::Selector::parse(sel_str) {
                            out.push((selector, decl_text.clone()));
                        }
                    }
                }
                // Evaluate @media blocks against viewport width and recurse
                // into matching rules (gmail_creatine_week)
                CssRule::Media(media_rule)
                    if eval_media_list(&media_rule.query, viewport_width) =>
                {
                    Self::collect_rules(&media_rule.rules.0, viewport_width, out);
                }
                _ => {}
            }
        }
    }

    /// Match an element against all rules in the stylesheet.
    /// The element must be passed as an ElementRef so scraper can check
    /// ancestor/descendant relationships.
    pub(crate) fn match_element_ref(&self, el_ref: &scraper::ElementRef) -> String {
        let mut matched = String::new();
        for (selector, decl_text) in &self.rules {
            if selector.matches(el_ref) {
                matched.push_str(decl_text);
            }
        }
        matched
    }

    pub(crate) fn selector_count(&self) -> usize {
        self.rules.len()
    }
}

/// Inherited CSS properties that cascade down the tree.
#[derive(Debug, Clone)]
pub(crate) struct InheritedStyle {
    pub(crate) color: (u8, u8, u8, u8),
    pub(crate) font_size: f32,
    pub(crate) font_family: String,
    pub(crate) font_weight: u16,
    pub(crate) font_italic: bool,
    pub(crate) text_align: TextAlign,
    pub(crate) line_height: Option<f32>,
    pub(crate) white_space_nowrap: bool,
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) text_transform: Option<TextTransform>,
    pub(crate) text_decoration_underline: bool,
    pub(crate) text_decoration_line_through: bool,
}

impl Default for InheritedStyle {
    fn default() -> Self {
        Self {
            color: (0, 0, 0, 255),
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            font_weight: 400,
            font_italic: false,
            text_align: TextAlign::Left,
            line_height: None,
            white_space_nowrap: false,
            letter_spacing: None,
            text_transform: None,
            text_decoration_underline: false,
            text_decoration_line_through: false,
        }
    }
}

impl InheritedStyle {
    pub(crate) fn with_overrides(&self, css: &ComputedStyle, tag: &str) -> Self {
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
        if let Some(lh) = css.line_height {
            out.line_height = Some(lh);
        } else if let Some(factor) = css.line_height_factor {
            out.line_height = Some(out.font_size * factor);
        }
        if css.white_space_nowrap {
            out.white_space_nowrap = true;
        }
        if let Some(ls) = css.letter_spacing {
            out.letter_spacing = Some(ls);
        }
        if let Some(tt) = css.text_transform {
            out.text_transform = Some(tt);
        }
        // text-decoration is NOT inherited in CSS, so reset for each element
        // then apply explicit CSS or tag defaults
        out.text_decoration_underline = false;
        out.text_decoration_line_through = false;
        if let Some(u) = css.text_decoration_underline {
            out.text_decoration_underline = u;
        }
        if let Some(lt) = css.text_decoration_line_through {
            out.text_decoration_line_through = lt;
        }
        // Tag-based defaults
        match tag {
            "a" => {
                // Only apply default link blue when no color was explicitly set
                // via CSS — checking the value is wrong because explicit black
                // is indistinguishable from inherited black (creatine_header)
                if css.color.is_none() {
                    out.color = (0, 102, 204, 255); // link blue
                }
                // Default underline for links unless explicitly overridden
                if css.text_decoration_underline.is_none() {
                    out.text_decoration_underline = true;
                }
            }
            "b" | "strong" => out.font_weight = out.font_weight.max(700),
            "i" | "em" => out.font_italic = true,
            "u" | "ins"
                if css.text_decoration_underline.is_none() =>
            {
                out.text_decoration_underline = true;
            }
            "del" | "s"
                if css.text_decoration_line_through.is_none() =>
            {
                out.text_decoration_line_through = true;
            }
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
            // Tables create new formatting contexts — reset text-align
            // so outer td align="center" doesn't cascade into nested tables.
            "table"
                if css.text_align.is_none() =>
            {
                out.text_align = TextAlign::Left;
            }
            _ => {}
        }
        out
    }
}

/// Tags that are CSS inline elements — they don't create layout boxes,
/// they just apply styling to their children.
pub(crate) fn is_inline_tag(tag: &str) -> bool {
    matches!(
        tag,
        "span" | "a" | "strong" | "em" | "b" | "i" | "u" | "small" | "big" | "sub" | "sup"
        | "code" | "kbd" | "samp" | "var" | "cite" | "abbr" | "mark" | "del" | "ins" | "s"
        | "q" | "dfn" | "ruby" | "rt" | "rp" | "bdi" | "bdo" | "wbr" | "time" | "data"
        | "output" | "font"
    )
}

/// Extract natural (width, height) from a PNG data URI by decoding just
/// the IHDR chunk header. Returns None if the src is not a PNG data URI
/// or the header can't be parsed. (creatine_products)
fn png_data_uri_dimensions(src: &str) -> Option<(u32, u32)> {
    let b64 = src.strip_prefix("data:image/png;base64,")?;
    let b64_prefix: String = b64.chars().filter(|c| !c.is_whitespace()).take(48).collect();
    let decoded = base64_decode_prefix(&b64_prefix)?;
    if decoded.len() < 24 {
        return None;
    }
    if &decoded[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes([decoded[16], decoded[17], decoded[18], decoded[19]]);
    let h = u32::from_be_bytes([decoded[20], decoded[21], decoded[22], decoded[23]]);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn base64_decode_prefix(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    }
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = val(chunk[2])?;
        let d = val(chunk[3])?;
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            out.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            out.push((c << 6) | d);
        }
    }
    Some(out)
}

pub(crate) fn element_style(
    tag: &str,
    el: &scraper::node::Element,
    css: &ComputedStyle,
) -> Style {
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

        "table" => {
            // Tables: use HTML width attr if present, otherwise auto.
            // CSS percentage widths (like width:100%) are intentionally NOT applied
            // to the taffy size because they inflate min-content measurements in
            // nested table layouts. The table layout algorithm handles available
            // space correctly without them.
            let table_width = if let Some(px) = css.width_px {
                length(px)
            } else if let Some(pct) = css.width_pct {
                percent(pct)
            } else {
                percent_width_from_attr(el).unwrap_or(auto())
            };
            Style {
                display: Display::Table,
                item_is_table: true,
                size: Size {
                    width: table_width,
                    height: auto(),
                },
                max_size: Size {
                    width: css.max_width_px.map_or(auto(), length),
                    height: auto(),
                },
                margin: Rect {
                    top: css
                        .margin_top
                        .map_or(auto(), LengthPercentageAuto::length),
                    bottom: css
                        .margin_bottom
                        .map_or(auto(), LengthPercentageAuto::length),
                    left: if css.margin_left_auto {
                        LengthPercentageAuto::auto()
                    } else {
                        css.margin_left
                            .map_or(auto(), LengthPercentageAuto::length)
                    },
                    right: if css.margin_right_auto {
                        LengthPercentageAuto::auto()
                    } else {
                        css.margin_right
                            .map_or(auto(), LengthPercentageAuto::length)
                    },
                },
                padding: Rect {
                    top: css
                        .padding_top
                        .map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    bottom: css
                        .padding_bottom
                        .map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    left: css
                        .padding_left
                        .map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    right: css
                        .padding_right
                        .map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                },
                table_layout: if css.table_layout_fixed {
                    taffy::TableLayout::Fixed
                } else {
                    taffy::TableLayout::Auto
                },
                ..Default::default()
            }
        }
        "tr" => Style {
            display: Display::TableRow,
            ..Default::default()
        },
        "td" | "th" => Style {
            display: Display::TableCell,
            padding: Rect {
                top: length(2.0),
                bottom: length(2.0),
                left: length(4.0),
                right: length(4.0),
            },
            ..Default::default()
        },
        "thead" | "tbody" | "tfoot" => Style {
            display: Display::TableRowGroup,
            ..Default::default()
        },

        "center" => Style {
            display: Display::Block,
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..Default::default()
        },

        // Inline elements: use Flex so they shrink-wrap their content
        // instead of stretching to fill parent width (no true inline in taffy)
        "span" | "a" | "strong" | "em" | "b" | "i" | "u" | "small" | "big" | "sub" | "sup"
        | "code" | "kbd" | "samp" | "var" | "cite" | "abbr" | "mark" | "del" | "ins" | "s"
        | "q" | "dfn" | "ruby" | "rt" | "rp" | "bdi" | "bdo" | "wbr" | "time" | "data"
        | "output" | "font" => Style {
            display: Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            ..Default::default()
        },

        "img" => {
            // Parse width/height from HTML attrs — support px ("600") and percent ("100%")
            // CSS overrides are applied later in apply_css_overrides
            let w_attr = el.attr("width");
            let h_attr = el.attr("height");
            let img_w = match w_attr {
                Some(v) if v.ends_with('%') => v
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .ok()
                    .map(|p| percent(p / 100.0)),
                Some(v) => v.parse::<f32>().ok().map(length),
                None => None,
            };
            let w_px = w_attr.and_then(|v| v.parse::<f32>().ok());
            // When CSS explicitly says height:auto, read intrinsic dimensions
            // from PNG data URI src and use aspect_ratio so taffy computes the
            // correct height for both px and % widths. Without intrinsic data
            // (no src or not a PNG data URI), fall back to square.
            // (creatine_products, creatine_hero)
            let css_height_auto = css.height_auto;
            let html_height_auto = matches!(h_attr, Some("auto") | Some(""));
            let intrinsic = el.attr("src").and_then(png_data_uri_dimensions);
            let (img_h, ratio) = if css_height_auto || html_height_auto {
                if let Some((nat_w, nat_h)) = intrinsic {
                    // Use aspect_ratio — height stays auto, taffy derives it
                    (None, Some(nat_w as f32 / nat_h as f32))
                } else {
                    // No intrinsic data (no src or not a PNG data URI) — use
                    // small cap to avoid inflating broken images (header_test)
                    (w_px.map(|w| length(w.min(32.0))), None)
                }
            } else {
                match h_attr {
                    Some(v) => (v.parse::<f32>().ok().map(length), None),
                    // No height attr, no CSS height:auto — use small default
                    // to avoid inflating logo-style images (header_test)
                    None => (w_px.map(|w| length(w.min(32.0))), None),
                }
            };
            Style {
                display: Display::Block,
                size: Size {
                    width: img_w.unwrap_or(auto()),
                    height: img_h.unwrap_or(auto()),
                },
                aspect_ratio: ratio,
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
            display: Display::Block,
            size: Size {
                width: length(0.0),
                // Height is set to inherited line-height in build_nodes
                height: length(0.0),
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

pub(crate) fn apply_align_attr(mut style: Style, el: &scraper::node::Element) -> Style {
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

pub(crate) fn apply_css_overrides(mut style: Style, css: &ComputedStyle) -> Style {
    // CSS default is content-box; taffy defaults to border-box.
    // Table cells keep border-box so column widths represent outer widths
    // consistently with taffy's table layout algorithm.
    // CSS default is content-box; taffy defaults to border-box
    style.box_sizing = taffy::BoxSizing::ContentBox;

    if css.display_none {
        style.display = Display::None;
        return style;
    }
    if css.display_inline_block {
        style.display = Display::Block;
    }

    // Tables handle their own CSS in element_style to avoid
    // percentage widths inflating min-content measurements.
    // Tables keep border-box so width:100% includes borders (matching browser behavior).
    if style.display == Display::Table {
        style.box_sizing = taffy::BoxSizing::BorderBox;
        if let Some(v) = css.border_top {
            style.border.top = LengthPercentage::length(v);
        }
        if let Some(v) = css.border_bottom {
            style.border.bottom = LengthPercentage::length(v);
        }
        if let Some(v) = css.border_left {
            style.border.left = LengthPercentage::length(v);
        }
        if let Some(v) = css.border_right {
            style.border.right = LengthPercentage::length(v);
        }
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
    if css.margin_left_auto {
        style.margin.left = LengthPercentageAuto::auto();
    } else if let Some(v) = css.margin_left {
        style.margin.left = LengthPercentageAuto::length(v);
    }
    if css.margin_right_auto {
        style.margin.right = LengthPercentageAuto::auto();
    } else if let Some(v) = css.margin_right {
        style.margin.right = LengthPercentageAuto::length(v);
    }

    if let Some(v) = css.border_top {
        style.border.top = LengthPercentage::length(v);
    }
    if let Some(v) = css.border_bottom {
        style.border.bottom = LengthPercentage::length(v);
    }
    if let Some(v) = css.border_left {
        style.border.left = LengthPercentage::length(v);
    }
    if let Some(v) = css.border_right {
        style.border.right = LengthPercentage::length(v);
    }

    style
}

pub(crate) fn percent_width_from_attr(el: &scraper::node::Element) -> Option<Dimension> {
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

// --- Media query evaluation ---

/// Evaluate a media query list (OR semantics: any query matching → true).
/// Empty list matches everything per CSS spec.
fn eval_media_list(list: &lightningcss::media_query::MediaList, viewport_width: f32) -> bool {
    if list.media_queries.is_empty() {
        return true;
    }
    list.media_queries
        .iter()
        .any(|mq| eval_media_query(mq, viewport_width))
}

/// Evaluate a single media query against the viewport width.
/// Handles qualifier (only/not), media type (screen/all/print), and conditions.
fn eval_media_query(query: &MediaQuery, viewport_width: f32) -> bool {
    // Check media type — we're a screen renderer, so screen and all match
    let type_matches = matches!(
        query.media_type,
        MediaType::All | MediaType::Screen
    );

    let condition_matches = match &query.condition {
        Some(condition) => eval_media_condition(condition, viewport_width),
        None => true,
    };

    let result = type_matches && condition_matches;

    // "not" negates the entire query, "only" is a no-op for modern parsers
    match query.qualifier {
        Some(Qualifier::Not) => !result,
        _ => result,
    }
}

/// Evaluate a media condition tree (features, not, and/or operations).
fn eval_media_condition(condition: &MediaCondition, viewport_width: f32) -> bool {
    match condition {
        MediaCondition::Feature(feature) => eval_media_feature(feature, viewport_width),
        MediaCondition::Not(inner) => !eval_media_condition(inner, viewport_width),
        MediaCondition::Operation {
            operator,
            conditions,
        } => match operator {
            Operator::And => conditions
                .iter()
                .all(|c| eval_media_condition(c, viewport_width)),
            Operator::Or => conditions
                .iter()
                .any(|c| eval_media_condition(c, viewport_width)),
        },
        // Unknown tokens — can't evaluate, skip conservatively
        MediaCondition::Unknown(_) => false,
    }
}

/// Extract pixel value from a media feature length value.
fn length_to_px(value: &MediaFeatureValue) -> Option<f32> {
    match value {
        MediaFeatureValue::Length(Length::Value(lv)) => lv.to_px(),
        // Number 0 is valid as a zero-length in media queries
        MediaFeatureValue::Number(n) if *n == 0.0 => Some(0.0),
        _ => None,
    }
}

/// Evaluate a single media feature against the viewport width.
/// Only width features are supported — other features (height, color, etc.)
/// are conservatively treated as non-matching.
fn eval_media_feature(feature: &QueryFeature<'_, MediaFeatureId>, viewport_width: f32) -> bool {
    use lightningcss::media_query::MediaFeatureComparison::*;
    use lightningcss::media_query::MediaFeatureName;

    match feature {
        // (width) — boolean: true if viewport has non-zero width
        QueryFeature::Boolean { name } => matches!(
            name,
            MediaFeatureName::Standard(MediaFeatureId::Width)
        ),

        // (width: 600px) or (min-width: 480px) — plain equality or
        // range comparison (lightningcss normalizes min-/max- to Range)
        QueryFeature::Plain { name, value } => match name {
            MediaFeatureName::Standard(MediaFeatureId::Width) => {
                length_to_px(value).is_some_and(|px| (viewport_width - px).abs() < 0.01)
            }
            _ => false,
        },

        // (width >= 480px), (width <= 600px), etc.
        // lightningcss normalizes min-width/max-width to this form
        QueryFeature::Range {
            name,
            operator,
            value,
        } => match name {
            MediaFeatureName::Standard(MediaFeatureId::Width) => {
                let Some(px) = length_to_px(value) else {
                    return false;
                };
                match operator {
                    Equal => (viewport_width - px).abs() < 0.01,
                    GreaterThan => viewport_width > px,
                    GreaterThanEqual => viewport_width >= px,
                    LessThan => viewport_width < px,
                    LessThanEqual => viewport_width <= px,
                }
            }
            _ => false,
        },

        // (480px <= width <= 600px)
        QueryFeature::Interval {
            name,
            start,
            start_operator,
            end,
            end_operator,
        } => match name {
            MediaFeatureName::Standard(MediaFeatureId::Width) => {
                let (Some(start_px), Some(end_px)) = (length_to_px(start), length_to_px(end))
                else {
                    return false;
                };
                let start_ok = match start_operator {
                    LessThan => start_px < viewport_width,
                    LessThanEqual => start_px <= viewport_width,
                    _ => false,
                };
                let end_ok = match end_operator {
                    LessThan => viewport_width < end_px,
                    LessThanEqual => viewport_width <= end_px,
                    _ => false,
                };
                start_ok && end_ok
            }
            _ => false,
        },
    }
}
