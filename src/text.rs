use std::cell::RefCell;

use cosmic_text::{Attrs, Family, FontSystem};

// Shared font system for text measurement during tree building.
thread_local! {
    pub(crate) static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
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

/// Resolve line height: use CSS value if set, otherwise default to font_size * 1.2.
pub(crate) fn resolve_line_height(font_size: f32, css_line_height: Option<f32>) -> f32 {
    // CSS line-height:normal uses the font's actual metrics.
    // Browser default is typically ~1.2 for most fonts, 1.0 for Ahem.
    css_line_height.unwrap_or((font_size * 1.2).ceil()).max(1.0)
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
