# rt-renderer

Pure Rust HTML email rendering pipeline. Parses HTML email bodies and renders them to pixel buffers, targeting 100ms or less per email.

Part of [Ratatoskr](https://github.com/folknor/ratatoskr), a native email client built with [iced](https://github.com/iced-rs/iced).

## Pipeline

```
HTML string
  -> scraper/html5ever (DOM parse)
  -> lightningcss (CSS: inline styles + <style> blocks)
  -> style resolution (cascade, inheritance, HTML attributes)
  -> taffy (layout)
  -> tiny-skia (pixel output) -> iced widgets (planned)
```

## Performance

All test emails render in **3-23ms** (pipeline only, excluding rasterization), well under the 100ms target. This replaced a C++ litehtml FFI approach that took 1,260ms per render due to 27K FFI round-trips for font metrics.

## Dependencies

- **scraper** -- HTML parsing (html5ever + ego-tree)
- **taffy** -- CSS layout (block, flex, grid, table)
- **cosmic-text** -- text measurement and glyph rasterization
- **tiny-skia** -- CPU pixel rendering (scaffold; final target is iced widgets)
- **lightningcss** -- CSS parsing

## Status

Work in progress. The rendering pipeline handles block layout, inline text flow, tables, flex containers, borders, backgrounds, and basic images. See [TODO.md](TODO.md) for known issues.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
