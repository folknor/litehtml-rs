use clap::Parser;

#[derive(Parser)]
#[command(name = "litehtml-rs", about = "HTML email rendering pipeline")]
struct Cli {
    /// HTML file to render
    file: String,

    /// Use Ahem test font for deterministic text measurement
    #[arg(long)]
    fixture: bool,

    /// Viewport width in pixels for layout and media query evaluation (default: 800)
    #[arg(long, default_value_t = 800.0)]
    width: f32,

    /// Output directory for PNG and JSON files (default: same directory as input)
    #[arg(long)]
    output_dir: Option<String>,
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

    let result = litehtml_rs::render(&html_str, cli.width);

    if result.selector_count > 0 {
        println!("Style rules extracted: {} selectors", result.selector_count);
    }
    println!(
        "Parsed: {} elements, {} text nodes, {} tables",
        result.element_count, result.text_count, result.table_count
    );
    println!("Inline styles: {}", result.inline_style_count);
    println!("Layout: {}x{} (root)", result.layout_width, result.layout_height);

    if result.layout_width == 0.0 || result.layout_height == 0.0 {
        eprintln!("Layout produced zero-size output");
        return;
    }

    // Determine output paths
    let stem = std::path::Path::new(&cli.file)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();
    let (png_path, json_path) = if let Some(ref dir) = cli.output_dir {
        std::fs::create_dir_all(dir).unwrap();
        (format!("{dir}/pipeline.png"), format!("{dir}/pipeline.json"))
    } else {
        let parent = std::path::Path::new(&cli.file).parent().unwrap().to_str().unwrap();
        (format!("{parent}/{stem}_pipeline.png"), format!("{parent}/{stem}_pipeline.json"))
    };

    // Write PNG
    let t4 = std::time::Instant::now();
    result.pixmap.save_png(&png_path).unwrap();
    let png_time = t4.elapsed();
    println!("Saved to {png_path}");

    // Write JSON dump
    std::fs::write(&json_path, &result.json).unwrap();
    println!("Dumped {} elements to {json_path}", result.json_entry_count);

    // Timing
    let pipeline_total =
        result.parse_time + result.tree_build_time + result.layout_time + result.render_time;

    println!("--- Timing ---");
    println!("  Font init:    {:?} (one-time)", result.font_init_time);
    println!("  HTML parse:   {:?}", result.parse_time);
    println!("  Tree+CSS:     {:?}", result.tree_build_time);
    println!("  Layout:       {:?}", result.layout_time);
    println!("  Render:       {:?}", result.render_time);
    println!("  PNG encode:   {:?}", png_time);
    println!("  Total (no PNG): {:?}", pipeline_total);

    // Machine-readable key=value output for brokkr
    eprintln!("elapsed_ms={}", pipeline_total.as_millis());
    eprintln!("parse_ms={}", result.parse_time.as_micros() as f64 / 1000.0);
    eprintln!("tree_css_ms={}", result.tree_build_time.as_micros() as f64 / 1000.0);
    eprintln!("layout_ms={}", result.layout_time.as_micros() as f64 / 1000.0);
    eprintln!("render_ms={}", result.render_time.as_micros() as f64 / 1000.0);
    eprintln!("png_ms={}", png_time.as_micros() as f64 / 1000.0);
    eprintln!("font_init_ms={}", result.font_init_time.as_micros() as f64 / 1000.0);
}
