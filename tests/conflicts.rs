use changelog_merge::{ConflictKind, MergeOptions, merge};
fn doc(s: &str) -> String {
    format!("## Unreleased\n### Added\n{s}")
}
#[test]
fn entry_conflicts() {
    for (b, o, t, kind) in [
        (
            "- Base.\n",
            "- Ours.\n",
            "- Theirs.\n",
            ConflictKind::ConcurrentModification,
        ),
        ("- Base.\n", "", "- Theirs.\n", ConflictKind::DeleteModify),
        (
            "- A.\n- B.\n",
            "- Rewrite.\n",
            "- A.\n- B.\n- Added.\n",
            ConflictKind::AmbiguousEntryIdentity,
        ),
        (
            "- Base.\n",
            "- A.\n- B.\n- Base.\n",
            "- B.\n- A.\n- Base.\n",
            ConflictKind::Ordering,
        ),
    ] {
        let r = merge(&doc(b), &doc(o), &doc(t), &MergeOptions::default()).unwrap();
        assert!(
            r.conflicts.iter().any(|c| c.kind == kind),
            "{:?}",
            r.conflicts
        );
        assert!(r.content.contains("<<<<<<< ours"));
        assert!(r.content.contains("||||||| base"));
        assert!(r.content.contains(">>>>>>> theirs"));
    }
}
#[test]
fn structural_conflicts() {
    let b = doc("- Base.\n");
    for (o, t, kind) in [
        (
            b.replace("Added", "Changed"),
            b.replace("Added", "Fixed"),
            ConflictKind::SectionRename,
        ),
        (
            "## Unreleased\n### Added\n\n### Fixed\n- Base.\n".to_owned(),
            doc("- Modified.\n"),
            ConflictKind::ConflictingMove,
        ),
    ] {
        let r = merge(&b, &o, &t, &MergeOptions::default()).unwrap();
        assert!(
            r.conflicts.iter().any(|c| c.kind == kind),
            "{:?}",
            r.conflicts
        );
    }
}
#[test]
fn marker_options_and_opaque() {
    let b = doc("- Base.\n");
    let mut options = MergeOptions::default();
    options.conflict_style.marker_size = 11;
    options.conflict_style.ours_label = "local".into();
    let r = merge(
        &b,
        &b.replace("Base", "Ours"),
        &b.replace("Base", "Theirs"),
        &options,
    )
    .unwrap();
    assert!(r.content.contains("<<<<<<<<<<< local"));
    let r = merge("# Base\n", "# Ours\n", "# Theirs\n", &options).unwrap();
    assert_eq!(r.conflicts[0].kind, ConflictKind::OpaqueContent);
}
#[test]
fn rewritten_release_cut_and_strict_edit() {
    let b = doc("- Base.\n");
    let o = "## Unreleased\n### Added\n\n## [1.0.0]\n### Added\n- Rewritten released entry.\n";
    let t = doc("- New.\n- Base.\n");
    let r = merge(&b, o, &t, &MergeOptions::default()).unwrap();
    assert_eq!(r.conflicts[0].kind, ConflictKind::ReleaseCut);
    let options = MergeOptions {
        strict: true,
        ..Default::default()
    };
    let r = merge(&b, &doc("- Edited.\n"), &t, &options).unwrap();
    assert_eq!(r.conflicts[0].kind, ConflictKind::AmbiguousEntryIdentity);
}
