# Native iced Rendering Pipeline

## Goal

Replace litehtml (C++ FFI) with a pure Rust pipeline that renders HTML email directly to iced widgets. No software rasterization, no pixel buffer, no image Handle detour.

**Target: <100ms from HTML string to rendered iced widget tree.**

Currently achieving 3-23ms for the pipeline (parse → layout → rasterize), well under target.

## Architecture

```
HTML string
  → scraper/html5ever (parse DOM, ~1ms)
  → lightningcss (inline styles + <style> blocks)
  → style resolution (cascade, inheritance, HTML attrs)
  → taffy (layout, ~1-4ms)
  → tiny-skia (current) → iced widgets (final target)
```

## Current state

### Done
- HTML parsing with scraper/html5ever
- CSS parsing with lightningcss (inline styles, `<style>` blocks, shorthand expansion)
- Style resolution: selector matching, cascade, inheritance, HTML attribute fallbacks
- Layout with taffy: block, flex, table (via local fork)
- Text measurement and rendering with cosmic-text (rich text spans, per-span font metrics)
- Rasterization to tiny-skia pixel buffers
- Visual reference testing against Chrome (`brokkr litehtml test --all`)
- Library/binary split (`src/lib.rs` + `src/main.rs`)

### Remaining: pipeline to iced widgets
- Replace tiny-skia rasterization with iced widget tree emission
- Text nodes → `rich_text` with computed font properties
- Background colors → container with background style
- Borders → container with border style
- Border-radius → container with rounded border
- Images → `iced::widget::image`
- Wrap in `scrollable` for overflow
- Wire up to ratatoskr's `Engine` trait
- Performance validation end-to-end

See `TODO.md` for known rendering issues (line-height, centering, table overflow).
