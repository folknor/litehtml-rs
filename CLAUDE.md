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
  → lightningcss (inline styles + <style> blocks)
  → style resolution (cascade, inheritance, HTML attrs)
  → taffy (layout, ~1-4ms)
  → tiny-skia (scaffold) → iced widgets (final target)
```

Table layout is the critical missing piece — taffy doesn't support it (issue #467). Email HTML is 80%+ tables. Currently approximated with flex rows.

## Key files

- `src/main.rs` — The rendering pipeline
- `Cargo.toml` — Dependencies: scraper, taffy, tiny-skia, cosmic-text, ego-tree, lightningcss
- `brokkr.toml` — Brokkr dev tooling config
- `docs/RENDERING-RESEARCH.md` — Performance analysis, ecosystem survey, benchmarks
- `docs/PIPELINE-PLAN.md` — Implementation plan and work items
- `test-emails/` — HTML test corpus (numbered by complexity + real Gmail emails)
- `test-emails-eml/` — Raw .eml test corpus

## Commands

Never use `cargo` directly — use brokkr for everything.

```bash
brokkr check                                             # clippy + tests
brokkr run -- test-emails/1.html                         # build release + run
brokkr run --time -- test-emails/1.html                  # release run with timing output
brokkr bench run -- test-emails/1.html                   # benchmark (3 runs, best-of, stored in results.db)
brokkr bench run --runs 5 -- test-emails/1.html          # benchmark with 5 runs
brokkr bench run --variant "name" -- test-emails/1.html  # benchmark with variant label
brokkr hotpath -- test-emails/1.html                     # function-level timing profiling
brokkr hotpath --alloc -- test-emails/1.html             # allocation profiling
brokkr results                                           # query benchmark results
brokkr results --compare-last --command "bench run"      # diff two most recent runs
brokkr history                                           # browse command history (all projects)
```

`brokkr bench` requires a clean git tree to store results, but ignores dirty markdown files and `results.db` itself.

## Dependencies

- **scraper** 0.25 (html5ever + ego-tree DOM)
- **taffy** 0.8 (layout: block, flex, grid)
- **cosmic-text** 0.18 (text measurement + glyph rasterization)
- **tiny-skia** 0.11 (CPU rasterization, scaffold only)
- **ego-tree** 0.10 (DOM tree traversal)
- **lightningcss** 1.0.0-alpha.71 (CSS parsing — inline styles + stylesheet rules)
- **hotpath** 0.14 (profiling, behind `profile` feature flag)

## Bash rules

- Never use sed, find, awk, or complex bash commands
- Never chain commands with &&
- Never chain commands with ;
- Never pipe commands with |
- Never read or write from /tmp. All data lives in the project.
- Never run raw cargo, curl, pkill. Use `brokkr`.

## Parent project

Part of **Ratatoskr** email client (`/home/folk/Programs/ratatoskr/`). Migrating from React/Tauri to pure Rust with iced. This renderer is the critical path item.
