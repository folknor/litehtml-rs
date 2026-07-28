# Visual Harness Improvements

Findings and proposed changes from the 2026-07-28 review of the visual reference
testing setup. Covers three codebases: brokkr's compare/capture code, brokkr's
prepare.js, and this renderer. Written ahead of the brokkr harness wiring work.

## Status (2026-07-28)

Items 1-9 landed in brokkr (commit 13782ac), with three notes:

- **Item 5 was half-done already**: the approval table stored
  `element_match_pct` all along; only `determine_status` ignored it. The
  ratchet fires on a >0.5pp drop below the approved value.
- **Item 4 deviation**: offenders are not stored in the results db, so the
  `report` command can't show them retroactively. Instead the full worst-first
  list persists as `fixtures/<id>/offenders.txt` (removed on clean runs) and
  the top 10 print under `FAIL_THRESHOLD`/`REGRESSION` rows in `visual`.
- **Item 8 resolution**: the pre-screenshot viewport resize was removed
  entirely, not just the cap - `fullPage` doesn't need it, and keeping the
  measurement viewport means the JSON dump and PNG can't disagree on
  viewport-dependent layout. (First recapture may shift references for
  fixtures with vh-dependent styling, if any exist.)

prepare.js dep bumps (cheerio 1.2, image-size 2.0) landed with an image-size
2.x API fix (brokkr 32de5b5); not yet exercised against the corpus. Items
10-16 (prepare.js fidelity) and 17-23 remain open; next steps per the order
table are validating the honest scores against the corpus, then item 21
(approve baselines).

## What the review established

The overall methodology is sound: deterministic fixtures (Ahem + natural-size
gray placeholders), Chrome as oracle, dual structural/visual metrics, both
renderers consuming the identical prepared file. The element matcher already
joins by dom path (`html>body[0]>div[2]` format, identical convention on both
sides) - it is NOT positional. The misleading scores come from a small number
of specific defects, not from the design.

Evidence, gathered by sequence-aligning and path-joining the JSON dumps by hand:

- **monster_snacks reports 29% element match but is ~97% correct**: after
  dropping `br` and head-only tags, both dumps contain exactly 744 elements and
  721 match within 3px. The remaining 23 are zero-height empty `tbody`/`tr`
  where Chrome reports container width and we report `w=0`.
- **gmail_creatine_week reports 13%**: of ~1500 failing pairs, 1388 were pure
  y-drift (x/w/h all fine). Integer-vs-fractional line-height rounding
  accumulates to ~6px over the 10,400px email, blowing past the absolute 2px
  tolerance for every element below the drift point. Only ~84 elements had
  genuinely wrong geometry - the footer social icons (fixed in ff8c785).

## Changes in brokkr: `src/litehtml/compare.rs`

Highest leverage, all small, all independently testable against the current
fixture corpus.

1. **Parent-relative geometry comparison** (replaces absolute `POS_TOLERANCE`).
   Every element's parent path is `path.rsplit_once('>')`; both maps are already
   keyed by path, so the parent box lookup is free. Compare x/y relative to the
   parent's box instead of absolute document coordinates. Cumulative drift stops
   compounding and the 2px tolerance becomes meaningful. ~10 lines, no capture
   or dump changes.
2. **Filter `br` from the denominator.** Chrome emits `br` boxes; the pipeline
   folds them into rich-text leaves and never will. Every one lands in
   `chrome_only` and drags the score down. `LayoutElement` already deserializes
   `tag` (currently dead_code) - filter alongside the existing `is_head_path`.
3. **Skip zero-height elements from scoring.** Empty `tbody`/`tr` differ only
   in an invisible convention (Chrome: container width with h=0; us: 0x0).
   Skip elements where the reference has `h == 0`.
4. **Report worst offenders.** Matched-but-wrong-geometry elements are counted
   but never listed, and they are the single most useful debugging output (the
   hand-rolled version pointed straight at the social icon region out of 2,193
   elements). Everything needed is in scope in `compare_elements`: print the
   top ~10 by max delta with their paths, in `test` output and `report`.
5. **Element-match ratchet.** `Status::Regression` currently only fires on
   pixel diff vs the approved baseline. Store the element match pct in the
   approval too and regress on a drop.

Pixel comparison is fine as is. Known biases, acceptable for now: the
denominator is the whole canvas, so sparse/whitespace-heavy emails score
flattering pixel percentages (only matters when comparing across fixtures -
compare against each fixture's own history instead); anti-aliased edges exceed
the per-channel fuzz of 13, so a 1px pre-diff dilation would forgive AA noise
if it ever becomes the dominant term.

## Changes in brokkr: capture script (`src/litehtml/cmd.rs`, `CAPTURE_JS`)

6. **Await font loading before measuring.** `networkidle0` does not cover
   data-URI `@font-face` decoding, so `getBoundingClientRect` can run before
   Ahem is active and capture fallback-font geometry. Add
   `await page.evaluate(() => document.fonts.ready)` after `goto`, before the
   measurement pass. This is the one real nondeterminism risk in the capture.
7. **Record capture metadata.** chrome.json carries no Chrome/Puppeteer
   version, viewport, or scale factor - a silent Chrome update changing
   fractional line heights would look like mysterious drift across all fixtures
   at once. Write a sidecar `chrome.meta.json` (browser version, viewport,
   deviceScaleFactor, timestamp) so neither JSON parser needs format changes.
8. **Delete the dead viewport-height cap.** `Math.min(bodyHeight, 10000)` is
   ignored by `fullPage: true` screenshots; it only confuses readers.
9. **Bump the `goto` timeout** (currently 10s) - multi-MB data-URI emails are
   close to it.

## Changes in brokkr: `scripts/litehtml-prepare/prepare.js`

Fidelity of the corpus itself. Everything here is self-consistent today (both
renderers see the same prepared file), but these make prepared fixtures drift
from how the raw email actually renders.

10. **Stop overwriting author `width`/`height` attrs with fetched natural
    dimensions.** `el.attr("width", String(dims.width))` runs unconditionally;
    an email with `<img width="300">` whose CDN image is 600x400 becomes
    `width="600"` and lays out differently from the real email. Natural size
    is already carried by the placeholder PNG itself - only set attrs when the
    author did not provide them.
11. **Normalize pre-existing data-URI images too.** JPEG/GIF data URIs survive
    prepare untouched: Chrome renders the real image, the pipeline cannot even
    read the dimensions (it only parses PNG headers) - a guaranteed divergence
    no renderer work can fix. Re-encode all image content to gray PNGs,
    using image-size on the decoded buffer.
12. **Make Ahem injection robust.** `* { font-family: 'ahem' !important }` has
    zero specificity and loses to any author `!important` font rule. Since
    prepare already rewrites style text, rewrite every `font-family`
    declaration to `'ahem'` (and neutralize other `@font-face` rules) instead
    of fighting the cascade.
13. **Pretty-printer whitespace.** Re-serializing long inline runs onto
    indented lines introduces whitespace between inline siblings that had none
    (`<span>a</span><span>b</span>` gains a space); whitespace-only text nodes
    between elements are dropped. Keep inline runs verbatim regardless of
    length. Inter-inline-element whitespace is exactly what the renderer has
    fixture-driven logic about (creatine_products).
14. **Fix the `img { background-color }` hack regex.** `/img\s*\{/` matches
    substrings (`.desktop img {`, `.bigimg {`) and can leave an orphaned
    selector prefix that corrupts the following rule. Anchor with
    `(^|[\s,}])img\s*\{`.
15. **Small fetch improvements**: negative-cache failed fetches (dead CDN URLs
    currently cost a 10s timeout on every re-prepare) and send a User-Agent
    (some image CDNs 403 the node default).
16. **Document the extract limitation**: sibling `<td>` stubs are preserved but
    `<colgroup>` and other rows are not, so auto-layout column widths in a
    sub-fixture can legitimately differ from the full email.

## Changes in this repo (renderer / dump)

17. ~~**Reduce cumulative line-height drift.**~~ Fixed 2026-07-28, and the
    original attribution was wrong: the drift source was the `border` CSS
    shorthand being silently dropped (`border: 1px solid` on MJML button tds,
    2px per button), not line-height rounding. Fractional line heights survive
    to taffy, whose whole-pixel rounding doesn't accumulate error. After adding
    `border`/`border-width`/`border-color` shorthand handling, dy stays within
    ±0.7px over gmail_creatine_week's 10,400px (`scripts/drift_analysis.py`).
18. **No dump changes needed for matching.** The pipeline dump already emits
    `path` in the compatible format; head elements are filtered on the compare
    side. Leave the zero-height `tbody` convention alone - harness-side
    skipping (item 3) is the right fix, not making taffy report fake widths.
19. Known renderer gaps that will show up once scores are trustworthy, both in
    TODO.md: `valign` on `<tr>` cascading to cells, and the taffy fork clamping
    tables to specified width instead of max(specified, min-content)
    (gmail_creatine_week social icon boxes are 50px vs Chrome's 80px).

## Methodology

20. **Gate on the ratchet, not absolute thresholds.** Per-fixture absolute
    thresholds can't be meaningful across fixtures (denominator bias, email
    length). The approve-baseline + regression-delta model is right: absolute
    numbers are for humans on the dashboard, gates are "worse than approved by
    more than noise". Extend the ratchet to element match (item 5).
21. **Approve baselines after eyeballing.** Nothing is approved today
    (`brokkr visual-status` shows all unapproved), so `Status::Regression` can
    never fire and only threshold gates are active. Once items 1-3 land and
    scores are honest, eyeball each fixture's diff.png and approve, then treat
    every future regression as loud.
22. **Re-prepare the corpus after prepare.js changes.** Items 10-13 change
    prepared output; fixtures and Chrome references must be regenerated
    together (`prepare` -> `visual --recapture`) and re-approved. Bundle this
    with the planned fixture migration in TODO.md rather than doing it twice.
23. **Longer term, weight element mismatches by box area** so a hero image
    counts more than a spacer div, making the element score track what a human
    sees in the diff.

## Suggested order

| Step | Items | Where |
|------|-------|-------|
| 1 | 1, 2, 3 (honest element scores) | brokkr compare.rs |
| 2 | 6 (font race) | brokkr cmd.rs |
| 3 | 4 (worst offenders in report) | brokkr compare.rs/cmd.rs |
| 4 | 21 (approve baselines) | workflow |
| 5 | 10-14 + 22 (corpus fidelity + re-prepare) | prepare.js + fixtures |
| 6 | 5, 7 (ratchets, metadata) | brokkr |
| 7 | 17, 19 (renderer drift + gaps) | this repo + taffy fork |
