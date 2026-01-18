//! Performance benchmarks for message rendering
//!
//! Tests render time for different message counts and content sizes.
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spoq::markdown::{render_markdown, MarkdownCache};

/// Generate test markdown content with varying complexity
fn generate_markdown_content(paragraphs: usize) -> String {
    let paragraph = r#"
This is a **test paragraph** with some `inline code` and *italics*.
It includes [links](https://example.com) and various markdown elements.

```rust
fn example_code() {
    let x = 42;
    println!("Hello, world!");
}
```

## Heading

- List item 1
- List item 2
- List item 3

"#;

    (0..paragraphs)
        .map(|i| format!("### Section {}\n\n{}", i + 1, paragraph))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Benchmark markdown rendering without cache
fn bench_markdown_render_uncached(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_render_uncached");

    for size in [1, 5, 10, 25, 50].iter() {
        let content = generate_markdown_content(*size);
        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_paragraphs", size)),
            &content,
            |b, content| {
                b.iter(|| {
                    let lines = render_markdown(black_box(content));
                    black_box(lines)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark markdown rendering with cache (warm cache)
fn bench_markdown_render_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_render_cached");

    for size in [1, 5, 10, 25, 50].iter() {
        let content = generate_markdown_content(*size);
        let mut cache = MarkdownCache::new();

        // Warm the cache
        let _ = cache.render(&content);

        group.throughput(Throughput::Bytes(content.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_paragraphs", size)),
            &content,
            |b, content| {
                b.iter(|| {
                    let lines = cache.render(black_box(content));
                    black_box(lines)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark markdown cache miss (cold cache, simulating content changes)
fn bench_markdown_render_cache_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_render_cache_miss");

    // Generate multiple unique content strings to force cache misses
    let contents: Vec<String> = (0..100)
        .map(|i| format!("Unique content {}: {}", i, generate_markdown_content(5)))
        .collect();

    group.throughput(Throughput::Elements(contents.len() as u64));

    group.bench_function("100_unique_contents", |b| {
        b.iter(|| {
            let mut cache = MarkdownCache::new();
            for content in &contents {
                let lines = cache.render(black_box(content));
                black_box(lines);
            }
        });
    });

    group.finish();
}

/// Benchmark selection text extraction
fn bench_selection_extract(c: &mut Criterion) {
    use spoq::selection::{extract_selected_text, ContentPosition, SelectionMode, SelectionRange};

    let mut group = c.benchmark_group("selection_extract");

    // Create test content
    let lines: Vec<&str> = vec![
        "First line of content here",
        "Second line with more text",
        "Third line continues",
        "Fourth line of test content",
        "Fifth line wrapping up",
    ];

    // Small selection (single line)
    let small_selection = SelectionRange::new(
        ContentPosition::new(0, 5),
        ContentPosition::new(0, 15),
        SelectionMode::Character,
    );

    // Large selection (multiple lines)
    let large_selection = SelectionRange::new(
        ContentPosition::new(0, 0),
        ContentPosition::new(4, 20),
        SelectionMode::Character,
    );

    group.bench_function("small_selection", |b| {
        b.iter(|| {
            let text = extract_selected_text(black_box(&lines), black_box(&small_selection));
            black_box(text)
        });
    });

    group.bench_function("large_selection", |b| {
        b.iter(|| {
            let text = extract_selected_text(black_box(&lines), black_box(&large_selection));
            black_box(text)
        });
    });

    group.finish();
}

/// Benchmark selection highlighting
fn bench_selection_highlight(c: &mut Criterion) {
    use ratatui::text::Line;
    use spoq::selection::{default_highlight_style, highlight_line_selection};

    let mut group = c.benchmark_group("selection_highlight");

    // Create test lines with varying complexity
    let simple_line = Line::from("Simple text content without any formatting");
    let complex_line = Line::from(vec![
        ratatui::text::Span::raw("Normal "),
        ratatui::text::Span::styled("bold", ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD)),
        ratatui::text::Span::raw(" text "),
        ratatui::text::Span::styled("code", ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)),
        ratatui::text::Span::raw(" more"),
    ]);

    let style = default_highlight_style();

    group.bench_function("simple_line_full", |b| {
        b.iter(|| {
            let result = highlight_line_selection(
                black_box(&simple_line),
                black_box(0),
                black_box(42),
                black_box(style),
            );
            black_box(result)
        });
    });

    group.bench_function("simple_line_partial", |b| {
        b.iter(|| {
            let result = highlight_line_selection(
                black_box(&simple_line),
                black_box(10),
                black_box(30),
                black_box(style),
            );
            black_box(result)
        });
    });

    group.bench_function("complex_line_partial", |b| {
        b.iter(|| {
            let result = highlight_line_selection(
                black_box(&complex_line),
                black_box(5),
                black_box(20),
                black_box(style),
            );
            black_box(result)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_markdown_render_uncached,
    bench_markdown_render_cached,
    bench_markdown_render_cache_miss,
    bench_selection_extract,
    bench_selection_highlight,
);

criterion_main!(benches);
