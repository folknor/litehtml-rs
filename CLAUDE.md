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

- `src/lib.rs` — Public API: `render()` pipeline function
- `src/css.rs` — CSS parsing, ComputedStyle, property application
- `src/style.rs` — StyleIndex, InheritedStyle, element styling
- `src/tree.rs` — DOM tree building, text measurement, NodeData
- `src/text.rs` — Font system, text attributes, line height
- `src/render.rs` — Pixel rendering, text drawing, rounded rects
- `src/main.rs` — CLI wrapper (clap)
- `Cargo.toml` — Dependencies
- `brokkr.toml` — Brokkr dev tooling config
- `fixtures/fixtures.toml` — Visual test fixture manifest
- `fixtures/src/` — Fixture HTML files
- `fixtures/reference/` — Chrome reference screenshots + layout JSON
- `docs/RENDERING-RESEARCH.md` — Performance analysis, ecosystem survey, benchmarks
- `docs/PIPELINE-PLAN.md` — Implementation plan and work items
- `test-emails/` — HTML test corpus (numbered by complexity + real Gmail emails)
- `test-emails-eml/` — Raw .eml test corpus
- `TODO.md` — Known issues and planned improvements

## Commands

Never use `cargo` directly — use brokkr for everything.

```bash
brokkr check                                             # clippy + tests
brokkr run -- --fixture fixtures/src/foo.html            # build release + render fixture (output: fixtures/src/foo_pipeline.png + .json)
brokkr run -- test-emails/1.html                         # render without Ahem fixture mode
brokkr run --time -- test-emails/1.html                  # release run with timing output (not stored in db)
brokkr bench run -- test-emails/1.html                   # benchmark (3 runs, best-of, stored in results.db)
brokkr bench run --runs 5 -- test-emails/1.html          # benchmark with 5 runs
brokkr bench run --variant "name" -- test-emails/1.html  # benchmark with variant label
brokkr hotpath -- test-emails/1.html                     # function-level timing profiling
brokkr hotpath --alloc -- test-emails/1.html              # allocation profiling
brokkr results                                           # query benchmark results
brokkr results --compare-last --command "bench run"      # diff two most recent runs
brokkr history                                           # browse command history (all projects)
```

### Visual reference testing (brokkr litehtml)

Compares pipeline output against Chrome reference renders. Fixtures are defined in `fixtures/fixtures.toml`.

```bash
brokkr litehtml test --all                               # run all fixtures
brokkr litehtml test --suite smoke                       # run fixtures tagged "smoke"
brokkr litehtml test text_flow_test                      # run single fixture by ID
brokkr litehtml test --recapture text_flow_test          # force-regenerate Chrome reference
brokkr litehtml list                                     # list fixtures and tags
brokkr litehtml status                                   # show last run vs approved baselines
brokkr litehtml approve text_flow_test                   # record current divergence as accepted (clean tree required)
```

`brokkr bench` requires a clean git tree to store results, but ignores dirty markdown files and `results.db` itself.

**Always commit `.brokkr/`**. The `results.db` inside it is the benchmark history and must be tracked in git.

## Dependencies

- **scraper** 0.25 (html5ever + ego-tree DOM)
- **taffy** local fork (layout: block, flex, grid, table)
- **cosmic-text** 0.18 (text measurement + glyph rasterization)
- **tiny-skia** 0.11 (CPU rasterization, scaffold only)
- **ego-tree** 0.10 (DOM tree traversal)
- **lightningcss** 1.0.0-alpha.71 (CSS parsing — inline styles + stylesheet rules)
- **hotpath** 0.14 (profiling, behind `profile` feature flag)
- **clap** 4 (CLI argument parsing, binary only)

## Bash rules

- Never use sed, find, awk, or complex bash commands
- Never chain commands with &&
- Never chain commands with ;
- Never pipe commands with |
- Never read or write from /tmp. All data lives in the project. `*.png` and `*.json` are gitignored, so render output can safely go in the project directory.
- Never run raw cargo, curl, pkill. Use `brokkr`.

## Commit rules

- Don't commit pure markdown changes on their own. Bundle them with the code change they relate to, or skip them unless the update is substantive.

## Parent project

Part of **Ratatoskr** email client (`/home/folk/Programs/ratatoskr/`). Migrating from React/Tauri to pure Rust with iced. This renderer is the critical path item.
