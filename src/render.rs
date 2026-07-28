use std::collections::HashMap;

use cosmic_text::{Metrics, Shaping, SwashCache};
use taffy::prelude::*;

use crate::css::TextAlign;
use crate::text::{build_text_attrs, resolve_line_height, FONT_SYSTEM};
use crate::tree::{NodeData, RichTextSpan, TextMeasure};

pub(crate) fn render_node(
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
        if let Some((r, g, b, a)) = data.background_color
            && a > 0
        {
            let paint = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(
                    tiny_skia::Color::from_rgba8(r, g, b, a),
                ),
                anti_alias: data.border_radius.is_some()
                    || data.border_radius_pct.is_some(),
                ..Default::default()
            };
            let radius = data
                .border_radius
                .or_else(|| data.border_radius_pct.map(|pct| w.min(h) * pct));
            if let Some(radius) = radius {
                if let Some(path) = rounded_rect_path(x, y, w, h, radius) {
                    pixmap.fill_path(
                        &path,
                        &paint,
                        tiny_skia::FillRule::Winding,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            } else if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                pixmap.fill_rect(
                    rect,
                    &paint,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        }

        // Draw image placeholder (gray box matching Chrome's img { background-color: #d0d0d0 })
        if data.is_img
            && let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h)
        {
            let paint = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(
                    tiny_skia::Color::from_rgba8(208, 208, 208, 255),
                ),
                anti_alias: false,
                ..Default::default()
            };
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }

        // Draw <hr> as a gray line
        if data.is_hr
            && let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, 1.0)
        {
            let paint = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(
                    tiny_skia::Color::from_rgba8(180, 180, 180, 255),
                ),
                anti_alias: false,
                ..Default::default()
            };
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }

        // Draw borders
        let draw_border = |pixmap: &mut tiny_skia::Pixmap,
                           bx: f32,
                           by: f32,
                           bw: f32,
                           bh: f32,
                           color: Option<(u8, u8, u8, u8)>| {
            let (r, g, b, a) = color.unwrap_or((200, 200, 200, 255));
            if let Some(rect) = tiny_skia::Rect::from_xywh(bx, by, bw, bh) {
                let paint = tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(
                        tiny_skia::Color::from_rgba8(r, g, b, a),
                    ),
                    anti_alias: false,
                    ..Default::default()
                };
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }
        };
        if let Some(bw) = data.border_top
            && bw > 0.0
        {
            draw_border(pixmap, x, y, w, bw, data.border_top_color);
        }
        if let Some(bw) = data.border_bottom
            && bw > 0.0
        {
            draw_border(pixmap, x, y + h - bw, w, bw, data.border_bottom_color);
        }
        if let Some(bw) = data.border_left
            && bw > 0.0
        {
            draw_border(pixmap, x, y, bw, h, data.border_left_color);
        }
        if let Some(bw) = data.border_right
            && bw > 0.0
        {
            draw_border(
                pixmap,
                x + w - bw,
                y,
                bw,
                h,
                data.border_right_color,
            );
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

pub(crate) fn draw_text(
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
        let line_height =
            resolve_line_height(&fs, font_size, data.line_height, &data.font_family);
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut fs, metrics);
        // Use container width for text wrapping
        let available_width = container_width.max(1.0);
        buffer.set_size(Some(available_width), Some(line_height * 20.0));

        if let Some(ref spans) = data.rich_spans {
            let rich: Vec<(&str, cosmic_text::Attrs)> = spans
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let span_fs = s.font_size.max(1.0);
                    let span_lh =
                        resolve_line_height(&fs, span_fs, data.line_height, &s.font_family);
                    let mut attrs = build_text_attrs(
                        &s.font_family,
                        s.font_weight,
                        s.font_italic,
                        s.letter_spacing,
                        span_fs,
                    )
                    .metadata(i);
                    if (span_fs - font_size).abs() > 0.1 {
                        attrs = attrs.metrics(Metrics::new(span_fs, span_lh));
                    }
                    (s.text.as_str(), attrs)
                })
                .collect();
            let default_attrs = build_text_attrs(
                &data.font_family,
                data.font_weight,
                data.font_italic,
                data.letter_spacing,
                font_size,
            );
            buffer.set_rich_text(rich, &default_attrs, Shaping::Advanced, None);
        } else {
            let attrs = build_text_attrs(
                &data.font_family,
                data.font_weight,
                data.font_italic,
                data.letter_spacing,
                font_size,
            );
            buffer.set_text(text, &attrs, Shaping::Advanced, None);
        }
        buffer.shape_until_scroll(&mut fs, false);

        let (cr, cg, cb, ca) = data.text_color;
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
            for glyph in run.glyphs {
                // Look up per-glyph color (with alpha) from rich text spans
                let (gr, gg, gb, ga) = if let Some(ref spans) = data.rich_spans {
                    let idx = glyph.metadata;
                    if idx < spans.len() {
                        spans[idx].color
                    } else {
                        (cr, cg, cb, ca)
                    }
                } else {
                    (cr, cg, cb, ca)
                };
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(image) =
                    swash_cache.get_image_uncached(&mut fs, physical.cache_key)
                {
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
                                        let glyph_alpha = image.data[i];
                                        // Combine glyph coverage with text color alpha
                                        let alpha = ((glyph_alpha as u16 * ga as u16)
                                            / 255)
                                            as u8;
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

            // Draw text-decoration lines (underline, line-through) per span
            if let Some(ref spans) = data.rich_spans {
                draw_decoration_lines(
                    pixmap, &run, spans, draw_x, draw_y, baseline_y, font_size,
                );
            }
        }
    });
}

/// Draw underline and/or line-through decoration lines for a layout run.
/// Groups consecutive glyphs by span to draw continuous lines.
fn draw_decoration_lines(
    pixmap: &mut tiny_skia::Pixmap,
    run: &cosmic_text::LayoutRun,
    spans: &[RichTextSpan],
    draw_x: i32,
    draw_y: i32,
    baseline_y: i32,
    font_size: f32,
) {
    let pix_w = pixmap.width() as i32;
    let pix_h = pixmap.height() as i32;
    // Thickness: ~1/16th of font size, minimum 1px
    let thickness = (font_size / 16.0).ceil().max(1.0) as i32;
    // Underline offset: below baseline, ~1/8th font size below
    let underline_offset = (font_size / 8.0).ceil() as i32;
    // Line-through: roughly at middle of x-height (≈ 0.4 × font_size above baseline)
    let strikethrough_offset = -(font_size * 0.3).ceil() as i32;

    // Group consecutive glyphs by span index
    let glyphs = &run.glyphs;
    if glyphs.is_empty() {
        return;
    }

    let mut i = 0;
    while i < glyphs.len() {
        let span_idx = glyphs[i].metadata;
        let (has_underline, has_line_through) = if span_idx < spans.len() {
            (spans[span_idx].underline, spans[span_idx].line_through)
        } else {
            (false, false)
        };

        if !has_underline && !has_line_through {
            i += 1;
            continue;
        }

        // Find the extent of consecutive glyphs in this span
        let start_x = draw_x + glyphs[i].x as i32;
        let mut end_x = start_x + glyphs[i].w as i32;
        let mut j = i + 1;
        while j < glyphs.len() && glyphs[j].metadata == span_idx {
            end_x = draw_x + glyphs[j].x as i32 + glyphs[j].w as i32;
            j += 1;
        }

        let (r, g, b, a) = if span_idx < spans.len() {
            spans[span_idx].color
        } else {
            (0, 0, 0, 255)
        };

        if has_underline {
            draw_decoration_rect(
                pixmap.data_mut(),
                pix_w,
                pix_h,
                start_x,
                draw_y + baseline_y + underline_offset,
                end_x - start_x,
                thickness,
                r,
                g,
                b,
                a,
            );
        }
        if has_line_through {
            draw_decoration_rect(
                pixmap.data_mut(),
                pix_w,
                pix_h,
                start_x,
                draw_y + baseline_y + strikethrough_offset,
                end_x - start_x,
                thickness,
                r,
                g,
                b,
                a,
            );
        }

        i = j;
    }
}

/// Draw a filled rectangle for text decoration (underline/line-through).
#[allow(clippy::too_many_arguments)]
fn draw_decoration_rect(
    data: &mut [u8],
    pix_w: i32,
    pix_h: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    for dy in 0..height {
        let py = y + dy;
        if py < 0 || py >= pix_h {
            continue;
        }
        for dx in 0..width {
            let px = x + dx;
            if px >= 0 && px < pix_w {
                blend_pixel(data, pix_w as u32, px as u32, py as u32, r, g, b, a);
            }
        }
    }
}

/// Build a rounded rectangle path using cubic bezier curves for corners.
pub(crate) fn rounded_rect_path(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
) -> Option<tiny_skia::Path> {
    // Clamp radius to half the smaller dimension
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r < 0.5 {
        // Degenerate to regular rect
        return tiny_skia::PathBuilder::from_rect(tiny_skia::Rect::from_xywh(x, y, w, h)?)
            .into();
    }
    // Kappa: cubic bezier control point distance for a quarter circle
    let k = r * 0.5522848;
    let mut pb = tiny_skia::PathBuilder::new();
    // Start at top-left, after the radius
    pb.move_to(x + r, y);
    // Top edge
    pb.line_to(x + w - r, y);
    // Top-right corner
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    // Right edge
    pb.line_to(x + w, y + h - r);
    // Bottom-right corner
    pb.cubic_to(
        x + w,
        y + h - r + k,
        x + w - r + k,
        y + h,
        x + w - r,
        y + h,
    );
    // Bottom edge
    pb.line_to(x + r, y + h);
    // Bottom-left corner
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    // Left edge
    pb.line_to(x, y + r);
    // Top-left corner
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn blend_pixel(
    data: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
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
