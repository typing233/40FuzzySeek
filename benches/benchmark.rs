use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};

fn generate_lines(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("src/components/module_{}/handler_{}.rs", i % 1000, i))
        .collect()
}

fn bench_fuzzy_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("fuzzy_match");
    group.measurement_time(Duration::from_secs(5));

    for size in [1_000, 10_000, 100_000, 1_000_000] {
        let lines = generate_lines(size);
        let query = "handler";

        group.bench_with_input(
            BenchmarkId::new("match_all", size),
            &lines,
            |b, lines| {
                b.iter(|| {
                    let atom = Atom::new(
                        query,
                        CaseMatching::Smart,
                        Normalization::Smart,
                        AtomKind::Fuzzy,
                        false,
                    );
                    let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
                    let mut buf = Vec::new();
                    let mut count = 0u32;

                    for line in lines.iter() {
                        let haystack = Utf32Str::new(line, &mut buf);
                        if atom.score(haystack, &mut matcher).is_some() {
                            count += 1;
                        }
                        buf.clear();
                    }
                    black_box(count)
                });
            },
        );
    }
    group.finish();
}

fn bench_unicode_match(c: &mut Criterion) {
    let lines: Vec<String> = (0..10_000)
        .map(|i| format!("文件_{}/处理器_{}/模块.rs", i % 100, i))
        .collect();

    c.bench_function("unicode_10k", |b| {
        b.iter(|| {
            let atom = Atom::new(
                "处理",
                CaseMatching::Smart,
                Normalization::Smart,
                AtomKind::Fuzzy,
                false,
            );
            let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
            let mut buf = Vec::new();
            let mut count = 0u32;

            for line in lines.iter() {
                let haystack = Utf32Str::new(line, &mut buf);
                if atom.score(haystack, &mut matcher).is_some() {
                    count += 1;
                }
                buf.clear();
            }
            black_box(count)
        });
    });
}

fn bench_strip_ansi(c: &mut Criterion) {
    let lines: Vec<String> = (0..10_000)
        .map(|i| format!("\x1b[32m{}\x1b[0m: \x1b[1msome content here {}\x1b[0m", i, i * 2))
        .collect();

    c.bench_function("strip_ansi_10k", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for line in &lines {
                let stripped = strip_ansi_escapes::strip(line);
                total += stripped.len();
            }
            black_box(total)
        });
    });
}

criterion_group!(benches, bench_fuzzy_match, bench_unicode_match, bench_strip_ansi);
criterion_main!(benches);
