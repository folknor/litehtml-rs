use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use cosmic_text::{Attrs, Family, FontSystem, Metrics, Shaping, SwashCache};
use lightningcss::properties::display::Display as CssDisplay;
use lightningcss::properties::font::{
    FontFamily as CssFontFamily, FontSize as CssFontSize, FontStyle as CssFontStyle,
    FontWeight as CssFontWeight,
};
use lightningcss::properties::Property;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::values::color::CssColor;
use lightningcss::values::length::LengthPercentage as CssLengthPercentage;
use scraper::{Html, Node};
use taffy::prelude::*;

/// Computed style for a node, extracted from inline CSS.
#[derive(Debug, Clone, Default)]
struct ComputedStyle {
    display_none: bool,
    display_inline_block: bool,
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
    margin_left_auto: bool,
    margin_right_auto: bool,
    background_color: Option<(u8, u8, u8, u8)>,
    color: Option<(u8, u8, u8, u8)>,
    font_size: Option<f32>,
    font_family: Option<String>,
    font_weight: Option<u16>,
    font_style_italic: bool,
    border_top: Option<f32>,
    border_bottom: Option<f32>,
    border_left: Option<f32>,
    border_right: Option<f32>,
    border_top_color: Option<(u8, u8, u8, u8)>,
    border_bottom_color: Option<(u8, u8, u8, u8)>,
    border_left_color: Option<(u8, u8, u8, u8)>,
    border_right_color: Option<(u8, u8, u8, u8)>,
    line_height: Option<f32>,
    line_height_factor: Option<f32>,
    text_align: Option<TextAlign>,
    white_space_nowrap: bool,
    letter_spacing: Option<f32>,
    text_transform: Option<TextTransform>,
    table_layout_fixed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TextTransform {
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TextAlign {
    Left,
    Center,
    Right,
}

/// Convert a CssColor to RGBA tuple.
fn css_color_to_rgba(color: &CssColor) -> Option<(u8, u8, u8, u8)> {
    match color {
        CssColor::RGBA(rgba) => Some((rgba.red, rgba.green, rgba.blue, rgba.alpha)),
        other => {
            // Try converting to RGB for named colors, hsl, etc.
            if let Ok(rgb) = other.to_rgb() {
                if let CssColor::RGBA(rgba) = rgb {
                    return Some((rgba.red, rgba.green, rgba.blue, rgba.alpha));
                }
            }
            None
        }
    }
}

/// Extract px value from a CssLengthPercentage.
fn lp_to_px(lp: &CssLengthPercentage) -> Option<f32> {
    use lightningcss::values::length::LengthValue;
    match lp {
        CssLengthPercentage::Dimension(LengthValue::Px(v)) => Some(*v),
        CssLengthPercentage::Dimension(LengthValue::Pt(v)) => Some(v * 4.0 / 3.0),
        CssLengthPercentage::Dimension(LengthValue::Em(v)) => Some(v * 16.0),
        CssLengthPercentage::Dimension(LengthValue::Rem(v)) => Some(v * 16.0),
        _ => None,
    }
}

/// Extract percentage from a CssLengthPercentage (as 0.0-1.0).
fn lp_to_pct(lp: &CssLengthPercentage) -> Option<f32> {
    match lp {
        CssLengthPercentage::Percentage(p) => Some(p.0),
        _ => None,
    }
}

/// Apply a single lightningcss Property to our ComputedStyle.
fn apply_property(style: &mut ComputedStyle, prop: &Property) {
    use lightningcss::properties::border::BorderSideWidth;
    use lightningcss::properties::size::{MaxSize, Size};
    use lightningcss::values::length::LengthPercentageOrAuto;
    match prop {
        Property::Display(d) => {
            use lightningcss::properties::display::{DisplayKeyword, DisplayPair, DisplayOutside, DisplayInside};
            match d {
                CssDisplay::Keyword(DisplayKeyword::None) => {
                    style.display_none = true;
                }
                CssDisplay::Pair(DisplayPair { outside: DisplayOutside::Inline, inside: DisplayInside::FlowRoot, .. }) => {
                    style.display_inline_block = true;
                }
                _ => {}
            }
        }
        Property::Width(s) => match s {
            Size::LengthPercentage(lp) => {
                if let Some(px) = lp_to_px(lp) {
                    style.width_px = Some(px);
                } else if let Some(pct) = lp_to_pct(lp) {
                    style.width_pct = Some(pct);
                }
            }
            _ => {}
        },
        Property::Height(s) => match s {
            Size::LengthPercentage(lp) => {
                if let Some(px) = lp_to_px(lp) {
                    style.height_px = Some(px);
                } else if let Some(pct) = lp_to_pct(lp) {
                    style.height_pct = Some(pct);
                }
            }
            _ => {}
        },
        Property::MaxWidth(s) => {
            if let MaxSize::LengthPercentage(lp) = s {
                style.max_width_px = lp_to_px(lp);
            }
        }
        Property::PaddingTop(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.padding_top = lp_to_px(lp);
            }
        }
        Property::PaddingBottom(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.padding_bottom = lp_to_px(lp);
            }
        }
        Property::PaddingLeft(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.padding_left = lp_to_px(lp);
            }
        }
        Property::PaddingRight(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.padding_right = lp_to_px(lp);
            }
        }
        Property::Padding(p) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &p.top {
                style.padding_top = lp_to_px(lp);
            }
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &p.bottom {
                style.padding_bottom = lp_to_px(lp);
            }
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &p.left {
                style.padding_left = lp_to_px(lp);
            }
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &p.right {
                style.padding_right = lp_to_px(lp);
            }
        }
        Property::MarginTop(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.margin_top = lp_to_px(lp);
            }
        }
        Property::MarginBottom(lp) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = lp {
                style.margin_bottom = lp_to_px(lp);
            }
        }
        Property::MarginLeft(lp) => match lp {
            LengthPercentageOrAuto::Auto => style.margin_left_auto = true,
            LengthPercentageOrAuto::LengthPercentage(lp) => {
                style.margin_left = lp_to_px(lp);
                style.margin_left_auto = false;
            }
        },
        Property::MarginRight(lp) => match lp {
            LengthPercentageOrAuto::Auto => style.margin_right_auto = true,
            LengthPercentageOrAuto::LengthPercentage(lp) => {
                style.margin_right = lp_to_px(lp);
                style.margin_right_auto = false;
            }
        },
        Property::Margin(m) => {
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &m.top {
                style.margin_top = lp_to_px(lp);
            }
            if let LengthPercentageOrAuto::LengthPercentage(lp) = &m.bottom {
                style.margin_bottom = lp_to_px(lp);
            }
            match &m.left {
                LengthPercentageOrAuto::Auto => style.margin_left_auto = true,
                LengthPercentageOrAuto::LengthPercentage(lp) => {
                    style.margin_left = lp_to_px(lp);
                    style.margin_left_auto = false;
                }
            }
            match &m.right {
                LengthPercentageOrAuto::Auto => style.margin_right_auto = true,
                LengthPercentageOrAuto::LengthPercentage(lp) => {
                    style.margin_right = lp_to_px(lp);
                    style.margin_right_auto = false;
                }
            }
        }
        Property::BackgroundColor(c) => {
            style.background_color = css_color_to_rgba(c);
        }
        Property::Color(c) => {
            style.color = css_color_to_rgba(c);
        }
        Property::FontSize(fs) => {
            style.font_size = match fs {
                CssFontSize::Length(lp) => lp_to_px(lp),
                _ => None,
            };
        }
        Property::FontFamily(families) => {
            if let Some(first) = families.first() {
                style.font_family = Some(match first {
                    CssFontFamily::FamilyName(name) => {
                        use lightningcss::traits::ToCss;
                        use lightningcss::stylesheet::PrinterOptions;
                        name.to_css_string(PrinterOptions::default())
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string()
                    }
                    CssFontFamily::Generic(g) => format!("{:?}", g).to_lowercase(),
                });
            }
        }
        Property::FontWeight(fw) => {
            style.font_weight = Some(match fw {
                CssFontWeight::Absolute(a) => {
                    use lightningcss::properties::font::AbsoluteFontWeight;
                    match a {
                        AbsoluteFontWeight::Weight(n) => *n as u16,
                        AbsoluteFontWeight::Normal => 400,
                        AbsoluteFontWeight::Bold => 700,
                    }
                }
                CssFontWeight::Bolder => 700,
                CssFontWeight::Lighter => 300,
            });
        }
        Property::FontStyle(fs) => {
            style.font_style_italic = matches!(
                fs,
                CssFontStyle::Italic | CssFontStyle::Oblique(_)
            );
        }
        Property::LineHeight(lh) => {
            use lightningcss::properties::font::LineHeight;
            match lh {
                LineHeight::Length(lp) => {
                    style.line_height = lp_to_px(lp);
                }
                LineHeight::Number(n) => {
                    style.line_height_factor = Some(*n);
                }
                _ => {}
            }
        }
        Property::TextAlign(ta) => {
            use lightningcss::properties::text::TextAlign as CssTextAlign;
            style.text_align = match ta {
                CssTextAlign::Center => Some(TextAlign::Center),
                CssTextAlign::Right | CssTextAlign::End => Some(TextAlign::Right),
                CssTextAlign::Left | CssTextAlign::Start => Some(TextAlign::Left),
                _ => None,
            };
        }
        Property::WhiteSpace(ws) => {
            use lightningcss::properties::text::WhiteSpace;
            if matches!(ws, WhiteSpace::NoWrap | WhiteSpace::Pre) {
                style.white_space_nowrap = true;
            }
        }
        Property::LetterSpacing(ls) => {
            use lightningcss::properties::text::Spacing;
            if let Spacing::Length(l) = ls {
                style.letter_spacing = l.to_px();
            }
        }
        Property::TextTransform(tt) => {
            use lightningcss::properties::text::TextTransformCase;
            style.text_transform = match tt.case {
                TextTransformCase::Uppercase => Some(TextTransform::Uppercase),
                TextTransformCase::Lowercase => Some(TextTransform::Lowercase),
                TextTransformCase::Capitalize => Some(TextTransform::Capitalize),
                _ => None,
            };
        }
        Property::BorderTopWidth(w) => {
            style.border_top = match w {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
        }
        Property::BorderBottomWidth(w) => {
            style.border_bottom = match w {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
        }
        Property::BorderLeft(b) => {
            style.border_left = match &b.width {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
            style.border_left_color = css_color_to_rgba(&b.color);
        }
        Property::BorderRight(b) => {
            style.border_right = match &b.width {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
            style.border_right_color = css_color_to_rgba(&b.color);
        }
        Property::BorderTop(b) => {
            style.border_top = match &b.width {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
            style.border_top_color = css_color_to_rgba(&b.color);
        }
        Property::BorderBottom(b) => {
            style.border_bottom = match &b.width {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
            style.border_bottom_color = css_color_to_rgba(&b.color);
        }
        Property::BorderLeftWidth(w) => {
            style.border_left = match w {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
        }
        Property::BorderRightWidth(w) => {
            style.border_right = match w {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
        }
        Property::BorderTopColor(c) => {
            style.border_top_color = css_color_to_rgba(c);
        }
        Property::BorderBottomColor(c) => {
            style.border_bottom_color = css_color_to_rgba(c);
        }
        Property::BorderLeftColor(c) => {
            style.border_left_color = css_color_to_rgba(c);
        }
        Property::BorderRightColor(c) => {
            style.border_right_color = css_color_to_rgba(c);
        }
        _ => {}
    }
}

/// Parse inline style attribute using lightningcss.
fn parse_inline_style(css: &str) -> ComputedStyle {
    use lightningcss::stylesheet::StyleAttribute;
    let mut style = ComputedStyle::default();
    let Ok(attr) = StyleAttribute::parse(css, ParserOptions::default()) else {
        return style;
    };
    for prop in &attr.declarations.declarations {
        apply_property(&mut style, prop);
    }
    for prop in &attr.declarations.important_declarations {
        apply_property(&mut style, prop);
    }
    // lightningcss doesn't parse table-layout, so check raw CSS
    if css.contains("table-layout") && css.contains("fixed") {
        style.table_layout_fixed = true;
    }
    style
}

/// Parse CSS color from string (for HTML attributes like bgcolor).
fn parse_css_color(val: &str) -> Option<(u8, u8, u8, u8)> {
    // Use lightningcss for proper parsing, with a color property wrapper
    let css = format!("color: {val}");
    let style = parse_inline_style(&css);
    style.color
}

/// Parse a CSS value as px (for HTML attributes like width="600").
fn parse_css_value_px(val: &str) -> Option<f32> {
    let val = val.trim();
    if val == "0" {
        return Some(0.0);
    }
    // Try as plain number first (HTML attributes)
    if let Ok(n) = val.parse::<f32>() {
        return Some(n);
    }
    // Try as CSS length
    let css = format!("width: {val}");
    let style = parse_inline_style(&css);
    style.width_px
}

/// Parse a CSS value as percentage (0.0-1.0).
fn parse_css_value_pct(val: &str) -> Option<f32> {
    let css = format!("width: {val}");
    let style = parse_inline_style(&css);
    style.width_pct
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
    style
}

/// Per-node rendering data.
#[derive(Debug, Clone)]
struct NodeData {
    tag: String,
    dom_path: String,
    depth: usize,
    id_attr: Option<String>,
    classes: Option<String>,
    background_color: Option<(u8, u8, u8, u8)>,
    text: Option<String>,
    text_color: (u8, u8, u8, u8),
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_italic: bool,
    line_height: Option<f32>,
    letter_spacing: Option<f32>,
    border_top: Option<f32>,
    border_bottom: Option<f32>,
    border_left: Option<f32>,
    border_right: Option<f32>,
    border_top_color: Option<(u8, u8, u8, u8)>,
    border_bottom_color: Option<(u8, u8, u8, u8)>,
    border_left_color: Option<(u8, u8, u8, u8)>,
    border_right_color: Option<(u8, u8, u8, u8)>,
    is_hr: bool,
    is_img: bool,
    text_align: TextAlign,
    padding: (f32, f32, f32, f32),
    margin: (f32, f32, f32, f32),
    max_width_px: Option<f32>,
    rich_spans: Option<Vec<RichTextSpan>>,
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
            is_hr: false,
            is_img: false,
            text_align: TextAlign::Left,
            padding: (0.0, 0.0, 0.0, 0.0),
            margin: (0.0, 0.0, 0.0, 0.0),
            max_width_px: None,
            rich_spans: None,
        }
    }
}

// Shared font system for text measurement during tree building.
thread_local! {
    static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
}

/// Load the Ahem test font into the font system.
fn load_ahem_font() {
    let ahem_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Ahem.ttf");
    if ahem_path.exists() {
        FONT_SYSTEM.with(|fs| {
            let mut fs = fs.borrow_mut();
            let data = std::fs::read(&ahem_path).expect("Failed to read Ahem.ttf");
            fs.db_mut().load_font_data(data);
        });
    }
}

/// Whether fixture mode is enabled (forces Ahem font)
static FIXTURE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_fixture_mode() -> bool {
    FIXTURE_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Resolve font family, overriding to Ahem in fixture mode.
fn resolve_font_family(family: &str) -> Family<'_> {
    if is_fixture_mode() {
        return Family::Name("Ahem");
    }
    match family {
        "serif" => Family::Serif,
        "sans-serif" | "sans serif" => Family::SansSerif,
        "monospace" => Family::Monospace,
        name => Family::Name(name),
    }
}

/// Build cosmic-text Attrs, forcing Ahem normal weight/style in fixture mode
/// so cosmic-text doesn't fall back to system fonts for bold/italic.
/// Ahem metrics are identical across weights so this doesn't affect layout.
fn build_text_attrs(family: &str, weight: u16, italic: bool, letter_spacing: Option<f32>) -> Attrs<'_> {
    let f = resolve_font_family(family);
    let mut attrs = if is_fixture_mode() {
        Attrs::new().family(f).weight(cosmic_text::Weight::NORMAL).style(cosmic_text::Style::Normal)
    } else {
        Attrs::new()
            .family(f)
            .weight(cosmic_text::Weight(weight))
            .style(if italic { cosmic_text::Style::Italic } else { cosmic_text::Style::Normal })
    };
    if let Some(ls) = letter_spacing {
        attrs = attrs.letter_spacing(ls);
    }
    attrs
}

/// A span of text with its own styling, used for rich text (inline elements).
#[derive(Debug, Clone)]
struct RichTextSpan {
    text: String,
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_italic: bool,
    color: (u8, u8, u8, u8),
    letter_spacing: Option<f32>,
}

/// Context stored on taffy leaf nodes for dynamic text measurement.
#[derive(Debug, Clone)]
struct TextMeasure {
    text: String,
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_italic: bool,
    line_height: Option<f32>,
    white_space_nowrap: bool,
    letter_spacing: Option<f32>,
    spans: Option<Vec<RichTextSpan>>,
}

/// Apply text-transform to a string.
fn apply_text_transform(text: &str, transform: Option<TextTransform>) -> String {
    match transform {
        Some(TextTransform::Uppercase) => text.to_uppercase(),
        Some(TextTransform::Lowercase) => text.to_lowercase(),
        Some(TextTransform::Capitalize) => {
            let mut result = String::with_capacity(text.len());
            let mut prev_is_space = true;
            for c in text.chars() {
                if prev_is_space && c.is_alphabetic() {
                    result.extend(c.to_uppercase());
                } else {
                    result.push(c);
                }
                prev_is_space = c.is_whitespace();
            }
            result
        }
        None => text.to_string(),
    }
}

/// Resolve line height: use CSS value if set, otherwise default to font_size * 1.4.
fn resolve_line_height(font_size: f32, css_line_height: Option<f32>) -> f32 {
    css_line_height.unwrap_or((font_size * 1.4).ceil()).max(1.0)
}

/// Measure function called by taffy during layout to determine text node size.
fn measure_text_node(
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
        known_dimensions.width.unwrap_or_else(|| match available_space.width {
            AvailableSpace::Definite(w) => w,
            AvailableSpace::MinContent => 1.0,
            AvailableSpace::MaxContent => f32::MAX,
        })
    };

    // Measure text wrapping at the available width
    let fs = ctx.font_size.max(1.0);
    let line_height = resolve_line_height(fs, ctx.line_height);

    FONT_SYSTEM.with(|font_sys| {
        let mut font_sys = font_sys.borrow_mut();
        let metrics = Metrics::new(fs, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut font_sys, metrics);
        buffer.set_size(&mut font_sys, Some(available_width), Some(line_height * 100.0));

        if let Some(ref spans) = ctx.spans {
            let rich: Vec<(&str, cosmic_text::Attrs)> = spans
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let attrs = build_text_attrs(&s.font_family, s.font_weight, s.font_italic, s.letter_spacing)
                        .metadata(i);
                    (s.text.as_str(), attrs)
                })
                .collect();
            let default_attrs = build_text_attrs(&ctx.font_family, ctx.font_weight, ctx.font_italic, ctx.letter_spacing);
            buffer.set_rich_text(&mut font_sys, rich, &default_attrs, Shaping::Basic, None);
        } else {
            let attrs = build_text_attrs(&ctx.font_family, ctx.font_weight, ctx.font_italic, ctx.letter_spacing);
            buffer.set_text(&mut font_sys, &ctx.text, &attrs, Shaping::Basic, None);
        }
        buffer.shape_until_scroll(&mut font_sys, false);

        let num_lines = buffer.layout_runs().count().max(1) as f32;
        let text_width = buffer.layout_runs().map(|r| r.line_w).fold(0.0_f32, f32::max);

        Size {
            width: known_dimensions.width.unwrap_or(text_width.min(available_width)),
            height: known_dimensions.height.unwrap_or(line_height * num_lines),
        }
    })
}

fn measure_text_width(text: &str, font_size: f32, family: &str, weight: u16, italic: bool) -> f32 {
    let font_size = font_size.max(1.0);
    FONT_SYSTEM.with(|fs| {
        let mut fs = fs.borrow_mut();
        let line_height = (font_size * 1.4).ceil().max(1.0);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        buffer.set_size(&mut fs, Some(f32::MAX), Some(line_height));

        let attrs = build_text_attrs(family, weight, italic, None);
        buffer.set_text(&mut fs, text, &attrs, Shaping::Basic, None);
        buffer.shape_until_scroll(&mut fs, false);

        buffer.layout_runs().map(|run| run.line_w).sum::<f32>()
    })
}

/// Style rules extracted from <style> blocks.
/// Each rule is a (compiled selector, CSS declaration text) pair.
/// Uses scraper's selector matching for full CSS selector support
/// (descendant, child, compound selectors, etc.).
struct StyleIndex {
    rules: Vec<(scraper::Selector, String)>,
}

impl StyleIndex {
    fn from_document(document: &Html) -> Self {
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
        use lightningcss::traits::ToCss;
        use lightningcss::stylesheet::PrinterOptions;

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
    fn match_element_ref(&self, el_ref: &scraper::ElementRef) -> String {
        let mut matched = String::new();
        for (selector, decl_text) in &self.rules {
            if selector.matches(el_ref) {
                matched.push_str(decl_text);
            }
        }
        matched
    }

    fn selector_count(&self) -> usize {
        self.rules.len()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dump_mode = args.iter().any(|a| a == "--dump");
    let fixture_mode = args.iter().any(|a| a == "--fixture");
    if fixture_mode {
        FIXTURE_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
        load_ahem_font();
    }
    let path = args.iter().find(|a| !a.starts_with('-') && *a != &args[0]).cloned().unwrap_or_else(|| {
        eprintln!("Usage: litehtml-rs [--dump] [--fixture] <html-file>");
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

    // Extract <style> block rules using lightningcss
    let style_index = StyleIndex::from_document(&document);
    let selector_count = style_index.selector_count();
    if selector_count > 0 {
        println!("Style rules extracted: {} selectors", selector_count);
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
    taffy.compute_layout_with_measure(root, viewport, measure_text_node).unwrap();
    let layout_time = t2.elapsed();

    let layout = taffy.layout(root).unwrap();
    println!(
        "Layout: {}x{} (root)",
        layout.size.width, layout.size.height
    );

    // Dump mode: output JSON layout data and exit
    if dump_mode {
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

        let mut entries = Vec::new();
        dump_json(&taffy, &node_data, root, 0.0, 0.0, &mut entries);
        let out_path = path.replace(".html", "_pipeline.json");
        let json = format!("[\n{}\n]", entries.join(",\n"));
        std::fs::write(&out_path, &json).unwrap();
        eprintln!("Dumped {} elements to {}", entries.len(), out_path);
        return;
    }

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

    let pipeline_total = parse_time + tree_build_time + layout_time + render_time;

    println!("Saved to {out_path}");
    println!("--- Timing ---");
    println!("  Font init:    {:?} (one-time)", font_init_time);
    println!("  HTML parse:   {:?}", parse_time);
    println!("  Tree+CSS:     {:?}", tree_build_time);
    println!("  Layout:       {:?}", layout_time);
    println!("  Render:       {:?}", render_time);
    println!("  PNG encode:   {:?}", png_time);
    println!("  Total (no PNG): {:?}", pipeline_total);
    println!(
        "  Total (warm):   {:?} (excludes font init)",
        pipeline_total
    );

    // Machine-readable key=value output for brokkr
    eprintln!("elapsed_ms={}", pipeline_total.as_millis());
    eprintln!("parse_ms={}", parse_time.as_micros() as f64 / 1000.0);
    eprintln!("tree_css_ms={}", tree_build_time.as_micros() as f64 / 1000.0);
    eprintln!("layout_ms={}", layout_time.as_micros() as f64 / 1000.0);
    eprintln!("render_ms={}", render_time.as_micros() as f64 / 1000.0);
    eprintln!("png_ms={}", png_time.as_micros() as f64 / 1000.0);
    eprintln!("font_init_ms={}", font_init_time.as_micros() as f64 / 1000.0);
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
    line_height: Option<f32>,
    white_space_nowrap: bool,
    letter_spacing: Option<f32>,
    text_transform: Option<TextTransform>,
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

/// Tags that are CSS inline elements — they don't create layout boxes,
/// they just apply styling to their children.
fn is_inline_tag(tag: &str) -> bool {
    matches!(
        tag,
        "span" | "a" | "strong" | "em" | "b" | "i" | "u" | "small" | "big" | "sub" | "sup"
        | "code" | "kbd" | "samp" | "var" | "cite" | "abbr" | "mark" | "del" | "ins" | "s"
        | "q" | "dfn" | "ruby" | "rt" | "rp" | "bdi" | "bdo" | "wbr" | "time" | "data"
        | "output" | "font"
    )
}

/// Recursively collect text from inline children into rich text spans.
/// Returns None if any non-inline content is found (images, nested blocks, etc.)
/// signaling fallback to the current flex approach.
fn collect_inline_text(
    node_ref: ego_tree::NodeRef<'_, Node>,
    inherited: &InheritedStyle,
    style_index: &StyleIndex,
    spans: &mut Vec<RichTextSpan>,
) -> bool {
    for child in node_ref.children() {
        match child.value() {
            Node::Text(text) => {
                // Normalize whitespace: collapse runs of whitespace to single spaces
                let collapsed: String = text.text.split_whitespace().collect::<Vec<_>>().join(" ");
                if collapsed.is_empty() {
                    continue;
                }
                let collapsed = apply_text_transform(&collapsed, inherited.text_transform);
                // Add space separator between spans if needed
                if let Some(last) = spans.last() {
                    if !last.text.ends_with(' ') && !collapsed.starts_with(' ') {
                        spans.push(RichTextSpan {
                            text: " ".to_string(),
                            font_size: inherited.font_size,
                            font_family: inherited.font_family.clone(),
                            font_weight: inherited.font_weight,
                            font_italic: inherited.font_italic,
                            color: inherited.color,
                            letter_spacing: inherited.letter_spacing,
                        });
                    }
                }
                spans.push(RichTextSpan {
                    text: collapsed,
                    font_size: inherited.font_size,
                    font_family: inherited.font_family.clone(),
                    font_weight: inherited.font_weight,
                    font_italic: inherited.font_italic,
                    color: inherited.color,
                    letter_spacing: inherited.letter_spacing,
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
/// are flattened — their children become direct children of the parent.
fn build_nodes(
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
                    if let Node::Element(sib_el) = s.value() {
                        if sib_el.name() == tag {
                            idx += 1;
                        }
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

            let child_inherited = inherited.with_overrides(&computed, tag);

            let mut style = element_style(tag, el, &computed);

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
                tag: tag.to_string(),
                dom_path: node_path.clone(),
                depth,
                id_attr: el.attr("id").map(|s| s.to_string()),
                classes: el.attr("class").map(|s| s.to_string()),
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
                rich_spans: None,
            };

            // If this is a <table> with cellpadding, propagate to child cells
            let child_cellpadding = if tag == "table" {
                el.attr("cellpadding")
                    .and_then(|v| parse_css_value_px(v))
                    .or(cellpadding)
            } else {
                cellpadding
            };

            // Apply cellpadding to td/th cells
            let mut style = if (tag == "td" || tag == "th") && child_cellpadding.is_some() {
                let cp = child_cellpadding.unwrap();
                let mut s = style;
                // Only override default padding, not explicit CSS padding
                if computed.padding_top.is_none() {
                    s.padding = Rect {
                        top: length(cp),
                        bottom: length(cp),
                        left: length(cp),
                        right: length(cp),
                    };
                }
                s
            } else {
                style
            };

            // Try rich text collection for block/cell elements with inline children
            if matches!(style.display, Display::Block | Display::TableCell) {
                let has_inline_children = node_ref.children().any(|child| {
                    match child.value() {
                        Node::Element(child_el) => is_inline_tag(child_el.name()) || child_el.name() == "br",
                        Node::Text(_) => false,
                        _ => false,
                    }
                });
                if has_inline_children {
                    let mut spans = Vec::new();
                    if collect_inline_text(node_ref, &child_inherited, style_index, &mut spans) && !spans.is_empty() {
                        // Success: create a single rich text leaf node
                        let full_text: String = spans.iter().map(|s| s.text.as_str()).collect();
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
                        let leaf_id = taffy.new_leaf_with_context(Style::default(), text_ctx).unwrap();
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
                    build_nodes(child.id(), tree, taffy, node_data, &child_inherited, style_index, child_cellpadding, depth + 1, &node_path)
                })
                .collect();

            // Fallback: if block has inline children but rich text collection failed,
            // switch to flex layout so inline children can shrink-wrap.
            if style.display == Display::Block {
                let has_inline_children = node_ref.children().any(|child| {
                    if let Node::Element(child_el) = child.value() {
                        is_inline_tag(child_el.name())
                    } else {
                        false
                    }
                });
                if has_inline_children {
                    style.display = Display::Flex;
                    style.flex_wrap = FlexWrap::Wrap;
                }
            }

            let id = taffy.new_with_children(style, &children).unwrap();
            node_data.insert(id, data);
            vec![id]
        }
        Node::Text(text) => {
            let text_str = text.text.trim();
            if text_str.is_empty() {
                return vec![];
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
        _ => vec![]
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
                    top: css.margin_top.map_or(auto(), |v| LengthPercentageAuto::length(v)),
                    bottom: css.margin_bottom.map_or(auto(), |v| LengthPercentageAuto::length(v)),
                    left: if css.margin_left_auto { LengthPercentageAuto::auto() } else { css.margin_left.map_or(auto(), |v| LengthPercentageAuto::length(v)) },
                    right: if css.margin_right_auto { LengthPercentageAuto::auto() } else { css.margin_right.map_or(auto(), |v| LengthPercentageAuto::length(v)) },
                },
                padding: Rect {
                    top: css.padding_top.map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    bottom: css.padding_bottom.map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    left: css.padding_left.map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                    right: css.padding_right.map_or(LengthPercentage::length(0.0), LengthPercentage::length),
                },
                table_layout: if css.table_layout_fixed { taffy::TableLayout::Fixed } else { taffy::TableLayout::Auto },
                ..Default::default()
            }
        },
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
            display: Display::Block,
            size: Size {
                width: length(0.0),
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
        if let Some(v) = css.border_top { style.border.top = LengthPercentage::length(v); }
        if let Some(v) = css.border_bottom { style.border.bottom = LengthPercentage::length(v); }
        if let Some(v) = css.border_left { style.border.left = LengthPercentage::length(v); }
        if let Some(v) = css.border_right { style.border.right = LengthPercentage::length(v); }
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
    taffy: &TaffyTree<TextMeasure>,
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

        // Draw image placeholder (gray box matching Chrome's img { background-color: #d0d0d0 })
        if data.is_img {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                let paint = tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(
                        tiny_skia::Color::from_rgba8(208, 208, 208, 255),
                    ),
                    anti_alias: false,
                    ..Default::default()
                };
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
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

        // Draw borders
        let draw_border = |pixmap: &mut tiny_skia::Pixmap, bx: f32, by: f32, bw: f32, bh: f32, color: Option<(u8, u8, u8, u8)>| {
            let (r, g, b, a) = color.unwrap_or((200, 200, 200, 255));
            if let Some(rect) = tiny_skia::Rect::from_xywh(bx, by, bw, bh) {
                let paint = tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(r, g, b, a)),
                    anti_alias: false,
                    ..Default::default()
                };
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }
        };
        if let Some(bw) = data.border_top {
            if bw > 0.0 { draw_border(pixmap, x, y, w, bw, data.border_top_color); }
        }
        if let Some(bw) = data.border_bottom {
            if bw > 0.0 { draw_border(pixmap, x, y + h - bw, w, bw, data.border_bottom_color); }
        }
        if let Some(bw) = data.border_left {
            if bw > 0.0 { draw_border(pixmap, x, y, bw, h, data.border_left_color); }
        }
        if let Some(bw) = data.border_right {
            if bw > 0.0 { draw_border(pixmap, x + w - bw, y, bw, h, data.border_right_color); }
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
        let line_height = resolve_line_height(font_size, data.line_height);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        // Use container width for text wrapping
        let available_width = container_width.max(1.0);
        buffer.set_size(&mut fs, Some(available_width), Some(line_height * 20.0));

        if let Some(ref spans) = data.rich_spans {
            let rich: Vec<(&str, cosmic_text::Attrs)> = spans
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let attrs = build_text_attrs(&s.font_family, s.font_weight, s.font_italic, s.letter_spacing)
                        .metadata(i);
                    (s.text.as_str(), attrs)
                })
                .collect();
            let default_attrs = build_text_attrs(&data.font_family, data.font_weight, data.font_italic, data.letter_spacing);
            buffer.set_rich_text(&mut fs, rich, &default_attrs, Shaping::Advanced, None);
        } else {
            let attrs = build_text_attrs(&data.font_family, data.font_weight, data.font_italic, data.letter_spacing);
            buffer.set_text(&mut fs, text, &attrs, Shaping::Advanced, None);
        }
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
                // Look up per-glyph color from rich text spans
                let (gr, gg, gb) = if let Some(ref spans) = data.rich_spans {
                    let idx = glyph.metadata;
                    if idx < spans.len() {
                        let (r, g, b, _) = spans[idx].color;
                        (r, g, b)
                    } else {
                        (cr, cg, cb)
                    }
                } else {
                    (cr, cg, cb)
                };
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
                                                gr,
                                                gg,
                                                gb,
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
                size: Size { width: length(40.0), height: length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let text2 = taffy
            .new_leaf(Style {
                size: Size { width: length(40.0), height: length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let text3 = taffy
            .new_leaf(Style {
                size: Size { width: length(40.0), height: length(20.0) },
                ..Default::default()
            })
            .unwrap();

        // 3 cells with widths 100, 300, 200 (sum = 600 = table width)
        let cell1 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size { width: length(100.0), height: auto() },
                    ..Default::default()
                },
                &[text1],
            )
            .unwrap();
        let cell2 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size { width: length(300.0), height: auto() },
                    ..Default::default()
                },
                &[text2],
            )
            .unwrap();
        let cell3 = taffy
            .new_with_children(
                Style {
                    display: Display::TableCell,
                    size: Size { width: length(200.0), height: auto() },
                    ..Default::default()
                },
                &[text3],
            )
            .unwrap();
        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell1, cell2, cell3],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: length(600.0), height: auto() },
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
                size: Size { width: length(100.0), height: length(20.0) },
                ..Default::default()
            })
            .unwrap();
        let cell = taffy
            .new_with_children(
                Style { display: Display::TableCell, ..Default::default() },
                &[text],
            )
            .unwrap();
        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: length(600.0), height: auto() },
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
                size: Size { width: percent(0.2), height: auto() },
                ..Default::default()
            })
            .unwrap();
        let cell2 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: percent(0.5), height: auto() },
                ..Default::default()
            })
            .unwrap();
        let cell3 = taffy
            .new_leaf(Style {
                display: Display::TableCell,
                size: Size { width: percent(0.3), height: auto() },
                ..Default::default()
            })
            .unwrap();
        let row = taffy
            .new_with_children(
                Style { display: Display::TableRow, ..Default::default() },
                &[cell1, cell2, cell3],
            )
            .unwrap();
        let table = taffy
            .new_with_children(
                Style {
                    display: Display::Table,
                    size: Size { width: length(600.0), height: auto() },
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
