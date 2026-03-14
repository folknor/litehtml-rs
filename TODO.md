# Rendering Pipeline TODO

## Waiting on taffy

- **Table cell padding with content-box**: Cell padding-bottom not reflected in row height when cells use `box_sizing: ContentBox`. Causes footer rows to be too short (28px vs 40px expected). Also causes social icon table cells to be 58px wide instead of 46px. Written up and sent to taffy dev.

## Layout issues

- **Default line-height from font metrics**: Currently hardcoded to `font_size * 1.2`. Should query cosmic-text for the font's actual ascent/descent metrics to compute `line-height: normal` correctly. Ahem's correct default is 1.0 (no ascenders/descenders), typical system fonts are ~1.15. This causes pills/badges to be slightly too tall and cumulative vertical drift in paragraphs.
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
