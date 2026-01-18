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

criterion_group!(
    benches,
    bench_markdown_render_uncached,
    bench_markdown_render_cached,
    bench_markdown_render_cache_miss,
);

criterion_main!(benches);
