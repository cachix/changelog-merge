#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data)
        && let Ok(doc) = changelog_merge::parse(source)
    {
        assert_eq!(doc.source, source);
        for r in &doc.releases {
            for c in &r.categories {
                for e in &c.entries {
                    assert!(source.get(e.span.start..e.span.end).is_some());
                    assert!(e.span.start >= c.span.start && e.span.end <= c.span.end);
                }
            }
        }
        let result = changelog_merge::merge(source, source, source, &Default::default()).unwrap();
        assert_eq!(result.content, source);
    }
});
