# Native iced Rendering Pipeline

## Goal

Replace litehtml (C++ FFI) with a pure Rust pipeline that renders HTML email directly to iced widgets. No software rasterization, no pixel buffer, no image Handle detour.

**Target: <100ms from HTML string to rendered iced widget tree.**

## Architecture

```
HTML string
  → html5ever (parse into DOM tree)
  → lightningcss (parse <style> blocks + inline style="" attributes)
  → style resolution (match selectors, cascade, compute properties)
  → taffy (compute layout — positions and sizes for every box)
  → iced widget tree (container, rich_text, filled quads for backgrounds/borders)
  → iced renders natively (GPU-accelerated, text selection for free, theming for free)
```

## Pipeline Stages

### Stage 1: HTML → DOM tree
- **Crate**: html5ever (via scraper or custom TreeSink)
- **Input**: HTML string
- **Output**: Traversable DOM tree with element names, attributes, text nodes
- **Status**: Solved — html5ever is mature, ~1.5ms for 78KB email

### Stage 2: CSS → Parsed styles
- **Crate**: lightningcss
- **Input**: `<style>` blocks extracted from DOM, inline `style=""` attributes
- **Output**: Typed CSS property values per declaration
- **Work needed**: Extract stylesheets from DOM, parse inline styles via `transformStyleAttribute`

### Stage 3: Style resolution
- **Crate**: lightningcss or stylo (or custom, email CSS is simple)
- **Input**: Parsed stylesheets + DOM tree
- **Output**: Computed style per DOM node (colors, fonts, margins, padding, display, widths, etc.)
- **Work needed**: Selector matching, cascade, specificity, inheritance
- **Open question**: Is lightningcss enough or do we need stylo's full cascade? Email CSS is mostly inline styles + simple selectors — might not need the full Firefox engine.

### Stage 4: Layout
- **Crate**: taffy
- **Input**: DOM tree + computed styles
- **Output**: Position (x, y) and size (width, height) for every box
- **Work needed**:
  - Map computed CSS properties to taffy style structs
  - Build taffy layout tree mirroring DOM structure
  - Text measurement callback (cosmic-text or iced's built-in text engine)
  - **Table layout**: initially falls back to block display. Real table support is the big milestone.

### Stage 5: Render to iced widgets
- **Input**: DOM tree + computed styles + layout positions/sizes
- **Output**: iced `Element` tree
- **Work needed**:
  - Walk the laid-out tree and emit iced widgets:
    - Block boxes → `container` with absolute positioning
    - Text nodes → `rich_text` or `text` with computed font/color/size
    - Backgrounds → filled quads (renderer `fill_quad`)
    - Borders → quads or custom drawing
    - Images → `iced::widget::image`
  - Handle scrolling (iced `scrollable`)
  - Text selection comes free from iced's text widgets

## Work Items

### Phase 0: Scaffold
- [ ] Create a new example (`examples/pipeline.rs`) that loads an HTML file
- [ ] Add dependencies: html5ever (or scraper), lightningcss, taffy, tiny-skia
- [ ] Define intermediate data structures (DOM node → style → layout box)
- [ ] Render to tiny-skia pixel buffer first (fast to scaffold, self-contained)
- [ ] Display via basic iced image widget or dump to PNG for validation
- [ ] Swap to native iced widgets later once pipeline is proven (Phase 3)

### Phase 1: Parse + Style (get computed styles per node)
- [ ] Parse HTML with html5ever into a traversable DOM
- [ ] Extract `<style>` blocks from DOM
- [ ] Parse stylesheets with lightningcss
- [ ] Parse inline `style=""` attributes with lightningcss
- [ ] Implement basic selector matching + cascade (inline styles win almost always in email)
- [ ] Compute inherited properties (color, font-size, font-family, line-height)
- [ ] Validate: dump computed styles for a test email, compare against browser DevTools

### Phase 2: Layout (get positions and sizes)
- [ ] Map computed styles to taffy `Style` structs
- [ ] Build taffy node tree from DOM
- [ ] Implement text measurement function (cosmic-text or iced font system)
- [ ] Run `taffy::compute_layout()`
- [ ] Tables render as block elements (wrong but functional)
- [ ] Validate: dump layout positions, compare against browser rendering

### Phase 3: Render to iced widgets
- [ ] Walk layout tree, emit iced widgets with absolute positioning
- [ ] Text nodes → `rich_text` with computed font properties
- [ ] Background colors → container with background style
- [ ] Borders → container with border style
- [ ] Images → `iced::widget::image` (reuse existing image fetch infrastructure)
- [ ] Wrap in `scrollable` for overflow
- [ ] Validate: visual comparison against litehtml output and browser rendering

### Phase 4: Table layout
- [ ] Study CSS 2.1 table layout algorithm (https://www.w3.org/TR/CSS2/tables.html)
- [ ] Study taffy's architecture for adding new layout modes
- [ ] Implement table layout (either in taffy as contribution, or standalone)
- [ ] Validate: render table-heavy emails, compare against browser rendering

### Phase 5: Integration
- [ ] Replace litehtml engine with new pipeline in the `Engine` trait
- [ ] Wire up to ratatoskr's email view
- [ ] Performance validation: measure full pipeline against 100ms target
- [ ] Handle edge cases: empty emails, plain text fallback, deeply nested tables

## Open Questions

1. **lightningcss vs stylo for style resolution?** Email CSS is 90% inline styles. A simple cascade (inline > id > class > element, with inheritance) might be all we need. Try lightningcss first, pull in stylo only if we hit correctness issues.

2. **Text measurement: cosmic-text vs iced's text engine?** If we're rendering to iced widgets, we should measure text using the same engine iced uses for rendering. Otherwise measurements won't match. Need to investigate iced's font API.

3. **How to handle absolute positioning in iced?** iced doesn't have CSS-style absolute positioning. Options: canvas widget, custom layout widget, or floating overlay layer.

4. **Table layout scope for email?** Email table layout might be a smaller problem than general CSS tables. Most email tables use explicit widths, no colspan/rowspan nesting beyond 2-3 levels. Could we implement a subset?

5. **Can we reuse the existing `WebView` widget infrastructure?** The `Engine` trait, view management, scrolling, image fetching — all of that can stay. We're just replacing what's behind `Engine`.
