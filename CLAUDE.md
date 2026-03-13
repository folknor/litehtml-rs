# ratatoskr-renderer

Pure Rust HTML email rendering pipeline for Ratatoskr's iced UI.

**Repo**: https://github.com/folknor/rt-renderer-research (private)

## Status

The pipeline scaffold is working. All test emails render in **3-23ms** (pipeline only, excluding rasterization), well under the 100ms target. Currently rendering to tiny-skia pixel buffers for validation; final target is native iced widgets.

The old litehtml C++ renderer has been deleted. See `RENDERING-RESEARCH.md` for why (1,260ms per render, 27K FFI round-trips).

## Pipeline

```
HTML string
  → scraper/html5ever (parse DOM, ~1ms)
  → custom CSS parser (inline styles + <style> blocks, ~5ms)
  → style resolution (cascade, inheritance, HTML attrs)
  → taffy (layout, ~1-4ms)
  → tiny-skia (scaffold) → iced widgets (final target)
```

Table layout is the critical missing piece — taffy doesn't support it (issue #467). Email HTML is 80%+ tables. Currently approximated with flex rows.

## Key files

- `pipeline/src/main.rs` — The rendering pipeline
- `pipeline/Cargo.toml` — Dependencies: scraper, taffy, tiny-skia, cosmic-text, ego-tree
- `docs/RENDERING-RESEARCH.md` — Performance analysis, ecosystem survey, benchmarks
- `docs/PIPELINE-PLAN.md` — Implementation plan and work items
- `test-emails/` — HTML test corpus (numbered by complexity + real Gmail emails)
- `test-emails-eml/` — Raw .eml test corpus

## Commands

```bash
cd pipeline
cargo build                                              # build
cargo run -- ../test-emails/1.html                       # run on test email
cargo run --release -- ../test-emails/1.html             # release benchmark
```

## Dependencies

- **scraper** 0.25 (html5ever + ego-tree DOM)
- **taffy** 0.8 (layout: block, flex, grid)
- **cosmic-text** 0.18 (text measurement + glyph rasterization)
- **tiny-skia** 0.11 (CPU rasterization, scaffold only)
- **ego-tree** 0.10 (DOM tree traversal)

## Parent project

Part of **Ratatoskr** email client (`/home/folk/Programs/ratatoskr/`). Migrating from React/Tauri to pure Rust with iced. This renderer is the critical path item.
