use lightningcss::properties::display::Display as CssDisplay;
use lightningcss::properties::font::{
    FontFamily as CssFontFamily, FontSize as CssFontSize, FontStyle as CssFontStyle,
    FontWeight as CssFontWeight,
};
use lightningcss::properties::Property;
use lightningcss::stylesheet::ParserOptions;
use lightningcss::values::color::CssColor;
use lightningcss::values::length::LengthPercentage as CssLengthPercentage;

/// Computed style for a node, extracted from inline CSS.
#[derive(Debug, Clone, Default)]
pub(crate) struct ComputedStyle {
    pub(crate) display_none: bool,
    pub(crate) display_inline_block: bool,
    pub(crate) width_px: Option<f32>,
    pub(crate) height_px: Option<f32>,
    pub(crate) width_pct: Option<f32>,
    pub(crate) height_pct: Option<f32>,
    pub(crate) max_width_px: Option<f32>,
    pub(crate) padding_top: Option<f32>,
    pub(crate) padding_bottom: Option<f32>,
    pub(crate) padding_left: Option<f32>,
    pub(crate) padding_right: Option<f32>,
    pub(crate) margin_top: Option<f32>,
    pub(crate) margin_bottom: Option<f32>,
    pub(crate) margin_left: Option<f32>,
    pub(crate) margin_right: Option<f32>,
    pub(crate) margin_left_auto: bool,
    pub(crate) margin_right_auto: bool,
    pub(crate) background_color: Option<(u8, u8, u8, u8)>,
    pub(crate) color: Option<(u8, u8, u8, u8)>,
    pub(crate) font_size: Option<f32>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_weight: Option<u16>,
    pub(crate) font_style_italic: bool,
    pub(crate) border_top: Option<f32>,
    pub(crate) border_bottom: Option<f32>,
    pub(crate) border_left: Option<f32>,
    pub(crate) border_right: Option<f32>,
    pub(crate) border_top_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_bottom_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_left_color: Option<(u8, u8, u8, u8)>,
    pub(crate) border_right_color: Option<(u8, u8, u8, u8)>,
    pub(crate) line_height: Option<f32>,
    pub(crate) line_height_factor: Option<f32>,
    pub(crate) text_align: Option<TextAlign>,
    pub(crate) white_space_nowrap: bool,
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) text_transform: Option<TextTransform>,
    pub(crate) table_layout_fixed: bool,
    pub(crate) border_radius: Option<f32>,
    pub(crate) border_radius_pct: Option<f32>,
    pub(crate) text_decoration_underline: Option<bool>,
    pub(crate) text_decoration_line_through: Option<bool>,
    pub(crate) vertical_align: Option<VerticalAlign>,
    pub(crate) height_auto: bool,
}

/// Resolved `vertical-align` keyword. Used both for table cell content
/// alignment (mapped to taffy `align_content`) and for centering
/// inline-block children in flex fallback containers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextTransform {
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

/// Convert a CssColor to RGBA tuple.
pub(crate) fn css_color_to_rgba(color: &CssColor) -> Option<(u8, u8, u8, u8)> {
    match color {
        CssColor::RGBA(rgba) => Some((rgba.red, rgba.green, rgba.blue, rgba.alpha)),
        other => {
            // Try converting to RGB for named colors, hsl, etc.
            if let Ok(CssColor::RGBA(rgba)) = other.to_rgb() {
                return Some((rgba.red, rgba.green, rgba.blue, rgba.alpha));
            }
            None
        }
    }
}

/// Extract px value from a CssLengthPercentage.
pub(crate) fn lp_to_px(lp: &CssLengthPercentage) -> Option<f32> {
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
pub(crate) fn lp_to_pct(lp: &CssLengthPercentage) -> Option<f32> {
    match lp {
        CssLengthPercentage::Percentage(p) => Some(p.0),
        _ => None,
    }
}

/// Apply a single lightningcss Property to our ComputedStyle.
pub(crate) fn apply_property(style: &mut ComputedStyle, prop: &Property) {
    use lightningcss::properties::border::BorderSideWidth;
    use lightningcss::properties::size::{MaxSize, Size};
    use lightningcss::values::length::LengthPercentageOrAuto;
    match prop {
        Property::Display(d) => {
            use lightningcss::properties::display::{
                DisplayInside, DisplayKeyword, DisplayOutside, DisplayPair,
            };
            match d {
                CssDisplay::Keyword(DisplayKeyword::None) => {
                    style.display_none = true;
                }
                CssDisplay::Pair(DisplayPair {
                    outside: DisplayOutside::Inline,
                    inside: DisplayInside::FlowRoot,
                    ..
                }) => {
                    style.display_inline_block = true;
                }
                // display:inline-table flows horizontally like inline-block;
                // the element keeps table layout internally. MJML social
                // icons rely on this to sit side by side (gmail_creatine_week)
                CssDisplay::Pair(DisplayPair {
                    outside: DisplayOutside::Inline,
                    inside: DisplayInside::Table,
                    ..
                }) => {
                    style.display_inline_block = true;
                }
                _ => {}
            }
        }
        Property::Width(Size::LengthPercentage(lp)) => {
            if let Some(px) = lp_to_px(lp) {
                style.width_px = Some(px);
            } else if let Some(pct) = lp_to_pct(lp) {
                style.width_pct = Some(pct);
            }
        }
        Property::Height(Size::LengthPercentage(lp)) => {
            if let Some(px) = lp_to_px(lp) {
                style.height_px = Some(px);
            } else if let Some(pct) = lp_to_pct(lp) {
                style.height_pct = Some(pct);
            }
        }
        Property::Height(Size::Auto) => {
            style.height_auto = true;
        }
        Property::MaxWidth(MaxSize::LengthPercentage(lp)) => {
            style.max_width_px = lp_to_px(lp);
        }
        Property::PaddingTop(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.padding_top = lp_to_px(lp);
        }
        Property::PaddingBottom(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.padding_bottom = lp_to_px(lp);
        }
        Property::PaddingLeft(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.padding_left = lp_to_px(lp);
        }
        Property::PaddingRight(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.padding_right = lp_to_px(lp);
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
        Property::MarginTop(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.margin_top = lp_to_px(lp);
        }
        Property::MarginBottom(LengthPercentageOrAuto::LengthPercentage(lp)) => {
            style.margin_bottom = lp_to_px(lp);
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
                        use lightningcss::stylesheet::PrinterOptions;
                        use lightningcss::traits::ToCss;
                        name.to_css_string(PrinterOptions::default())
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string()
                    }
                    CssFontFamily::Generic(g) => format!("{g:?}").to_lowercase(),
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
                    // Try px first, fall back to percentage as factor
                    // (line-height: 150% → factor 1.5) (footer_footer_test)
                    if let Some(px) = lp_to_px(lp) {
                        style.line_height = Some(px);
                    } else if let Some(pct) = lp_to_pct(lp) {
                        style.line_height_factor = Some(pct);
                    }
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
        Property::VerticalAlign(va) => {
            use lightningcss::properties::font::VerticalAlign as CssVerticalAlign;
            use lightningcss::properties::font::VerticalAlignKeyword;
            if let CssVerticalAlign::Keyword(kw) = va {
                style.vertical_align = match kw {
                    VerticalAlignKeyword::Top => Some(VerticalAlign::Top),
                    VerticalAlignKeyword::Middle => Some(VerticalAlign::Middle),
                    VerticalAlignKeyword::Bottom => Some(VerticalAlign::Bottom),
                    // Taffy tables have no true baseline alignment; top is the
                    // closest approximation for same-font cell content
                    // (creatine_hero)
                    VerticalAlignKeyword::Baseline => Some(VerticalAlign::Top),
                    _ => None,
                };
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
        // The `border` shorthand was silently dropped: MJML buttons put
        // `border: 1px solid #000` on the td, so every button lost 2px of
        // width/height and the whole email drifted ~2px shorter per button
        // (gmail_creatine_week). `border: none`/`hidden` parses with a
        // default medium width that must not become a visible edge.
        Property::Border(b) => {
            use lightningcss::properties::border::LineStyle;
            let w = if matches!(b.style, LineStyle::None | LineStyle::Hidden) {
                None
            } else {
                match &b.width {
                    BorderSideWidth::Length(l) => l.to_px(),
                    _ => None,
                }
            };
            let c = css_color_to_rgba(&b.color);
            style.border_top = w;
            style.border_bottom = w;
            style.border_left = w;
            style.border_right = w;
            style.border_top_color = c;
            style.border_bottom_color = c;
            style.border_left_color = c;
            style.border_right_color = c;
        }
        // Width/color shorthands, same silent-drop issue as `border` above
        // (gmail_creatine_week)
        Property::BorderWidth(bw) => {
            let side = |w: &BorderSideWidth| match w {
                BorderSideWidth::Length(l) => l.to_px(),
                _ => None,
            };
            style.border_top = side(&bw.top);
            style.border_right = side(&bw.right);
            style.border_bottom = side(&bw.bottom);
            style.border_left = side(&bw.left);
        }
        Property::BorderColor(bc) => {
            style.border_top_color = css_color_to_rgba(&bc.top);
            style.border_right_color = css_color_to_rgba(&bc.right);
            style.border_bottom_color = css_color_to_rgba(&bc.bottom);
            style.border_left_color = css_color_to_rgba(&bc.left);
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
        Property::BorderTopLeftRadius(size, _)
        | Property::BorderTopRightRadius(size, _)
        | Property::BorderBottomLeftRadius(size, _)
        | Property::BorderBottomRightRadius(size, _)
            if style.border_radius.is_none() && style.border_radius_pct.is_none() =>
        {
            if let Some(px) = lp_to_px(&size.0) {
                style.border_radius = Some(px);
            } else if let Some(pct) = lp_to_pct(&size.0) {
                style.border_radius_pct = Some(pct);
            }
        }
        Property::BorderRadius(br, _) => {
            // Shorthand: take top-left as uniform radius
            let size = &br.top_left.0;
            if let Some(px) = lp_to_px(size) {
                style.border_radius = Some(px);
            } else if let Some(pct) = lp_to_pct(size) {
                style.border_radius_pct = Some(pct);
            }
        }
        Property::TextDecorationLine(line, _) => {
            use lightningcss::properties::text::TextDecorationLine;
            style.text_decoration_underline = Some(line.contains(TextDecorationLine::Underline));
            style.text_decoration_line_through =
                Some(line.contains(TextDecorationLine::LineThrough));
        }
        Property::TextDecoration(td, _) => {
            use lightningcss::properties::text::TextDecorationLine;
            style.text_decoration_underline =
                Some(td.line.contains(TextDecorationLine::Underline));
            style.text_decoration_line_through =
                Some(td.line.contains(TextDecorationLine::LineThrough));
        }
        _ => {}
    }
}

/// Parse inline style attribute using lightningcss.
pub(crate) fn parse_inline_style(css: &str) -> ComputedStyle {
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
pub(crate) fn parse_css_color(val: &str) -> Option<(u8, u8, u8, u8)> {
    // Use lightningcss for proper parsing, with a color property wrapper
    let css = format!("color: {val}");
    let style = parse_inline_style(&css);
    style.color
}

/// Parse a CSS value as px (for HTML attributes like width="600").
pub(crate) fn parse_css_value_px(val: &str) -> Option<f32> {
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
pub(crate) fn parse_css_value_pct(val: &str) -> Option<f32> {
    let css = format!("width: {val}");
    let style = parse_inline_style(&css);
    style.width_pct
}

/// Apply HTML attributes commonly used in email HTML (bgcolor, width, align, color).
pub(crate) fn apply_html_attrs(
    mut style: ComputedStyle,
    el: &scraper::node::Element,
) -> ComputedStyle {
    if let Some(bgcolor) = el.attr("bgcolor")
        && style.background_color.is_none()
    {
        style.background_color = parse_css_color(bgcolor);
    }
    if let Some(color) = el.attr("color")
        && style.color.is_none()
    {
        style.color = parse_css_color(color);
    }
    if let Some(width) = el.attr("width")
        && style.width_px.is_none()
        && style.width_pct.is_none()
    {
        if let Some(pct) = parse_css_value_pct(width) {
            style.width_pct = Some(pct);
        } else if let Some(px) = parse_css_value_px(width) {
            style.width_px = Some(px);
        }
    }
    // height:auto in CSS must win over the height attr (presentational hint),
    // otherwise MJML imgs with width:100%;height:auto keep their attr height
    // and ignore the aspect ratio when the width is constrained (creatine_hero)
    if let Some(height) = el.attr("height")
        && style.height_px.is_none()
        && style.height_pct.is_none()
        && !style.height_auto
    {
        style.height_px = parse_css_value_px(height);
    }
    style
}
