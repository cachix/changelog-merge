use changelog_merge::{MergeOptions, merge, parse};
fn doc(entries: &str) -> String {
    format!("# Changelog\n\n## [Unreleased]\n\n### Added\n\n{entries}")
}
fn clean(b: &str, o: &str, t: &str) -> String {
    let r = merge(b, o, t, &MergeOptions::default()).unwrap();
    assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
    r.content
}
#[test]
fn independent_additions() {
    let b = doc("- Initial feature.\n");
    let o = doc("- Added SSH support.\n- Initial feature.\n");
    let t = doc("- Added S3 support.\n- Initial feature.\n");
    assert_eq!(
        clean(&b, &o, &t),
        doc("- Added S3 support.\n- Added SSH support.\n- Initial feature.\n")
    );
    assert_eq!(clean(&b, &o, &t), clean(&b, &t, &o));
}
#[test]
fn edits_deletes_and_identical_modifications() {
    let b = doc("- Alpha.\n- Beta.\n- Gamma.\n");
    assert_eq!(
        clean(
            &b,
            &doc("- Alpha edited.\n- Beta.\n- Gamma.\n"),
            &doc("- Alpha.\n- Beta.\n- Gamma edited.\n")
        ),
        doc("- Alpha edited.\n- Beta.\n- Gamma edited.\n")
    );
    assert_eq!(
        clean(
            &b,
            &doc("- Beta.\n- Gamma.\n"),
            &doc("- Alpha.\n- Beta.\n- Gamma.\n- Extra.\n")
        ),
        doc("- Beta.\n- Gamma.\n- Extra.\n")
    );
    assert_eq!(
        clean(&b, &doc("- Beta.\n- Gamma.\n"), &doc("- Beta.\n- Gamma.\n")),
        doc("- Beta.\n- Gamma.\n")
    );
}
#[test]
fn preserve_branch_order_deduplicate_and_idempotence() {
    let b = doc("- Base.\n");
    let o = doc("- Zed.\n- Alpha.\n- Base.\n");
    let t = doc("- Middle.\n- Base.\n");
    let r = clean(&b, &o, &t);
    assert!(r.find("Zed").unwrap() < r.find("Alpha").unwrap());
    assert_eq!(clean(&b, &r, &o), r);
    let dup = clean(
        &b,
        &doc("- Same.\n- Same.\n- Base.\n"),
        &doc("- Same.\n- Base.\n"),
    );
    assert_eq!(dup.matches("- Same.").count(), 2);
}
#[test]
fn sections_and_opaque_regions() {
    let b = doc("- Base.\n");
    let o = format!("{b}\n### Custom\n\n- One.\n");
    let t = format!("{b}\n### Security\n\n- Two.\n");
    let r = clean(&b, &o, &t);
    assert!(r.contains("- One."));
    assert!(r.contains("- Two."));
    let t = format!("{b}\n### Custom\n\n- Two.\n");
    let r = clean(&b, &o, &t);
    assert_eq!(r.matches("### Custom").count(), 1);
    assert!(r.contains("- One."));
    assert!(r.contains("- Two."));
}
#[test]
fn casita_preservation_and_multiline() {
    let b = include_str!("fixtures/casita.md");
    let o = b.replace(
        "### Added\n\n",
        "### Added\n\n- Added `Foo::bar` API.\n\n  Wrapped explanation.\n\n",
    );
    let t = b
        .replace(
            "### Added\n\n",
            "### Added\n\n- Added [another API](https://example.com).\n\n",
        )
        .replace("### Changed\n\n", "### Changed\n\n- Updated `Bar`.\n\n");
    let r = clean(b, &o, &t);
    assert!(r.contains("- Added `Foo::bar` API.\n\n  Wrapped explanation."));
    assert_eq!(r.split("## [1.0.0]").nth(1), b.split("## [1.0.0]").nth(1));
    assert_eq!(
        parse(&r).unwrap().releases[0].categories[0].entries.len(),
        4
    );
}
#[test]
fn noops_and_line_endings() {
    for b in [doc("- Base."), doc("- Base.\n").replace('\n', "\r\n")] {
        let o = b.replace("Base", "Ours");
        assert_eq!(clean(&b, &b, &b), b);
        assert_eq!(clean(&b, &o, &b), o);
        assert_eq!(clean(&b, &b, &o), o);
    }
    let b = doc("- Base.\n").replace('\n', "\r\n");
    let o = b.replace("- Base.", "- A.\r\n- Base.");
    let t = b.replace("- Base.", "- B.\r\n- Base.");
    let r = clean(&b, &o, &t);
    assert!(!r.replace("\r\n", "").contains('\n'));
}
#[test]
fn exact_moves_and_different_new_releases() {
    let b = "## Unreleased\n### Added\n- Base.\n- Stay.\n### Fixed\n- Anchor.\n";
    let o = b
        .replace("- Base.\n", "")
        .replace("- Anchor.", "- Base.\n- Anchor.");
    let t = b.replace("- Stay.", "- Stay.\n- New.");
    let r = clean(b, &o, &t);
    assert_eq!(r.matches("- Base.").count(), 1);
    assert!(r.split("### Fixed").nth(1).unwrap().contains("- Base."));
    let b = "## [1.0.0]\n### Added\n- History.\n";
    let o = format!("## [2.0.0]\n### Added\n- Two.\n{b}");
    let t = format!("## [1.5.0]\n### Added\n- One.\n{b}");
    let r = clean(b, &o, &t);
    assert!(r.contains("- Two."));
    assert!(r.contains("- One."));
    assert!(r.ends_with(b));
}
#[test]
fn existing_duplicates_and_identical_edit() {
    let b = doc("- Same.\n- Same.\n- Anchor.\n");
    let o = doc("- First.\n- Same.\n- Same.\n- Anchor.\n");
    let t = doc("- Same.\n- Same.\n- Anchor.\n- Last.\n");
    let r = clean(&b, &o, &t);
    assert_eq!(r.matches("- Same.").count(), 2);
    let b = doc("- Base.\n- Anchor.\n");
    let o = doc("- Edited.\n- Anchor.\n- Ours.\n");
    let t = doc("- Edited.\n- Anchor.\n- Theirs.\n");
    let r = clean(&b, &o, &t);
    assert_eq!(r.matches("- Edited.").count(), 1);
}
#[test]
fn ten_thousand_entries_with_local_additions() {
    let entries: String = (0..10_000)
        .map(|i| format!("- Entry {i}: `Api::method` with preserved prose.\n"))
        .collect();
    let b = doc(&entries);
    let o = b.replace("- Entry 0:", "- Ours.\n- Entry 0:");
    let t = b.replace("- Entry 0:", "- Theirs.\n- Entry 0:");
    let r = clean(&b, &o, &t);
    assert_eq!(
        parse(&r).unwrap().releases[0].categories[0].entries.len(),
        10_002
    );
    assert!(r.ends_with(&entries));
}
