# Rendering Pipeline TODO

## Layout issues

- ~~**Default line-height from font metrics**~~: Fixed in 71b2227. Now queries font's actual ascent/descent/line_gap via ttf-parser. Ahem gets 1.0, system fonts ~1.15.
- ~~**Block element centering in table cells**~~: Fixed. When parent has `align="center"` and child is a block element with a fixed px width, auto margins are applied to center it.
- **Auto-width table overflow**: Auto-width tables (no explicit width) can overflow their parent container when content is wider than available space. The CAN-SPAM footer has `&nbsp;`-separated links that create a ~920px line inside an 800px viewport. Needs table max-width constraint or cell text wrapping within column bounds. Blocked on `&nbsp;` handling - preserving non-breaking spaces correctly causes overflow, collapsing them loses intended spacing.
- ~~**Table `align="center"` leaking text-align**~~: Fixed in 42108d7. `align="center"` on `<table>` was incorrectly setting text-align:center on cell content; now only affects table positioning.
- ~~**Leading whitespace at block start**~~: Fixed in 42108d7. HTML indentation was preserved as a leading space in the first text span of a block, causing ~20px offset with Ahem.
- **`<center>` tag**: Not properly centering child tables/content in all cases.
- ~~**Specified table width smaller than content min-content**~~ (`gmail_creatine_week` footer social icons): Fixed in the taffy fork (a41c9f8), two defects: (A) block.rs took a table child's specified width as its intrinsic contribution without measuring, bypassing the table algorithm's max(specified, min-content) growth; (B) table cell heights were measured with InherentSize so a `height:30px` style capped the row at 30 instead of flooring it (CSS 2.1 §17.5.3 makes cell height a minimum). Social icons now match Chrome within 0.4px (outer td 80×68, icons 60×60, no overflow). Known remaining fork limitation: `height` on `<tr>` itself is ignored (rows never read their own style - same gap as `valign` on `<tr>` above); documented as B7/B8 in the fork's TABLE-REVIEW.md.
- ~~**Header nav `<a>` links 10px too tall**~~ (`gmail_creatine_week` y≈142): Fixed. The flex fallback for inline content left taffy's default `align-items: stretch`, inflating auto-height inline-blocks (the nav separator `|` links) to the tallest sibling. Atomic inlines never stretch; the fallback now uses `align-items: start`, which matches since stretch and baseline both start items at the line top for same line-height content. creatine_header 90%→96%, creatine_products and flare_products 95%→96% element match.
- ~~**MJML `font-size:0` wrapper divs too tall**~~ (`gmail_creatine_week` y≈1341/1707/2766/2864/4400/5480/7383/8504/9336): Fixed by the same `align-items: start` change as the nav links below - the too-tall divs were stretched inline-block MJML columns, not text bugs. After the fix the only remaining mismatches on this fixture are the footer social-icon area (taffy fork table width clamp, see above) and sub-pixel noise.
- ~~**`width:100%` images not constrained by container in MJML tables**~~ (`creatine_hero`, `creatine_products`): Fixed. Root cause was in `apply_html_attrs`: the `height="925"` attr overrode CSS `height:auto`, giving the img a definite height; combined with the intrinsic aspect ratio, taffy derived width = 925 × ratio = 1200px as the cell's intrinsic contribution, inflating the entire table chain to 1200px. CSS `height:auto` now wins over the presentational attr, so the img resolves `width:100%` against its 600px cell and derives height from the aspect ratio. This took every MJML fixture from expected-fail to pass (creatine_hero 43.7% → 2.0% pixel diff).
- ~~**MJML `font-size:0` wrapper pattern collapsing child content**~~ (`creatine_header`): Fixed (description was stale - the collapse itself no longer reproduced). The remaining divergence was `StyleIndex::collect_rules` serializing important declarations with `to_css_string(false, ...)`, stripping the `!important` flag; when the rule text was merged with inline styles, the inline declaration won. MJML relies on `!important` media rules beating inline styles (`.menu-desktop-padding { padding-left:20px !important }` vs inline `padding:0 10px 10px`), which is what makes the nav links wide enough to wrap onto two rows in Chrome. Now serialized with the flag preserved; creatine_header at 1.1% pixel diff / 90% element match.

## gmail_gullinbursti_dividend remaining gaps (FAIL, 32.2%/76%)

Fixed so far: figure-table +906 width blowup (taffy column-width fix), em/unitless line-height inheritance, ul/ol UA defaults. Remaining ~172px height shortfall and offenders:

- **Em lengths resolve against a hardcoded 16px base** (`lp_to_px` in css.rs): margins/paddings/widths declared in `em` ignore the element's font size. Substack's typography sets em margins on p/h2/h3, accounting for most of the remaining shortfall (~105px "own" on the typography container). Fix requires resolving em at style-application time when the element's font size is known - a refactor of ComputedStyle to keep em values unresolved until `with_overrides`/`element_style`.
- **Header nav link** at dx +182 and a −20 nav table (y≈478): unidentified, likely related centering.
- **fs=18 div −20** (y≈2786): one line-height class not yet diagnosed.

## CSS properties not yet supported

- ~~**vertical-align**~~: Mostly fixed. On table cells, CSS `vertical-align` and HTML `valign` (top/middle/bottom) now map to taffy `align_content`, which is what the table algorithm reads for cell content positioning; cells default to CENTER matching the browser default of middle. `baseline` is approximated as top. Remaining gap: `valign` on `<tr>` should cascade to all cells in the row but is currently ignored (common in email HTML; no current fixture exercises it).
- ~~**text-decoration**~~: Fixed in 42108d7. Underline and line-through supported, parsed from CSS and applied per-span. Default underline on `<a>`, `<u>`, `<ins>`; line-through on `<del>`, `<s>`.
- **box-shadow**: Drop shadows.
- **overflow: hidden**: Content clipping.

## Text rendering

- ~~**`line-height` as number**~~: Resolved as minor. CSS line heights are kept fractional end-to-end and taffy's whole-pixel rounding absorbs them without cumulative drift - the ~6px y-drift on gmail_creatine_week previously blamed on line-height rounding was actually the dropped `border` shorthand (see below). After that fix the drift profile is within ±0.7px over 10,400px (`scripts/drift_analysis.py`). Residual per-element ±1px wobble (a 32.2px Chrome box dumped as 32 or 33) is rounding noise, not drift. The `.ceil()` only applies on the `line-height: normal` path where Ahem's 1.0 ratio makes it a no-op in fixtures.
- ~~**`line-height` as percentage**~~: Fixed in c059957. `line-height: 150%` was silently dropped; now treated as factor (1.5).
- **`&nbsp;`** and other HTML entities in text nodes: May not be handled correctly in all cases.

## Visual harness: element-match scores are misleading (for the brokkr wiring work)

Analysis from 2026-07-28 (chrome.json vs pipeline json, sequence-aligned by tag): the low element-match percentages on the big fixtures are comparison artifacts, not layout bugs.

- **monster_snacks reports 29% element match, but layout is ~97% correct**: after dropping `<br>` and head-only tags from both dumps, both have exactly 744 elements and 721 align within 3px. The score collapses because Chrome's dump emits `<br>` elements as boxes while our pipeline folds them into rich-text leaves, so any positional/sequential matcher derails at the first `<br>` run.
- **Empty `tbody`/`tr` width convention differs**: Chrome reports zero-height structural elements at container width (`w=590 h=0`), we report `w=0 h=0`. Invisible either way, but counts as a mismatch.
- ~~**Cumulative y-drift on long emails**~~: Root-caused and fixed 2026-07-28. The drift was NOT integer-vs-fractional line-height rounding: the `border` CSS shorthand (`border: 1px solid #000` on MJML button tds) was silently dropped in `apply_property`, losing 2px of cell height per button. Three buttons in the flow accounted for the ~6px. With the shorthand applied, dy stays within ±0.7px over the full 10,400px. `scripts/drift_analysis.py` joins the chrome/pipeline dumps by path and reports drift sources and the per-band drift profile.
- ~~**Our dump includes `head`/`style`/`meta`** with zero boxes; Chrome's doesn't.~~ Fixed 2026-07-28: `dump_json` now skips `display:none` subtrees (head content, `<style>` inside `<body>`, `.mobile-hide` etc.). Element match improved on every fixture; line_height_normal_test and flare_hero hit 100%.
- **Chrome-only boxes for inline elements folded into rich text** (`a`, `em`, `u`, `del`, ...): the pipeline folds inline text elements into rich-text spans, so Chrome's dump has boxes ours never will - same class as `br` (already filtered in brokkr). This is what keeps text-heavy fixtures low (text_decoration_test 63%, footer_footer_test 69%). Either brokkr extends the `br` filter to all folded inline tags in `chrome_only`, or the dump learns to emit synthetic boxes for folded spans (real work, but would let the harness verify link geometry).

Suggested harness fixes: match by dom_path instead of sequence, skip `br`/head-only tags and zero-height structural elements, and compare y positions with a proportional (not absolute) tolerance.

All of the above landed in brokkr 2026-07-28 (docs/HARNESS-IMPROVEMENTS.md, since deleted, has the full history in git). One future idea from that review not yet done anywhere: weight element mismatches by box area, so a hero image counts more than a spacer div and the element score tracks what a human sees in the diff.

## ~~Fixture migration to `brokkr litehtml prepare` / `extract`~~

Completed 2026-07-28, after prepare.js fidelity items 10-16 landed in brokkr (8a961d9). All 16 fixtures regenerated: both full emails re-prepared from `test-emails/` sources, the six sub-fixtures re-extracted from the prepared gmail_creatine_week (header divs 1-2, hero div 3, creatine products divs 4-7, flare hero div 8, flare products divs 9-12, monster snacks divs 13-19 - `brokkr outline --selectors` suggestions need a `body >` anchor prefix or they match nested MJML column divs), and the eight hand-crafted fixtures run through `prepare` in place. Chrome references recaptured, all diffs eyeballed, baselines approved. `brokkr.toml` updated: everything `expected = "pass"`, element threshold 90 across the board (current worst: 96), pixel thresholds 6-10%, stale waivers and notes removed. Suite state at approval: ten fixtures at 100% element match, none below 96%; pixel 0.2-5.6%.

## Test emails to fixture-ify

- ~~**gmail_creatine-week.html**~~: Split into 6 focused fixtures: creatine_header (PASS 1.3%), creatine_hero (PASS 1.9%), creatine_products (PASS 1.3%), flare_hero (PASS 1.8%), flare_products (PASS 1.5%), monster_snacks (PASS 5.0%). Original monolithic fixture retained as gmail_creatine_week.
- **gmail_steam-purchase.html**: Steam purchase receipt. Deep nested tables, custom @font-face (Motiva Sans), background images on cells.
- **gmail_gullinbursti-dividend.html**: Newsletter. Complex responsive CSS, modern selectors (`:has()`), figure layouts. Most complex.

## Dependencies

- **cssparser pinned to 0.33**: lightningcss 1.0.0-alpha.71 depends on cssparser 0.33. Upgrading cssparser to 0.37 causes version conflicts. Check if a newer lightningcss release unblocks the upgrade before release.

## Rendering quality

- **Glyph sub-pixel positioning**: Ahem font glyphs land at slightly different positions than Chrome, causing pixel-level diffs even when layout is structurally correct. Tested 2026-07-28: feeding fractional positions into `glyph.physical()` (quarter-pixel subpixel bins + swash AA) was a wash (±0.2pp per fixture) because taffy rounds layout to whole pixels, so our glyph origins are already integers - the edge noise comes from Chrome's *fractional layout*, not our rasterization. Fixing it means disabling taffy's layout rounding end-to-end (fractional dump output, full re-baseline); only worth it as a deliberate experiment.
