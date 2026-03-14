fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dump_mode = args.iter().any(|a| a == "--dump");
    let fixture_mode = args.iter().any(|a| a == "--fixture");
    if fixture_mode {
        litehtml_rs::text::FIXTURE_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
        litehtml_rs::text::load_ahem_font();
    }
    let path = args
        .iter()
        .find(|a| !a.starts_with('-') && *a != &args[0])
        .cloned()
        .unwrap_or_else(|| {
            eprintln!("Usage: litehtml-rs [--dump] [--fixture] <html-file>");
            std::process::exit(1);
        });
    let html_str = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });

    if dump_mode {
        let result = litehtml_rs::dump(&html_str);

        if result.selector_count > 0 {
            println!("Style rules extracted: {} selectors", result.selector_count);
        }
        println!(
            "Parsed: {} elements, {} text nodes, {} tables",
            result.element_count, result.text_count, result.table_count
        );
        println!("Inline styles: {}", result.inline_style_count);
        println!(
            "Layout: {}x{} (root)",
            result.layout_width, result.layout_height
        );

        let out_path = path.replace(".html", "_pipeline.json");
        std::fs::write(&out_path, &result.json).unwrap();
        eprintln!("Dumped {} elements to {}", result.entry_count, out_path);
        return;
    }

    let result = litehtml_rs::render(&html_str);

    if result.selector_count > 0 {
        println!("Style rules extracted: {} selectors", result.selector_count);
    }
    println!(
        "Parsed: {} elements, {} text nodes, {} tables",
        result.element_count, result.text_count, result.table_count
    );
    println!("Inline styles: {}", result.inline_style_count);
    println!(
        "Layout: {}x{} (root)",
        result.layout_width, result.layout_height
    );

    if result.layout_width == 0.0 || result.layout_height == 0.0 {
        eprintln!("Layout produced zero-size output");
        return;
    }

    let t4 = std::time::Instant::now();
    let out_path = path.replace(".html", "_pipeline.png");
    result.pixmap.save_png(&out_path).unwrap();
    let png_time = t4.elapsed();

    let pipeline_total =
        result.parse_time + result.tree_build_time + result.layout_time + result.render_time;

    println!("Saved to {out_path}");
    println!("--- Timing ---");
    println!("  Font init:    {:?} (one-time)", result.font_init_time);
    println!("  HTML parse:   {:?}", result.parse_time);
    println!("  Tree+CSS:     {:?}", result.tree_build_time);
    println!("  Layout:       {:?}", result.layout_time);
    println!("  Render:       {:?}", result.render_time);
    println!("  PNG encode:   {:?}", png_time);
    println!("  Total (no PNG): {:?}", pipeline_total);
    println!(
        "  Total (warm):   {:?} (excludes font init)",
        pipeline_total
    );

    // Machine-readable key=value output for brokkr
    eprintln!("elapsed_ms={}", pipeline_total.as_millis());
    eprintln!(
        "parse_ms={}",
        result.parse_time.as_micros() as f64 / 1000.0
    );
    eprintln!(
        "tree_css_ms={}",
        result.tree_build_time.as_micros() as f64 / 1000.0
    );
    eprintln!(
        "layout_ms={}",
        result.layout_time.as_micros() as f64 / 1000.0
    );
    eprintln!(
        "render_ms={}",
        result.render_time.as_micros() as f64 / 1000.0
    );
    eprintln!("png_ms={}", png_time.as_micros() as f64 / 1000.0);
    eprintln!(
        "font_init_ms={}",
        result.font_init_time.as_micros() as f64 / 1000.0
    );
}
