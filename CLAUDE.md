# ratatoskr-renderer

Pure Rust HTML email rendering pipeline for Ratatoskr's iced UI.

**Repo**: https://github.com/folknor/rt-renderer-research (private)

## Status

The pipeline scaffold is working. All test emails render in **3-23ms** (pipeline only, excluding rasterization), well under the 100ms target. Currently rendering to tiny-skia pixel buffers and saving PNG output for visual validation; final target is native iced widgets.

The old litehtml C++ renderer has been deleted. See `RENDERING-RESEARCH.md` for why (1,260ms per render, 27K FFI round-trips).

## Pipeline

```
HTML string
  → scraper/html5ever (parse DOM, ~1ms)
  → lightningcss (inline styles + <style> blocks)
  → style resolution (cascade, inheritance, HTML attrs)
  → taffy (layout, ~1-4ms)
  → tiny-skia (scaffold) → iced widgets (final target)
```

Table layout uses a local taffy fork (`/home/folk/Programs/taffy`) with CSS table support (Display::Table, TableRow, TableCell, TableRowGroup).

## Key files

- `src/lib.rs` - Public API: `render()` pipeline function
- `src/css.rs` - CSS parsing, ComputedStyle, property application
- `src/style.rs` - StyleIndex, InheritedStyle, element styling
- `src/tree.rs` - DOM tree building, text measurement, NodeData
- `src/text.rs` - Font system, text attributes, line height
- `src/render.rs` - Pixel rendering, text drawing, rounded rects
- `src/main.rs` - CLI wrapper (clap)
- `Cargo.toml` - Dependencies
- `brokkr.toml` - Brokkr dev tooling config
- `fixtures/<id>/` - Per-fixture directory: source HTML + Chrome reference + pipeline output + diff
- `docs/RENDERING-RESEARCH.md` - Performance analysis, ecosystem survey, benchmarks
- `docs/PIPELINE-PLAN.md` - Implementation plan and work items
- `test-emails/` - HTML test corpus (numbered by complexity + real Gmail emails)
- `test-emails-eml/` - Raw .eml test corpus
- `TODO.md` - Known issues and planned improvements

## Commands

Never use `cargo` directly - use brokkr for everything.

```bash
brokkr check                                             # clippy + tests
brokkr run -- -- --fixture fixtures/foo_test/foo_test.html   # build + render fixture (output in same directory)
brokkr run -- -- test-emails/1.html                      # render without Ahem fixture mode (second -- separates cargo args from binary args)
brokkr run --time -- test-emails/1.html                  # release run with timing output (not stored in db)
brokkr bench run -- test-emails/1.html                   # benchmark (3 runs, best-of, stored in results.db)
brokkr bench run --runs 5 -- test-emails/1.html          # benchmark with 5 runs
brokkr bench run --variant "name" -- test-emails/1.html  # benchmark with variant label
brokkr hotpath -- test-emails/1.html                     # function-level timing profiling
brokkr hotpath --alloc -- test-emails/1.html              # allocation profiling
brokkr results                                           # query benchmark results
brokkr results --compare-last --mode bench               # diff two most recent bench runs
brokkr results --grep "some-flag-value"                   # find runs by any flag in cli_args
brokkr history                                           # browse command history (all projects)
```

### Visual reference testing

Compares pipeline output against Chrome reference renders. Fixtures are defined in `brokkr.toml` under `[litehtml]`.

```bash
brokkr visual --all                                      # run all fixtures
brokkr visual --suite smoke                              # run fixtures tagged "smoke"
brokkr visual text_flow_test                             # run single fixture by ID
brokkr visual --recapture text_flow_test                 # force-regenerate Chrome reference
brokkr list                                              # list fixtures, tags, approval state
brokkr visual-status                                     # dashboard: fixtures vs baselines
brokkr approve text_flow_test                            # record current divergence as baseline (clean tree required)
brokkr report <run_id>                                   # show results for a past run
```

### Fixture preparation (prepare/outline/html-extract)

Creates deterministic, self-contained fixture HTML from raw email sources. See `docs/FIXTURE-PREPROCESSING.md` for the full spec.

```bash
# Step 1: Normalize a raw email - fetches images, replaces with correctly-sized
# gray placeholders, injects Ahem font, strips external @imports, pretty-prints.
# Image cache lives in .brokkr/prepare-cache/.
brokkr prepare test-emails/raw-email.html fixtures/email/email.html

# Step 2: Inspect the prepared HTML structure to find extract selectors.
# Shows section boundaries with content summaries (images, text).
brokkr outline fixtures/email/email.html --selectors
brokkr outline fixtures/email/email.html --depth 8       # deeper nesting
brokkr outline fixtures/email/email.html --full           # no depth limit

# Step 3: Extract a sub-fixture from a prepared email.
# Single section:
brokkr html-extract fixtures/email/email.html \
  --selector "div:nth-of-type(2) > table > tbody > tr > td > div:nth-of-type(3) > div" \
  fixtures/hero/hero.html
# Range of sibling sections (--from/--to):
brokkr html-extract fixtures/email/email.html \
  --from "div:nth-of-type(2) > table > tbody > tr > td > div:nth-of-type(4) > div" \
  --to   "div:nth-of-type(2) > table > tbody > tr > td > div:nth-of-type(7) > div" \
  fixtures/products/products.html
```

**Workflow**: `prepare` → `outline --selectors` → `html-extract` → `visual --recapture` → `visual`

Note: `brokkr test` now runs a single cargo test by name; visual fixture testing lives under `brokkr visual`.

Never hand-edit fixture HTML for Ahem injection or image replacement - always use `prepare`. Hand-crafted test fixtures (text_flow_test, etc.) should also be run through `prepare` to standardize their Ahem injection.

`brokkr bench` requires a clean git tree to store results, but ignores dirty markdown files and `results.db` itself.

**Always commit `.brokkr/`**. The `results.db` inside it is the benchmark history and must be tracked in git.

## Dependencies

- **scraper** 0.27 (html5ever + ego-tree DOM)
- **taffy** local fork (layout: block, flex, grid, table)
- **cosmic-text** 0.19 (text measurement + glyph rasterization)
- **tiny-skia** 0.12 (CPU rasterization, scaffold only)
- **ego-tree** 0.11 (DOM tree traversal)
- **lightningcss** 1.0.0-alpha.72 (CSS parsing - inline styles + stylesheet rules)
- **hotpath** 0.22 (profiling, behind `profile` feature flag)
- **clap** 4 (CLI argument parsing, binary only)

## Bash rules

- Never use sed, find, awk, or complex bash commands
- Never chain commands with &&
- Never chain commands with ;
- Never pipe commands with |
- Never read or write from /tmp. All data lives in the project. `*.png` and `*.json` are gitignored, so render output can safely go in the project directory.
- Never run raw cargo, curl, pkill. Use `brokkr`.

## Code rules

- Every behavioral change to rendering logic (CSS resolution, layout, tree building, pixel rendering) MUST include a code comment explaining what the change does and referencing at least one fixture by ID (e.g. `// Strip leading whitespace at block start (text_decoration_test)`). No uncommented rendering fixes.

## Gotchas

- **HTML `align` attribute**: On `<table>` it means "center the table in its parent" (margin auto). On `<td>`/`<th>` it means `text-align`. Don't treat them the same - this caused a bug where table `align="center"` set text-align on all cell content.
- **CSS `text-decoration` is NOT inherited**: Unlike color/font-size, it doesn't cascade to children. In our pipeline it's tracked per-`RichTextSpan` and resets to false in `InheritedStyle::with_overrides` for each element, then re-applied from CSS or tag defaults (`<a>`, `<u>`, `<del>`, etc.).
- **Leading whitespace in blocks**: HTML indentation whitespace must be stripped for the first text node in a block. The `collect_inline_text` function only preserves a leading space when `!spans.is_empty()` (i.e. between spans, not at block start).
- **Table cell vertical alignment uses `align_content`, not `align_items`**: taffy stretches cells to full row height and reads `align_content` for vertical positioning of cell content. HTML `valign` and CSS `vertical-align` both map to it (CSS wins over the attr); td/th default to `CENTER` because browsers default cells to middle.
- **Presentational attrs must lose to CSS**: `width`/`height`/`valign`/`bgcolor` attrs are only applied when the corresponding CSS property is absent — including keyword values like `height:auto`. Getting this wrong is expensive: a `height` attr overriding `height:auto` gave imgs a definite height, and taffy derived width from height × aspect-ratio, inflating a 600px table to 1200px (creatine_hero).

## Commit rules

- Don't commit pure markdown changes on their own. Bundle them with the code change they relate to, or skip them unless the update is substantive.

## Parent project

Part of **Ratatoskr** email client (`/home/folk/Programs/ratatoskr/`). Migrating from React/Tauri to pure Rust with iced. This renderer is the critical path item.
