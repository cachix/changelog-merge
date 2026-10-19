use changelog_merge::{MergeOptions, merge};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
fn document(entries: &str) -> String {
    format!("# Changelog\n\n## Unreleased\n\n### Added\n\n{entries}")
}
fn main() {
    let iterations = std::env::var("CHANGELOG_BENCH_ITERATIONS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 1000);
    let count = std::env::var("CHANGELOG_BENCH_ENTRIES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10_000)
        .clamp(10, 90_000);
    let entries: String = (0..count).map(|i| format!("- Entry {i}: `Api::method` with preserved multiline prose.\n\n  Additional details about operation {i}.\n\n")).collect();
    let base = document(&entries);
    let mut cases = vec![
        (
            "localized_additions",
            base.clone(),
            base.replace("- Entry 0:", "- Ours.\n\n- Entry 0:"),
            base.replace("- Entry 0:", "- Theirs.\n\n- Entry 0:"),
        ),
        (
            "localized_edits",
            base.clone(),
            base.replace("operation 0.", "operation 0 with retries."),
            base.replace(
                &format!("operation {}.", count - 1),
                "the last operation with timeouts.",
            ),
        ),
        (
            "release_cut",
            base.clone(),
            format!("## Unreleased\n\n## [1.0.0]\n\n### Added\n\n{entries}"),
            base.replace("- Entry 0:", "- Post-cut.\n\n- Entry 0:"),
        ),
    ];
    let sections: String = (0..count.min(1000))
        .map(|i| format!("### Category {i}\n\n- Unique entry {i}.\n\n"))
        .collect();
    let b = format!("## Unreleased\n\n{sections}");
    cases.push((
        "many_categories",
        b.clone(),
        b.replace("Unique entry 0.", "Unique entry 0 revised."),
        b.replace("Unique entry 1.", "Unique entry 1 revised."),
    ));
    let b = document(&"- Repeated existing entry.\n".repeat(count));
    cases.push((
        "duplicate_entries",
        b.clone(),
        b.replacen("- Repeated", "- Ours.\n- Repeated", 1),
        b.replacen("- Repeated", "- Theirs.\n- Repeated", 1),
    ));
    let b = format!(
        "{}\n{}",
        document("- Stable.\n"),
        (0..count)
            .map(|i| format!("[ref-{i}]: https://example.invalid/{i}\n"))
            .collect::<String>()
    );
    cases.push((
        "reference_definitions",
        b.clone(),
        format!("{b}[ours]: /ours\n"),
        format!("{b}[theirs]: /theirs\n"),
    ));
    let b = document(
        &(0..512)
            .map(|i| format!("- Old unrelated item {i}.\n"))
            .collect::<String>(),
    );
    cases.push((
        "rewrite_budget",
        b.clone(),
        document(
            &(0..512)
                .map(|i| format!("- Replacement unrelated item {i}.\n"))
                .collect::<String>(),
        ),
        format!("{b}- Independent.\n"),
    ));
    println!(
        "case,scale,base_entries,input_bytes,iterations,min_ms,p50_ms,p95_ms,max_ms,conflicts"
    );
    for (name, b, o, t) in cases {
        let base_entries: usize = changelog_merge::parse(&b)
            .unwrap()
            .releases
            .iter()
            .flat_map(|r| &r.categories)
            .map(|c| c.entries.len())
            .sum();
        let options = MergeOptions::default();
        let warmup = merge(&b, &o, &t, &options).expect("valid benchmark input");
        let mut samples = Vec::new();
        for _ in 0..iterations {
            let start = Instant::now();
            let result = merge(
                black_box(&b),
                black_box(&o),
                black_box(&t),
                black_box(&options),
            )
            .expect("valid benchmark input");
            black_box(result);
            samples.push(start.elapsed());
        }
        samples.sort();
        let percentile = |q: usize| {
            (iterations * q)
                .div_ceil(100)
                .saturating_sub(1)
                .min(iterations - 1)
        };
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        println!(
            "{name},{count},{base_entries},{},{iterations},{:.3},{:.3},{:.3},{:.3},{}",
            b.len() + o.len() + t.len(),
            ms(samples[0]),
            ms(samples[percentile(50)]),
            ms(samples[percentile(95)]),
            ms(samples[iterations - 1]),
            warmup.conflicts.len()
        );
    }
}
