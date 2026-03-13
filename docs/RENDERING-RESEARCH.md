# Rendering Engine Research

## Context

Ratatoskr needs to render HTML email bodies inside an iced GUI. The current approach uses **litehtml** (C++ HTML/CSS engine) via FFI, with **cosmic-text** for font shaping and **tiny-skia** for software rasterization. Profiling with hotpath reveals this is unacceptably slow.

### Performance Target

The widely-accepted UI responsiveness threshold (Nielsen Norman Group, adopted by Google's RAIL model) is **100ms** for interactions to feel instantaneous. For rendering an email when the user clicks on it, we need to be under 100ms. We're currently at **1,260ms** — over 12x too slow.

- **100ms** — feels instantaneous, system responds directly to user input
- **1,000ms** — user notices delay but stays in flow
- **10,000ms** — attention is lost

### Profiling Results (1000.html, ~78KB email)

```
+------------------------------+-------+-----------+-----------+-----------+---------+
| Function                     | Calls | Avg       | P95       | Total     | % Total |
+------------------------------+-------+-----------+-----------+-----------+---------+
| litehtml::render_view        | 2     | 632.19 ms | 698.88 ms | 1.26 s    | 36.76%  |
| litehtml::draw_view          | 2     | 327.02 ms | 380.90 ms | 654.03 ms | 19.02%  |
| litehtml::capture_frame      | 2     | 327.01 ms | 380.90 ms | 654.03 ms | 19.02%  |
| litehtml::rebuild_document   | 2     | 305.17 ms | 317.72 ms | 610.34 ms | 17.75%  |
| pixbuf::text_width           | 27352 | 14.27 µs  | 23.14 µs  | 390.40 ms | 11.35%  |
| pixbuf::draw_text            | 9728  | 29.21 µs  | 58.14 µs  | 284.14 ms | 8.26%   |
| litehtml::unpremultiply_rgba | 2     | 109.65 ms | 143.39 ms | 219.29 ms | 6.38%   |
| pixbuf::draw_solid_fill      | 14    | 4.93 ms   | 36.08 ms  | 68.95 ms  | 2.00%   |
| pixbuf::create_font          | 80    | 120.74 µs | 332.03 µs | 9.66 ms   | 0.28%   |
+------------------------------+-------+-----------+-----------+-----------+---------+
```

**Key findings:**
- **Layout is the bottleneck**, not rasterization. `rebuild_document` (HTML parse + CSS layout) = 610ms, driven by 27K `text_width` FFI round-trips (390ms).
- **Text drawing** = 284ms for 10K `draw_text` calls (cosmic-text shaping + tiny-skia compositing).
- **unpremultiply_rgba** = 219ms doing per-pixel math — pure overhead from premultiplied→straight alpha conversion.
- **Actual tiny-skia drawing** (fills, borders) = only ~70ms. Swapping rasterizers wouldn't help much.
- The fundamental problem is the C++ layout engine making 27K FFI callbacks for font metrics. This architecture cannot be optimized from the Rust side.

---

## litehtml Architecture (what we're replacing)

litehtml is a self-contained C++ HTML/CSS rendering engine (~21K lines of C++) with zero external dependencies. It computes layout and issues drawing commands via callbacks; it does not render anything itself.

### Components

**HTML Parser: Gumbo Parser**
- Vendored C99 library implementing the HTML5 parsing algorithm (~5K lines)
- Hosted at `codeberg.org/gumbo-parser/gumbo-parser`, largely unmaintained
- Handles malformed HTML via HTML5 error recovery
- Flow: `gumbo_parse_with_options()` → GumboOutput tree → converted to litehtml element tree via `create_node()`

**CSS Parser: Custom Implementation**
- Custom-built tokenizer + parser following W3C CSS Syntax Module Level 3
- Tokenizer (~600 lines): IDENT, FUNCTION, AT_KEYWORD, HASH, STRING, NUMBER, PERCENTAGE, DIMENSION tokens
- Parser: `parse_stylesheet()`, `consume_list_of_rules()`, `consume_component_value()`, `consume_style_block_contents()`
- Handles @import, @media, selector parsing, specificity calculation
- Not using any external CSS library

**Layout Engine**
- Multiple formatting contexts:
  - **Block** (`render_item_block`): Normal flow, margins, padding, borders, floats, clears
  - **Inline** (`render_item_inline_context`): Line boxes, text wrapping, vertical alignment
  - **Table** (`render_item_table`): Column/row sizing, border-collapse, auto/fixed width algorithms
  - **Flexbox** (`render_item_flex`): Flex direction, grow/shrink/basis, justify/align
  - **Positioned**: Absolute and fixed positioning
  - **Floats**: Left/right float stacks via `formatting_context`
- No CSS Grid support
- CSS units: px, pt, in, cm, mm, pc, %, em, rem, vw, vh, vmin, vmax

**DocumentContainer Callback Interface**
- Font operations: `create_font()`, `delete_font()`, `text_width()`, `pt_to_px()`, `get_default_font_size/name()`
- Text rendering: `draw_text(hdc, text, font, color, position)`
- Drawing primitives: `draw_solid_fill()`, `draw_linear/radial/conic_gradient()`, `draw_borders()`
- Image handling: `load_image()`, `get_image_size()`, `draw_image()`
- Clipping: `set_clip()`, `del_clip()`
- Events: `on_anchor_click()`, `set_cursor()`
- Metadata: `set_caption()`, `set_base_url()`, `get_viewport()`, `get_media_features()`
- CSS: `import_css()`, `transform_text()`

**Rendering Pipeline**
```
HTML string → gumbo_parse() → GumboOutput tree → litehtml element tree
  → CSS stylesheet parsing → selector matching → property computation (cascade, inheritance, specificity)
  → document::render(width) → render_item tree creation → layout calculation per formatting context
  → document::draw() → DocumentContainer callbacks (draw_text, draw_solid_fill, draw_borders, draw_image, ...)
```

### What makes litehtml suitable for email
- Table layout support (email HTML is 80%+ tables)
- Inline style parsing
- Handles malformed HTML (via Gumbo)
- No JavaScript, no external dependencies
- Relatively small codebase (~21K lines C++ + ~5K lines C for Gumbo)

### What makes litehtml unsuitable
- C++ via FFI — 27K round-trip callbacks per render for text measurement alone
- No way to batch font metric queries
- Custom CSS parser with limited modern CSS support
- Unmaintained Gumbo parser
- Layout algorithms are tightly coupled, hard to optimize incrementally

---

## Rust HTML Parsers

### Tier 1: Production-Grade

**html5ever** (servo/html5ever)
- Latest: v0.38.0 (January 2026), actively maintained by the Servo project
- Downloads: ~4.78M/month — the foundational HTML parser in the Rust ecosystem
- WHATWG HTML5 compliant. Passes all html5lib tokenizer tests; passes most tree-builder tests
- Does NOT produce a DOM itself — uses a `TreeSink` callback trait, you supply the tree implementation
- Common tree backends: markup5ever_rcdom (Rc-based, officially for testing), ego-tree (Vec-backed, cache-friendly, used by scraper), kuchikiki (Brave's maintained fork of kuchiki), dom_query's custom tree
- Excellent malformed HTML handling — implements the HTML5 error-recovery algorithm, same as browsers
- Performance: moderate. ~10-15x slower than simpler parsers like quick-xml, but doing spec-compliant error recovery
- **Best choice for parsing. You'd pair it with a tree backend.**

**scraper**
- Latest: v0.25.0 (December 2025), ~1.24M/month
- Convenience wrapper around html5ever + Servo's `selectors` crate + ego-tree
- `Html::parse_document()` returns a traversable DOM with CSS selector queries
- ego-tree is Vec-backed, efficient, full parent/child/sibling traversal
- Inherits html5ever's spec compliance and error recovery
- **Most practical all-in-one option** — parse HTML, get a DOM tree, query with CSS selectors

**dom_query**
- Latest: v0.26.0 (March 2026), very actively maintained, ~17K/month
- Enhanced fork of `nipper` (jQuery-like API over html5ever)
- Expanded CSS selector support: `:has`, `:has-text`, `:contains`
- Good alternative to scraper with more jQuery-style manipulation

### Tier 2: Useful with Caveats

**tl**
- Latest: v0.7.8 (January 2024) — no updates in 14+ months, 19 open issues
- ~424K/month downloads
- SIMD-accelerated, zero-copy parsing — fastest pure-Rust parser by a wide margin
- **Explicitly not HTML5 compliant.** Silently drops invalid tags rather than error-recovering
- **Not recommended for email** — malformed markup is the norm in email HTML, silent content loss is unacceptable

**html5gum**
- Latest: v0.8.3 (December 2025), ~45K/month
- WHATWG-compliant HTML5 **tokenizer only**, no tree builder
- ~2x slower than quick-xml but faster than html5ever's tokenizer
- Not directly useful — no DOM tree output

**select (select.rs)**
- Latest: v0.6.1 (March 2025), ~58K/month
- html5ever + markup5ever_rcdom wrapper
- Uses older html5ever (v0.26), less maintained than scraper — use scraper instead

**kuchikiki** (Brave)
- Released February 2025, Brave's maintained fork of archived kuchiki
- Reference-counted tree over html5ever
- Viable if you need kuchiki-style API, but scraper/dom_query are more active

### Tier 3: Not Suitable

**lol_html** (Cloudflare)
- v2.7.2 (February 2026), streaming/SAX-like rewriter, no DOM tree — designed for transformation, not rendering

**html_parser / lithtml**
- Pest-based, lightweight, no HTML5 error recovery — too simplistic for email

**browser-tester** (finitefield-org)
- Actively developed (March 2026) but 0 stars, niche testing harness
- Contains a custom HTML parser from scratch (~1295 lines in `src/core_impl/html.rs`)
- Hand-written byte-level scanner, handles comments, void tags, `<script>`, `<noscript>`, `<template>`, optional tag closing
- Post-parse normalization (implied table bodies, head/body elements)
- Partial spec compliance, basic malformed HTML tolerance — not production-grade

**marked** (dekellum)
- v0.3.0 (January 2021) — unmaintained for 5+ years. Vec-backed DOM, interesting architecture, dead project

### Benchmarks

Source: [y21/rust-html-parser-benchmark](https://github.com/y21/rust-html-parser-benchmark) — Criterion benchmarks on Wikipedia (~312 KiB HTML).

| Parser | Time | Throughput | Spec Compliant | Builds DOM |
|--------|------|-----------|----------------|------------|
| **tl** | 629 µs | 496 MiB/s | No | Flat tag list |
| **lol_html** | 789 µs | 396 MiB/s | Partial | No (streaming) |
| **htmlstream** | 1.80 ms | 174 MiB/s | No | No (SAX) |
| **html5ever** | 6.22 ms | 50 MiB/s | Yes (full) | Yes (via TreeSink) |

scraper, dom_query, kuchikiki, select.rs all use html5ever under the hood — their parse performance is effectively html5ever's. They differ in DOM representation and query API overhead, not parse speed.

The 10x gap between tl and html5ever reflects fundamentally different work: tl scans for tag boundaries (SIMD-accelerated, zero-copy) while html5ever builds a spec-compliant parse tree with error recovery, implicit element insertion, and foster parenting.

**For our use case, parse speed is irrelevant.** A 78KB email at html5ever's 50 MiB/s = ~1.5ms. Our 100ms budget is consumed by layout (610ms) and drawing (654ms), not parsing. Correctness on malformed email HTML matters far more than parse throughput.

Cross-language context ([imWildCat/html-parser-benchmark](https://github.com/imWildCat/html-parser-benchmark) — Wikipedia ~1MB + 10K CSS selector queries, Apple M1 Max):

| Runtime | Time |
|---------|------|
| Rust (kuchiki/html5ever) | 8 ms |
| Firefox JS | 4 ms |
| WebKit JS | 5 ms |
| Chromium JS | 11 ms |
| Node.js (jsdom) | 141 ms |
| Go (goquery) | 5,631 ms |

### Verdict

**html5ever** is the only serious option for email HTML parsing. Use it directly with a custom `TreeSink`, or via **scraper** (ego-tree + CSS selectors) for the easiest path to a traversable DOM.

---

## Rust CSS Parsers & Layout Engines

### CSS Parsers

**lightningcss** (Parcel)
- v1.32.0 (March 2026), very active, 7.5K GitHub stars, 1.6M dependents
- Full CSS property value parsing into typed Rust enums using CSS spec grammar
- Built on `cssparser` and `selectors` from Servo
- Can parse inline `style` attributes via `transformStyleAttribute`
- Handles modern colors, nesting, custom properties, calc(), media queries, selectors, shorthands
- No layout, no cascade/specificity resolution — purely parse/transform/serialize
- **Excellent CSS parser layer for email** — parse `<style>` blocks and inline `style=""` into typed values

**cssparser** (Servo)
- CSS Syntax Level 3 tokenization and component value tree building only
- Currently at v0.35.x, active (part of Servo/Stylo ecosystem)
- Does not parse property values into typed representations, does not understand selectors or specificity
- Foundation that both lightningcss and stylo build on — you'd use it indirectly
- **Too low-level on its own**

**selectors** (Servo)
- Parses CSS selectors, computes specificity, matches against a generic element tree
- You implement a trait describing your DOM, it handles matching
- Active, lives inside `servo/stylo` repo
- **Useful if building your own style resolution** — free selector parsing + specificity + matching

### CSS Engines (Cascade + Specificity)

**stylo** (Servo/Firefox)
- The full CSS style engine from Firefox/Servo
- Selector parsing, specificity, cascade, inheritance, property parsing into specified/computed values, media queries, pseudo-elements, `!important`, author/user/UA stylesheet layers
- Usable standalone as a crate with `default-features = false`
- `stylo_taffy` crate demonstrates standalone integration with Taffy for layout
- Moderate-to-heavy weight: ~14 dependencies including `servo_arc`, `cssparser`, `selectors`
- **The most capable option for correct cascade/specificity/inheritance**

### Layout Engines

**taffy** (Dioxus)
- Pure Rust CSS layout engine implementing Block, Flexbox, and CSS Grid
- **No table layout.** Open issue [#467](https://github.com/DioxusLabs/taffy/issues/467) in "Todo" status, no timeline
- No CSS parsing, no cascade — purely layout tree computation
- Active development
- **Missing table layout is a dealbreaker for email HTML**

**yoga-rs**
- Rust FFI bindings to Facebook's Yoga C++ library, flexbox-only
- Largely superseded by Taffy — not suitable

**morphorm** (Vizia)
- Simplified one-pass layout with Row/Column types, designed for UI widgets
- Not suitable for HTML/CSS document layout

### Full-Stack Renderers

**Blitz** (DioxusLabs)
- Modular HTML/CSS rendering: Stylo (cascade) + Taffy (layout) + Parley (text) + Vello (GPU rendering)
- Claims to support table layout, flexbox, grid, block, inline, absolute/fixed positioning
- Explicitly lists email HTML rendering as a target use case
- Pre-alpha: "There are still many bugs and missing features." Aiming for beta end-of-2025, production 2026
- Large dependency footprint (Stylo + Taffy + Parley + winit + reqwest + etc.)
- Could potentially use `blitz-dom` without windowing/networking layers
- **Most promising full-stack option, but not production ready**

**Gosub Engine**
- Pure Rust browser engine with HTML5 tokenizer/parser, CSS3 tokenizer/parser, document tree, rendering
- Plans to be a "standalone library that can be used by other projects"
- Currently "in its infancy" — no usable browser, rendering engine incomplete
- Not ready for use

### CSS Summary

| Crate | Parses CSS | Cascade/Specificity | Layout | Table Layout | Email-ready |
|-------|-----------|-------------------|--------|-------------|-------------|
| lightningcss | Full typed values | No | No | No | Good parser |
| cssparser | Tokens only | No | No | No | Too low-level |
| selectors | Selectors only | Yes (specificity) | No | No | Good for matching |
| stylo | Full (Firefox-grade) | Yes (full cascade) | No | No | Best cascade |
| taffy | No | No | Block/Flex/Grid | No | Missing tables |
| blitz | Via stylo | Via stylo | Block/Flex/Grid/Table | Claimed | Pre-alpha |

### Benchmarks

#### CSS Parsing

LightningCSS (built on Servo's `cssparser`) — parse + transform + minify, single pass:

| File | LightningCSS | esbuild | cssnano |
|------|-------------|---------|---------|
| Bootstrap 4 (~200KB, ~10K lines) | **4.16 ms** | 17.2 ms | 544.8 ms |
| Animate.css | **1.97 ms** | 11.9 ms | 283.1 ms |
| Tailwind CSS (large, ~2MB) | **43.4 ms** | 107.7 ms | 2,198 ms |

LightningCSS is ~4x faster than esbuild and ~130x faster than cssnano. Email CSS is tiny compared to these files — expect sub-millisecond parse times.

Source: [LightningCSS GitHub](https://github.com/parcel-bundler/lightningcss), [GoalSmashers CSS minification benchmark](https://goalsmashers.github.io/css-minification-benchmark/)

#### Style Resolution

Stylo (Firefox/Servo CSS engine):
- Style computation is embarrassingly parallel across DOM nodes. 4 cores → ~4x speedup.
- **2x to 18x faster** than the old Gecko style system depending on page complexity and core count.
- Typical full page restyle: **"a few milliseconds or tens of milliseconds"**
- MotionMark "Multiply" test: **24 ms with 6 threads** for style computation
- Style sharing optimization: for pages with many nodes sharing the same styles (common in email HTML with repeated table cells), Stylo skips computation by reusing previously computed styles
- Capped at 6 threads due to allocator contention at higher thread counts

Source: [Inside a super fast CSS engine: Quantum CSS (Mozilla Hacks)](https://hacks.mozilla.org/2017/08/inside-a-super-fast-css-engine-quantum-css-aka-stylo/)

#### Layout

Taffy (Dioxus — Rust flexbox/grid layout engine):
- 10,000-node tree at depth 14: **~3 ms** (after caching fix — was 17 seconds before, PR #246)
- Linear scaling with node count; Yoga (Meta's C++ flexbox engine) has exponential blowup on certain patterns
- PanGui benchmarks: Taffy was **129x faster** than Yoga on `perpendicular_expand_with_wrap` at 10x node count
- **No table layout support** (open issue [#467](https://github.com/DioxusLabs/taffy/issues/467))

Source: [Taffy GitHub](https://github.com/DioxusLabs/taffy), [PanGui layout benchmarks](https://pangui.io/blog/05-layout-rework-and-benchmarks/)

#### Browser Reference Points

- Chrome style recalculation for a moderately complex page: **~18 ms**
- Per-frame budget at 60fps: **16.7 ms** total for script + style + layout + paint
- Email HTML is simpler than a typical web page — hundreds to low thousands of nodes, minimal CSS

#### What This Means for Us

| Operation | Modern Rust crates | Our litehtml | Ratio |
|-----------|-------------------|-------------|-------|
| CSS parse | <1 ms (lightningcss) | Part of 610ms rebuild | — |
| Style resolution | ~24 ms (stylo, full page) | Part of 610ms rebuild | — |
| Layout | ~3 ms (taffy, 10K nodes) | Part of 610ms rebuild | — |
| Text measurement | Cacheable, batchable | 390ms (27K FFI calls) | — |
| **Total parse+style+layout** | **<30 ms estimated** | **610 ms measured** | **~20x** |

The 610ms `rebuild_document` in litehtml includes HTML parsing + CSS parsing + style resolution + layout + 27K FFI round-trips for `text_width`. Modern Rust crates can do all of this in under 30ms. The FFI round-trips for text measurement are the dominant cost — a native Rust layout engine can cache font metrics and batch text measurement, collapsing the 390ms `text_width` overhead.

No published benchmarks exist for litehtml or Blitz.

### Verdict

No single Rust crate gives you table layout + CSS cascade today. The hard part is **table layout** — the one thing email HTML relies on most, and the one thing missing from Taffy. Blitz is assembling the right pieces but isn't ready.

---

## Ladybird Browser

Ladybird's Rust effort (February 2026) is limited to **LibJS, their JavaScript engine** — specifically the lexer, parser, AST, scope collector, and bytecode code generator. ~25K lines of Rust produced in ~2 weeks using AI-assisted translation (Claude Code and Codex).

**HTML parser, CSS engine, layout engine: NOT rewritten.** All remain C++. The Rust code is not available as standalone crates. It lives within the monorepo and deliberately mimics C++ patterns. C++ remains the primary development language — "a sidetrack that runs for a long time."

**Nothing usable for HTML rendering from Ladybird.**

---

## Options

### Option 1: Port litehtml's layout engine to Rust

Replace each component with a Rust equivalent:
- Gumbo → **html5ever** (drop-in, better maintained, same HTML5 spec compliance)
- Custom CSS parser → **lightningcss** (typed property values, modern CSS support)
- Layout engine → **manual port** of litehtml's ~21K lines of C++ layout code
- DocumentContainer → direct Rust trait, eliminate FFI overhead

**Pros:**
- Mechanical, testable work — litehtml has well-structured layout code
- Can validate against litehtml's own test suite and existing rendering output
- Eliminates the 27K FFI round-trips that cause the performance problem
- Can incrementally improve layout algorithms once in Rust

**Cons:**
- ~21K lines of C++ to port (layout engine is the bulk)
- litehtml's layout algorithms have quirks and limitations
- Still ends up with a custom engine that needs ongoing maintenance

### Option 2: Build on existing Rust components

Assemble: **html5ever** (parsing) + **lightningcss** or **stylo** (CSS) + **custom table layout** + cosmic-text (text) + tiny-skia or vello (rasterization)

**Pros:**
- Uses best-in-class components for each layer
- Stylo gives browser-grade cascade correctness
- Each component is independently maintained

**Cons:**
- Table layout must be built from scratch (the hardest part)
- Integrating stylo is heavyweight
- Essentially building what Blitz is building, but scoped to email

### Option 3: Use Blitz (when ready)

Monitor Blitz development, contribute if possible, adopt when it reaches beta quality.

**Pros:**
- Right architecture (stylo + taffy + parley)
- Claims table layout support
- Active development, well-funded (Dioxus ecosystem)
- Explicitly targets email HTML rendering

**Cons:**
- Pre-alpha today
- Large dependency footprint
- Uses Vello (GPU) — would need architectural changes for iced integration, or use their CPU rendering path
- Timeline uncertain

### Option 4: Optimize litehtml FFI (short-term)

Keep litehtml but reduce FFI overhead:
- Batch `text_width` calls (cache measurements, avoid redundant queries)
- Cache font metrics per font description
- Move `unpremultiply_rgba` to SIMD
- Render in background thread, cache bitmaps per email

**Pros:**
- Minimal code changes
- Can ship improvements immediately
- Buys time for a proper solution

**Cons:**
- Fundamental architecture remains slow
- Still maintaining C++ FFI bridge
- Limited ceiling on improvement

### Option 5: HTML → iced widgets (the COSMIC markdown pattern)

Parse email HTML into an intermediate representation, walk it with a trait to produce native iced widgets (rich_text, container, row, column, table).

**Pros:**
- Native iced performance, no rasterization pipeline
- Text selection, accessibility, theming come free from iced
- No separate rendering engine to maintain

**Cons:**
- Endless edge cases mapping HTML/CSS to iced widget tree
- Table-for-layout is extremely hard to map correctly
- No clear definition of "done"
- CSS cascade/specificity would need custom implementation
- Lots of fidgety iteration

---

## Recommendation

**Short-term (now):** Option 4 — optimize litehtml FFI to make the current approach usable while we build the replacement.

**Medium-term (build):** Option 1 or Option 2 — port litehtml's layout to Rust (using html5ever + lightningcss), or build from components. The choice depends on whether we value litehtml's known-working table layout (port it) vs. cleaner architecture (build from components but face the table layout gap).

**Long-term (watch):** Option 3 — Blitz. If it delivers on table layout and reaches beta, it becomes the obvious choice. Worth contributing to if we're building table layout anyway (contribute to Taffy's table support).

**Avoid:** Option 5 (HTML → iced widgets) — too much fidgety iteration with no convergence guarantee.

---

## Open Questions

1. How complex is email HTML table layout in practice? If it's a small subset of CSS table layout, a purpose-built email table layout engine might be much smaller than a general one.
2. Can we extract and reuse Blitz's table layout implementation even if we don't adopt Blitz wholesale?
3. What does Taffy's table layout issue (#467) actually need? Could we contribute the implementation?
4. Is there a middle ground where we keep litehtml for layout but replace its callbacks with batched/cached Rust implementations?
