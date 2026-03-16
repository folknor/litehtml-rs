use lightningcss::stylesheet::ParserOptions;
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
    pub(crate) fn from_document(document: &Html) -> Self {
        use lightningcss::stylesheet::StyleSheet;
        use scraper::Selector;
        let mut rules = Vec::new();

        let style_sel = Selector::parse("style").unwrap();
        for style_el in document.select(&style_sel) {
            let css_text = style_el.text().collect::<String>();
            let Ok(sheet) = StyleSheet::parse(&css_text, ParserOptions::default()) else {
                continue;
            };
            Self::collect_rules(&sheet.rules.0, &mut rules);
        }

        Self { rules }
    }

    fn collect_rules(
        rules: &[lightningcss::rules::CssRule],
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
                CssRule::Media(_) => {
                    // Skip @media blocks — we render at a fixed 800px viewport
                    // and email @media queries are typically max-width:600px responsive
                    // overrides that shouldn't apply at desktop width.
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
                if out.color == (0, 0, 0, 255) {
                    out.color = (0, 102, 204, 255); // link blue
                }
                // Default underline for links unless explicitly overridden
                if css.text_decoration_underline.is_none() {
                    out.text_decoration_underline = true;
                }
            }
            "b" | "strong" => out.font_weight = out.font_weight.max(700),
            "i" | "em" => out.font_italic = true,
            "u" | "ins" => {
                if css.text_decoration_underline.is_none() {
                    out.text_decoration_underline = true;
                }
            }
            "del" | "s" => {
                if css.text_decoration_line_through.is_none() {
                    out.text_decoration_line_through = true;
                }
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
            "table" => {
                if css.text_align.is_none() {
                    out.text_align = TextAlign::Left;
                }
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
                        .map_or(auto(), |v| LengthPercentageAuto::length(v)),
                    bottom: css
                        .margin_bottom
                        .map_or(auto(), |v| LengthPercentageAuto::length(v)),
                    left: if css.margin_left_auto {
                        LengthPercentageAuto::auto()
                    } else {
                        css.margin_left
                            .map_or(auto(), |v| LengthPercentageAuto::length(v))
                    },
                    right: if css.margin_right_auto {
                        LengthPercentageAuto::auto()
                    } else {
                        css.margin_right
                            .map_or(auto(), |v| LengthPercentageAuto::length(v))
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
            let w = el.attr("width").and_then(|v| v.parse::<f32>().ok());
            let h = el.attr("height").and_then(|v| v.parse::<f32>().ok());
            // Without an actual image source, use width as height if no height attr
            // (most email images are roughly square placeholders)
            let default_h = w.unwrap_or(100.0).min(32.0);
            Style {
                display: Display::Block,
                size: Size {
                    width: w.map_or(length(100.0), length),
                    height: h.map_or(length(default_h), length),
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
