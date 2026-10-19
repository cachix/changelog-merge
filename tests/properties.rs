use changelog_merge::{MergeOptions, merge, parse};
use proptest::prelude::*;
proptest! {
    #[test]
    fn additions_survive_symmetrically(a in "[a-z]{1,30}", b in "[a-z]{1,30}") {
        let base = "## Unreleased\n### Added\n- Base.\n";
        let ours = base.replace("- Base.",&format!("- O: {a}.\n- Base."));
        let theirs = base.replace("- Base.",&format!("- T: {b}.\n- Base."));
        let r = merge(base,&ours,&theirs,&MergeOptions::default()).unwrap();
        prop_assert!(r.conflicts.is_empty());
        prop_assert!(r.content.contains(&format!("- O: {}.", a)), "ours survives");
        prop_assert!(r.content.contains(&format!("- T: {}.", b)), "theirs survives");
        prop_assert_eq!(&r.content,&merge(base,&theirs,&ours,&MergeOptions::default()).unwrap().content);
        prop_assert_eq!(&r.content,&merge(base,&ours,&theirs,&MergeOptions::default()).unwrap().content);
        prop_assert_eq!(&r.content,&merge(base,&r.content,&ours,&MergeOptions::default()).unwrap().content);
    }
    #[test]
    fn arbitrary_input_never_panics(bytes in prop::collection::vec(any::<u8>(),0..4096)) {
        if let Ok(source) = std::str::from_utf8(&bytes) {
            let _ = parse(source);
            let _ = merge(source,source,source,&MergeOptions::default());
        }
    }
    #[test]
    fn unicode_markdown_never_panics(source in ".{0,2000}",other in ".{0,1000}") {
        let _ = parse(&source);
        let _ = merge(&source,&other,"## Unreleased\n",&MergeOptions::default());
    }
}
fn operation_document(ops: &[u8], label: &str) -> String {
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
proptest! {
    #[test]
    fn structured_changes_never_hide_incompatible_operations(pairs in prop::collection::vec((0u8..4,0u8..4),1..16)) {
        let ours: Vec<_> = pairs.iter().map(|p| p.0).collect(); let theirs: Vec<_> = pairs.iter().map(|p| p.1).collect();
        let base = operation_document(&vec![0;pairs.len()],""); let o = operation_document(&ours," ours"); let t = operation_document(&theirs," theirs");
        let result = merge(&base,&o,&t,&MergeOptions::default()).unwrap();
        if result.conflicts.is_empty() {
            for (i,&(o,t)) in pairs.iter().enumerate() {
                prop_assert!(!matches!((o,t),(1,1)|(1,2)|(2,1)|(3,2)|(2,3)|(3,1)|(1,3)),"incompatible entry operations returned clean");
                let id = format!("unique_identifier_{i} ");
                let count = result.content.matches(&id).count();
                prop_assert_eq!(count,usize::from(o != 2 && t != 2));
            }
            let swapped = merge(&base,&t,&o,&MergeOptions::default()).unwrap();
            prop_assert!(swapped.conflicts.is_empty()); prop_assert_eq!(result.content,swapped.content);
        }
    }
    #[test]
    fn release_cut_conserves_membership(count in 1usize..20, post_cut in 1usize..8) {
        let entries: String = (0..count).map(|i| format!("- Released API {i}.\n")).collect();
        let new: String = (0..post_cut).map(|i| format!("- Post-cut API {i}.\n")).collect();
        let base = format!("## Unreleased\n\n### Added\n\n{entries}");
        let ours = format!("## Unreleased\n\n## [1.0.0]\n\n### Added\n\n{entries}");
        let theirs = format!("## Unreleased\n\n### Added\n\n{new}{entries}");
        let result = merge(&base,&ours,&theirs,&MergeOptions::default()).unwrap(); prop_assert!(result.conflicts.is_empty());
        let d = parse(&result.content).unwrap();
        prop_assert_eq!(d.releases[0].id.as_str(),"unreleased"); prop_assert_eq!(d.releases[0].categories[0].entries.len(),post_cut);
        prop_assert_eq!(d.releases[1].id.as_str(),"1.0.0"); prop_assert_eq!(d.releases[1].categories[0].entries.len(),count);
        prop_assert_eq!(&result.content,&merge(&base,&theirs,&ours,&MergeOptions::default()).unwrap().content);
    }
}

#[test]
fn overlapping_moves_preserve_both_branch_orders() {
    let base = operation_document(&[0, 0, 0, 0], "");
    let ours = operation_document(&[3, 3, 3, 0], "");
    let theirs = operation_document(&[0, 0, 3, 3], "");
    let result = merge(&base, &ours, &theirs, &MergeOptions::default()).unwrap();
    assert!(result.conflicts.is_empty(), "{:?}", result.conflicts);
    let swapped = merge(&base, &theirs, &ours, &MergeOptions::default()).unwrap();
    assert!(swapped.conflicts.is_empty());
    assert_eq!(result.content, swapped.content);
    let doc = parse(&result.content).unwrap();
    let fixed = &doc.releases[0].categories[1];
    assert_eq!(fixed.entries.len(), 4);
    for (i, entry) in fixed.entries.iter().enumerate() {
        assert!(
            entry
                .span
                .text(&doc.source)
                .contains(&format!("unique_identifier_{i} "))
        );
    }
}

#[test]
fn every_four_entry_move_overlap_preserves_order() {
    for mask in 0..256u16 {
        let ours: Vec<_> = (0..4)
            .map(|i| if mask & (1 << i) != 0 { 3 } else { 0 })
            .collect();
        let theirs: Vec<_> = (0..4)
            .map(|i| if mask & (1 << (i + 4)) != 0 { 3 } else { 0 })
            .collect();
        let base = operation_document(&[0; 4], "");
        let o = operation_document(&ours, "");
        let t = operation_document(&theirs, "");
        let result = merge(&base, &o, &t, &MergeOptions::default()).unwrap();
        assert!(
            result.conflicts.is_empty(),
            "mask {mask}: {:?}",
            result.conflicts
        );
        let swapped = merge(&base, &t, &o, &MergeOptions::default()).unwrap();
        assert!(swapped.conflicts.is_empty());
        assert_eq!(result.content, swapped.content, "mask {mask}");
        let doc = parse(&result.content).unwrap();
        for category in &doc.releases[0].categories {
            let mut previous = None;
            for entry in &category.entries {
                let index = (0..4)
                    .find(|i| {
                        entry
                            .span
                            .text(&doc.source)
                            .contains(&format!("unique_identifier_{i} "))
                    })
                    .unwrap();
                assert!(previous.is_none_or(|p| p < index), "mask {mask}");
                previous = Some(index);
            }
        }
    }
}
