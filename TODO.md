# Rendering Pipeline TODO

## Layout issues

- **Default line-height from font metrics**: Currently hardcoded to `font_size * 1.2`. Should query cosmic-text for the font's actual ascent/descent metrics to compute `line-height: normal` correctly. Ahem's correct default is 1.0 (no ascenders/descenders), typical system fonts are ~1.15. This causes pills/badges to be slightly too tall and cumulative vertical drift in paragraphs.
- **Block element centering in table cells**: `align="center"` on a `<td>` sets `justify_content: Center` but this doesn't center block children (like `<img display:block>`) within table cells. The header logo is 32px off-center. May need `margin: 0 auto` on centered block children, or a different approach for table cell child alignment.
- **Auto-width table overflow**: Auto-width tables (no explicit width) can overflow their parent container when content is wider than available space. The CAN-SPAM footer has `&nbsp;`-separated links that create a ~920px line inside an 800px viewport. Needs table max-width constraint or cell text wrapping within column bounds. Blocked on `&nbsp;` handling — preserving non-breaking spaces correctly causes overflow, collapsing them loses intended spacing.
- **`<center>` tag**: Not properly centering child tables/content in all cases.

## CSS properties not yet supported

- **vertical-align**: Used for table cell vertical alignment (middle, top, bottom). Partially handled via `align_items` but not fully correct.
- **text-decoration**: underline, line-through, etc. Links show no underline.
- **box-shadow**: Drop shadows.
- **overflow: hidden**: Content clipping.

## Text rendering

- **`line-height` as number**: Parsed and applied, but rounding differs from Chrome (we use `.ceil()`, Chrome may round differently).
- **`&nbsp;`** and other HTML entities in text nodes: May not be handled correctly in all cases.

## Rendering quality

- **Glyph sub-pixel positioning**: Ahem font glyphs land at slightly different positions than Chrome, causing pixel-level diffs even when layout is structurally correct.
