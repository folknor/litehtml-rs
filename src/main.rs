use clap::Parser;

#[derive(Parser)]
#[command(name = "litehtml-rs", about = "HTML email rendering pipeline")]
struct Cli {
    /// HTML file to render
    file: String,

    /// Dump element layout as JSON instead of rendering PNG
    #[arg(long)]
    dump: bool,

    /// Use Ahem test font for deterministic text measurement
    #[arg(long)]
    fixture: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.fixture {
        litehtml_rs::text::FIXTURE_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
        litehtml_rs::text::load_ahem_font();
    }

    let html_str = std::fs::read_to_string(&cli.file).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {e}", cli.file);
        std::process::exit(1);
    });

    if cli.dump {
        let result = litehtml_rs::dump(&html_str);
        print_stats(&result.selector_count, &result.element_count, &result.text_count, &result.table_count, &result.inline_style_count, result.layout_width, result.layout_height);

        let out_path = cli.file.replace(".html", "_pipeline.json");
        std::fs::write(&out_path, &result.json).unwrap();
        eprintln!("Dumped {} elements to {}", result.entry_count, out_path);
        return;
    }

    let result = litehtml_rs::render(&html_str);
    print_stats(&result.selector_count, &result.element_count, &result.text_count, &result.table_count, &result.inline_style_count, result.layout_width, result.layout_height);

    if result.layout_width == 0.0 || result.layout_height == 0.0 {
        eprintln!("Layout produced zero-size output");
        return;
    }

    let t4 = std::time::Instant::now();
    let out_path = cli.file.replace(".html", "_pipeline.png");
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
    eprintln!("parse_ms={}", result.parse_time.as_micros() as f64 / 1000.0);
    eprintln!("tree_css_ms={}", result.tree_build_time.as_micros() as f64 / 1000.0);
    eprintln!("layout_ms={}", result.layout_time.as_micros() as f64 / 1000.0);
    eprintln!("render_ms={}", result.render_time.as_micros() as f64 / 1000.0);
    eprintln!("png_ms={}", png_time.as_micros() as f64 / 1000.0);
    eprintln!("font_init_ms={}", result.font_init_time.as_micros() as f64 / 1000.0);
}

fn print_stats(selectors: &usize, elements: &usize, text_nodes: &usize, tables: &usize, inline_styles: &usize, w: f32, h: f32) {
    if *selectors > 0 {
        println!("Style rules extracted: {} selectors", selectors);
    }
    println!("Parsed: {} elements, {} text nodes, {} tables", elements, text_nodes, tables);
    println!("Inline styles: {}", inline_styles);
    println!("Layout: {}x{} (root)", w, h);
}
