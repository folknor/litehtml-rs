# Rendering Pipeline TODO

## Layout issues

- ~~**Default line-height from font metrics**~~: Fixed in 71b2227. Now queries font's actual ascent/descent/line_gap via ttf-parser. Ahem gets 1.0, system fonts ~1.15.
- ~~**Block element centering in table cells**~~: Fixed. When parent has `align="center"` and child is a block element with a fixed px width, auto margins are applied to center it.
- **Auto-width table overflow**: Auto-width tables (no explicit width) can overflow their parent container when content is wider than available space. The CAN-SPAM footer has `&nbsp;`-separated links that create a ~920px line inside an 800px viewport. Needs table max-width constraint or cell text wrapping within column bounds. Blocked on `&nbsp;` handling — preserving non-breaking spaces correctly causes overflow, collapsing them loses intended spacing.
- ~~**Table `align="center"` leaking text-align**~~: Fixed in 42108d7. `align="center"` on `<table>` was incorrectly setting text-align:center on cell content; now only affects table positioning.
- ~~**Leading whitespace at block start**~~: Fixed in 42108d7. HTML indentation was preserved as a leading space in the first text span of a block, causing ~20px offset with Ahem.
- **`<center>` tag**: Not properly centering child tables/content in all cases.

## CSS properties not yet supported

- **vertical-align**: Used for table cell vertical alignment (middle, top, bottom). Partially handled via `align_items` but not fully correct.
- ~~**text-decoration**~~: Fixed in 42108d7. Underline and line-through supported, parsed from CSS and applied per-span. Default underline on `<a>`, `<u>`, `<ins>`; line-through on `<del>`, `<s>`.
- **box-shadow**: Drop shadows.
- **overflow: hidden**: Content clipping.

## Text rendering

- **`line-height` as number**: Parsed and applied, but rounding differs from Chrome (we use `.ceil()`, Chrome may round differently).
- ~~**`line-height` as percentage**~~: Fixed in c059957. `line-height: 150%` was silently dropped; now treated as factor (1.5).
- **`&nbsp;`** and other HTML entities in text nodes: May not be handled correctly in all cases.

## Rendering quality

- **Glyph sub-pixel positioning**: Ahem font glyphs land at slightly different positions than Chrome, causing pixel-level diffs even when layout is structurally correct.
