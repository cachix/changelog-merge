#![no_main]
use changelog_merge::{MergeOptions, merge, parse};
use libfuzzer_sys::fuzz_target;
fn document(ops: &[u8], label: &str) -> String {
    let (mut added, mut fixed) = (String::new(), String::new());
    for (i, op) in ops.iter().enumerate() {
        let text = format!(
            "- Entry {i} unique_identifier_{i} existing API behavior{}.\n",
            if op % 4 == 1 { label } else { "" }
        );
        match op % 4 {
            0 | 1 => added.push_str(&text),
            2 => (),
            _ => fixed.push_str(&text),
        }
    }
    format!("## Unreleased\n\n### Added\n\n{added}\n### Fixed\n\n{fixed}")
}
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let count = (data[0] as usize % 16) + 1;
    let mut ours = vec![0u8; count];
    let mut theirs = vec![0u8; count];
    for i in 0..count {
        ours[i] = data.get(i + 1).copied().unwrap_or(0) % 4;
        theirs[i] = data.get(i + count + 1).copied().unwrap_or(0) % 4;
    }
    let base = document(&vec![0u8; count], "");
    let o = document(&ours, " ours");
    let t = document(&theirs, " theirs");
    let result = merge(&base, &o, &t, &MergeOptions::default()).unwrap();
    if result.conflicts.is_empty() {
        let doc = parse(&result.content).unwrap();
        for i in 0..count {
            assert!(!matches!(
                (ours[i], theirs[i]),
                (1, 1) | (1, 2) | (2, 1) | (3, 2) | (2, 3) | (3, 1) | (1, 3)
            ));
            let deleted = ours[i] == 2 || theirs[i] == 2;
            let found = doc
                .releases
                .iter()
                .flat_map(|r| &r.categories)
                .flat_map(|c| &c.entries)
                .filter(|e| e.normalized.contains(&format!("unique_identifier_{i} ")))
                .count();
            assert_eq!(found, usize::from(!deleted));
        }
        let swapped = merge(&base, &t, &o, &MergeOptions::default()).unwrap();
        assert!(swapped.conflicts.is_empty());
        assert_eq!(result.content, swapped.content);
    }
});
