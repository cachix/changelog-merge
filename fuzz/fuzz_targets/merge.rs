#![no_main]
use changelog_merge::{Document, MergeOptions, merge, parse};
use libfuzzer_sys::fuzz_target;
fn inventory(d: &Document) -> Vec<(String, String, String)> {
    let mut entries: Vec<_> = d
        .releases
        .iter()
        .flat_map(|r| {
            r.categories.iter().flat_map(move |c| {
                c.entries
                    .iter()
                    .map(move |e| (r.id.clone(), c.id.clone(), e.normalized.clone()))
            })
        })
        .collect();
    entries.sort();
    entries
}
fuzz_target!(|data: &[u8]| {
    let mut parts = data.splitn(3, |b| *b == 0);
    let b = String::from_utf8_lossy(parts.next().unwrap_or_default());
    let o = String::from_utf8_lossy(parts.next().unwrap_or_default());
    let t = String::from_utf8_lossy(parts.next().unwrap_or_default());
    let options = MergeOptions::default();
    if let Ok(result) = merge(&b, &o, &t, &options) {
        assert_eq!(result.content, merge(&b, &o, &t, &options).unwrap().content);
        if result.conflicts.is_empty() {
            let doc = parse(&result.content).unwrap();
            let swapped = merge(&b, &t, &o, &options).unwrap();
            assert!(swapped.conflicts.is_empty());
            assert_eq!(
                inventory(&doc),
                inventory(&parse(&swapped.content).unwrap())
            );
        }
    }
});
