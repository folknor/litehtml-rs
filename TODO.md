# Rendering Pipeline TODO

## Layout issues

- ~~**Default line-height from font metrics**~~: Fixed in 71b2227. Now queries font's actual ascent/descent/line_gap via ttf-parser. Ahem gets 1.0, system fonts ~1.15.
- ~~**Block element centering in table cells**~~: Fixed. When parent has `align="center"` and child is a block element with a fixed px width, auto margins are applied to center it.
- **Auto-width table overflow**: Auto-width tables (no explicit width) can overflow their parent container when content is wider than available space. The CAN-SPAM footer has `&nbsp;`-separated links that create a ~920px line inside an 800px viewport. Needs table max-width constraint or cell text wrapping within column bounds. Blocked on `&nbsp;` handling — preserving non-breaking spaces correctly causes overflow, collapsing them loses intended spacing.
- ~~**Table `align="center"` leaking text-align**~~: Fixed in 42108d7. `align="center"` on `<table>` was incorrectly setting text-align:center on cell content; now only affects table positioning.
- ~~**Leading whitespace at block start**~~: Fixed in 42108d7. HTML indentation was preserved as a leading space in the first text span of a block, causing ~20px offset with Ahem.
- **`<center>` tag**: Not properly centering child tables/content in all cases.
- **MJML `font-size:0` wrapper pattern collapsing child content** (`creatine_header`): The creatine_header fixture has a duplicate mobile nav section at the bottom (same "Klær", "Sko", "Matvarer", "Tilskudd", "Utstyr" links). Chrome renders this as ~270px of height, our pipeline collapses it to ~0px. The root cause is the MJML pattern where wrapper `<td>` elements use `font-size:0px` to collapse whitespace between inline-block children, while the actual content `<div>`s and `<a>` tags inside override with their own font-size (14px) and explicit heights (22px line-height). Our pipeline inherits `font-size:0` down and doesn't properly let child elements override it back for layout purposes. The section has: a `height:20px` spacer div, then 5 nav items each with `line-height:22px` and `font-size:14px` on the `<a>` tags, plus `height:10px` spacer divs — totaling ~270px that Chrome renders but we collapse. The `font-size:0` is on the `<td style="font-size:0px;word-break:break-word;">` and `<div style="font-size:0px;text-align:left;...">` wrappers.

## CSS properties not yet supported

- **vertical-align**: Used for table cell vertical alignment (middle, top, bottom). Partially handled via `align_items` but not fully correct.
- ~~**text-decoration**~~: Fixed in 42108d7. Underline and line-through supported, parsed from CSS and applied per-span. Default underline on `<a>`, `<u>`, `<ins>`; line-through on `<del>`, `<s>`.
- **box-shadow**: Drop shadows.
- **overflow: hidden**: Content clipping.

## Text rendering

- **`line-height` as number**: Parsed and applied, but rounding differs from Chrome (we use `.ceil()`, Chrome may round differently).
- ~~**`line-height` as percentage**~~: Fixed in c059957. `line-height: 150%` was silently dropped; now treated as factor (1.5).
- **`&nbsp;`** and other HTML entities in text nodes: May not be handled correctly in all cases.

## Test emails to fixture-ify

- ~~**gmail_creatine-week.html**~~: Split into 6 focused fixtures: creatine_header (PASS 1.3%), creatine_hero (PASS 1.9%), creatine_products (FAIL 29%), flare_hero (FAIL 18%), flare_products (FAIL 30%), monster_snacks (FAIL 29%). Original monolithic fixture retained as gmail_creatine_week.
- **gmail_steam-purchase.html**: Steam purchase receipt. Deep nested tables, custom @font-face (Motiva Sans), background images on cells.
- **gmail_gullinbursti-dividend.html**: Newsletter. Complex responsive CSS, modern selectors (`:has()`), figure layouts. Most complex.

## Rendering quality

- **Glyph sub-pixel positioning**: Ahem font glyphs land at slightly different positions than Chrome, causing pixel-level diffs even when layout is structurally correct.
