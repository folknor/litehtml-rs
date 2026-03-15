use std::cell::RefCell;
use std::collections::HashMap;

use cosmic_text::{Attrs, Family, FontSystem};

// Shared font system for text measurement during tree building.
thread_local! {
    pub(crate) static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
    /// Cache of font family name → line-height ratio (ascent + descent + line_gap) / units_per_em.
    static LINE_HEIGHT_RATIO_CACHE: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

/// Whether fixture mode is enabled (forces Ahem font)
pub static FIXTURE_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Load the Ahem test font into the font system.
pub fn load_ahem_font() {
    let ahem_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/Ahem.ttf");
    if ahem_path.exists() {
        FONT_SYSTEM.with(|fs| {
            let mut fs = fs.borrow_mut();
            let data = std::fs::read(&ahem_path).expect("Failed to read Ahem.ttf");
            fs.db_mut().load_font_data(data);
        });
    }
}

pub(crate) fn is_fixture_mode() -> bool {
    FIXTURE_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Resolve font family, overriding to Ahem in fixture mode.
pub(crate) fn resolve_font_family(family: &str) -> Family<'_> {
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
pub(crate) fn build_text_attrs(
    family: &str,
    weight: u16,
    italic: bool,
    letter_spacing: Option<f32>,
    font_size: f32,
) -> Attrs<'_> {
    let f = resolve_font_family(family);
    let mut attrs = if is_fixture_mode() {
        Attrs::new()
            .family(f)
            .weight(cosmic_text::Weight::NORMAL)
            .style(cosmic_text::Style::Normal)
    } else {
        Attrs::new()
            .family(f)
            .weight(cosmic_text::Weight(weight))
            .style(if italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            })
    };
    if let Some(ls_px) = letter_spacing {
        // cosmic-text expects letter-spacing in EM units, CSS gives us px
        let fs = font_size.max(1.0);
        attrs = attrs.letter_spacing(ls_px / fs);
    }
    attrs
}

/// Resolve line height: use CSS value if set, otherwise query the font's actual metrics
/// for `line-height: normal` (ascent + descent + line_gap scaled to font size).
pub(crate) fn resolve_line_height(
    font_system: &FontSystem,
    font_size: f32,
    css_line_height: Option<f32>,
    family: &str,
) -> f32 {
    if let Some(lh) = css_line_height {
        return lh.max(1.0);
    }
    let ratio = font_normal_line_height_ratio(font_system, family);
    (font_size * ratio).ceil().max(1.0)
}

/// Get the line-height ratio for `line-height: normal` from the font's metrics.
/// Returns (ascent + |descent| + line_gap) / units_per_em. Cached per resolved family.
fn font_normal_line_height_ratio(font_system: &FontSystem, family: &str) -> f32 {
    // Resolve to actual font name (Ahem in fixture mode)
    let resolved = if is_fixture_mode() { "Ahem" } else { family };

    // Check cache
    let cached = LINE_HEIGHT_RATIO_CACHE.with(|cache| cache.borrow().get(resolved).copied());
    if let Some(ratio) = cached {
        return ratio;
    }

    // Query font metrics via fontdb
    let ratio = query_font_line_height_ratio(font_system, resolved).unwrap_or(1.2);

    LINE_HEIGHT_RATIO_CACHE.with(|cache| {
        cache.borrow_mut().insert(resolved.to_string(), ratio);
    });

    ratio
}

/// Query the font's OS/2 or hhea metrics to compute the normal line-height ratio.
fn query_font_line_height_ratio(font_system: &FontSystem, family: &str) -> Option<f32> {
    let db = font_system.db();
    let fontdb_family = match family {
        "serif" => cosmic_text::fontdb::Family::Serif,
        "sans-serif" | "sans serif" => cosmic_text::fontdb::Family::SansSerif,
        "monospace" => cosmic_text::fontdb::Family::Monospace,
        name => cosmic_text::fontdb::Family::Name(name),
    };
    let query = cosmic_text::fontdb::Query {
        families: &[fontdb_family],
        ..Default::default()
    };
    let face_id = db.query(&query)?;
    db.with_face_data(face_id, |data, index| {
        let face = ttf_parser::Face::parse(data, index).ok()?;
        let units_per_em = face.units_per_em() as f32;
        let ascent = face.ascender() as f32;
        let descent = face.descender().unsigned_abs() as f32;
        let line_gap = face.line_gap().max(0) as f32;
        Some((ascent + descent + line_gap) / units_per_em)
    })?
}

/// Apply text-transform to a string.
pub(crate) fn apply_text_transform(
    text: &str,
    transform: Option<crate::css::TextTransform>,
) -> String {
    use crate::css::TextTransform;
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
