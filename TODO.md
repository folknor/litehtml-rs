# Rendering Pipeline TODO

## Layout issues

- ~~**Default line-height from font metrics**~~: Fixed in 71b2227. Now queries font's actual ascent/descent/line_gap via ttf-parser. Ahem gets 1.0, system fonts ~1.15.
- ~~**Block element centering in table cells**~~: Fixed. When parent has `align="center"` and child is a block element with a fixed px width, auto margins are applied to center it.
- **Auto-width table overflow**: Auto-width tables (no explicit width) can overflow their parent container when content is wider than available space. The CAN-SPAM footer has `&nbsp;`-separated links that create a ~920px line inside an 800px viewport. Needs table max-width constraint or cell text wrapping within column bounds. Blocked on `&nbsp;` handling - preserving non-breaking spaces correctly causes overflow, collapsing them loses intended spacing.
- ~~**Table `align="center"` leaking text-align**~~: Fixed in 42108d7. `align="center"` on `<table>` was incorrectly setting text-align:center on cell content; now only affects table positioning.
- ~~**Leading whitespace at block start**~~: Fixed in 42108d7. HTML indentation was preserved as a leading space in the first text span of a block, causing ~20px offset with Ahem.
- **`<center>` tag**: Not properly centering child tables/content in all cases.
- **Specified table width smaller than content min-content** (`gmail_creatine_week` footer social icons): CSS auto table layout treats a specified width as a minimum - Chrome grows `<table style="width:30px">` containing a 60px img to 60px wide (used width = max(specified, min-content)). Our taffy fork clamps to the specified width, so the icon tables render 50px wide instead of 80px (icons still draw at 60px, overflowing their box). Fix belongs in the taffy fork's table algorithm.
- ~~**Header nav `<a>` links 10px too tall**~~ (`gmail_creatine_week` y≈142): Fixed. The flex fallback for inline content left taffy's default `align-items: stretch`, inflating auto-height inline-blocks (the nav separator `|` links) to the tallest sibling. Atomic inlines never stretch; the fallback now uses `align-items: start`, which matches since stretch and baseline both start items at the line top for same line-height content. creatine_header 90%→96%, creatine_products and flare_products 95%→96% element match.
- ~~**MJML `font-size:0` wrapper divs too tall**~~ (`gmail_creatine_week` y≈1341/1707/2766/2864/4400/5480/7383/8504/9336): Fixed by the same `align-items: start` change as the nav links below - the too-tall divs were stretched inline-block MJML columns, not text bugs. After the fix the only remaining mismatches on this fixture are the footer social-icon area (taffy fork table width clamp, see above) and sub-pixel noise.
- ~~**`width:100%` images not constrained by container in MJML tables**~~ (`creatine_hero`, `creatine_products`): Fixed. Root cause was in `apply_html_attrs`: the `height="925"` attr overrode CSS `height:auto`, giving the img a definite height; combined with the intrinsic aspect ratio, taffy derived width = 925 × ratio = 1200px as the cell's intrinsic contribution, inflating the entire table chain to 1200px. CSS `height:auto` now wins over the presentational attr, so the img resolves `width:100%` against its 600px cell and derives height from the aspect ratio. This took every MJML fixture from expected-fail to pass (creatine_hero 43.7% → 2.0% pixel diff).
- ~~**MJML `font-size:0` wrapper pattern collapsing child content**~~ (`creatine_header`): Fixed (description was stale - the collapse itself no longer reproduced). The remaining divergence was `StyleIndex::collect_rules` serializing important declarations with `to_css_string(false, ...)`, stripping the `!important` flag; when the rule text was merged with inline styles, the inline declaration won. MJML relies on `!important` media rules beating inline styles (`.menu-desktop-padding { padding-left:20px !important }` vs inline `padding:0 10px 10px`), which is what makes the nav links wide enough to wrap onto two rows in Chrome. Now serialized with the flag preserved; creatine_header at 1.1% pixel diff / 90% element match.

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

## Fixture migration to `brokkr litehtml prepare` / `extract`

All fixtures need to be regenerated using the new `prepare` and `extract` commands (see `docs/FIXTURE-PREPROCESSING.md`). This replaces the manual Ahem injection, 1x1 base64 image hacks, and hand-extracted sub-fixtures with a deterministic pipeline that produces correctly-sized image placeholders and preserves layout context.

### Full email fixtures (run `prepare` on source, recapture Chrome reference)

- [ ] **gmail_creatine_week**: Source: `test-emails/gmail_creatine-week.html`. Prepare, replace fixture HTML, recapture, re-approve.
- [ ] **gmail_gpu_price_changes**: Source: `test-emails/gmail_gpu-price-changes.html` (verify source location). Prepare, replace, recapture.

### Extracted sub-fixtures (run `extract` on prepared full email, recapture)

All extracted from prepared `gmail_creatine_week`. Need to determine correct CSS selectors for each section.

- [ ] **creatine_header**: Extract header/nav section. Determine selector.
- [ ] **creatine_hero**: Extract hero product section (image + text). Determine selector.
- [ ] **creatine_products**: Extract creatine product grid. Determine selector.
- [ ] **flare_hero**: Extract flare pants hero section. Determine selector.
- [ ] **flare_products**: Extract flare product grid. Determine selector.
- [ ] **monster_snacks**: Extract monster snacks product section. Determine selector.

### Hand-crafted test fixtures (run `prepare` to standardize Ahem injection)

These don't have external images but need the Ahem injection standardized (remove manually embedded WOFF2, let `prepare` inject from shared source).

- [ ] **text_flow_test**: Prepare, recapture.
- [ ] **table_test**: Prepare, recapture.
- [ ] **footer_test**: Prepare, recapture.
- [ ] **header_test**: Prepare, recapture. Has one image placeholder - verify sizing.
- [ ] **border_test**: Prepare, recapture.
- [ ] **footer_footer_test**: Prepare, recapture.
- [ ] **text_decoration_test**: Prepare, recapture.
- [ ] **line_height_normal_test**: Prepare, recapture.

### After migration

- [ ] Update `brokkr.toml` thresholds and expected statuses based on new baselines.
- [ ] Remove old `data-fixture="ahem"` style blocks from any remaining fixtures.
- [ ] Verify all fixtures pass `brokkr litehtml test --all` with updated baselines.

## Test emails to fixture-ify

- ~~**gmail_creatine-week.html**~~: Split into 6 focused fixtures: creatine_header (PASS 1.3%), creatine_hero (PASS 1.9%), creatine_products (PASS 1.3%), flare_hero (PASS 1.8%), flare_products (PASS 1.5%), monster_snacks (PASS 5.0%). Original monolithic fixture retained as gmail_creatine_week.
- **gmail_steam-purchase.html**: Steam purchase receipt. Deep nested tables, custom @font-face (Motiva Sans), background images on cells.
- **gmail_gullinbursti-dividend.html**: Newsletter. Complex responsive CSS, modern selectors (`:has()`), figure layouts. Most complex.

## Dependencies

- **cssparser pinned to 0.33**: lightningcss 1.0.0-alpha.71 depends on cssparser 0.33. Upgrading cssparser to 0.37 causes version conflicts. Check if a newer lightningcss release unblocks the upgrade before release.

## Rendering quality

- **Glyph sub-pixel positioning**: Ahem font glyphs land at slightly different positions than Chrome, causing pixel-level diffs even when layout is structurally correct.
