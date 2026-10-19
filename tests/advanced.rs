use changelog_merge::{ConflictKind, EventKind, MergeOptions, merge, parse};
fn clean(b: &str, o: &str, t: &str) -> String {
    let result = merge(b, o, t, &MergeOptions::default()).unwrap();
    assert!(
        result.conflicts.is_empty(),
        "{:?}\n{}",
        result.conflicts,
        result.content
    );
    result.content
}
fn doc(entries: &str) -> String {
    format!("## Unreleased\n\n### Added\n\n{entries}")
}
#[test]
fn release_cut_keeps_post_cut_work_unreleased() {
    let b = doc("- Released.\n");
    let o = "## Unreleased\n\n### Added\n\n## [1.0.0] - 2026-10-19\n\n### Added\n\n- Released.\n";
    let t = doc("- Post-cut.\n- Released.\n");
    let r = clean(&b, o, &t);
    assert_eq!(r.matches("- Released.").count(), 1);
    let (unreleased, release) = r.split_once("## [1.0.0]").unwrap();
    assert!(unreleased.contains("- Post-cut."));
    assert!(!unreleased.contains("- Released."));
    assert!(release.contains("- Released."));
    assert_eq!(r, clean(&b, &t, o));
}
#[test]
fn release_cut_removing_empty_sections_and_adding_new_work() {
    let b = doc("- Released.\n");
    let o = "## [1.0.0]\n\n### Added\n\n- Released.\n";
    let t = doc("- New.\n- Released.\n");
    let r = clean(&b, o, &t);
    assert!(r.contains("## Unreleased"));
    assert_eq!(r.matches("- Released.").count(), 1);
    assert!(r.contains("- New."));
}
#[test]
fn competing_release_cuts_and_cut_vs_edit_conflict() {
    let b = doc("- Released.\n");
    let o = "## Unreleased\n\n## [1.0.0]\n### Added\n- Released.\n";
    for t in [o.replace("1.0.0", "2.0.0"), doc("- Released with edits.\n")] {
        let r = merge(&b, o, &t, &MergeOptions::default()).unwrap();
        assert!(
            r.conflicts
                .iter()
                .any(|c| c.kind == ConflictKind::ReleaseCut)
        );
    }
}
#[test]
fn larger_rewrite_gaps_match_unique_candidates() {
    let b = doc("- Add SSH importer retries.\n- Fix S3 storage timeouts.\n- Anchor.\n");
    let o = doc(
        "- Add SSH importer automatic retries.\n- Fix S3 storage request timeouts.\n- Anchor.\n",
    );
    let t = format!("{b}- Another.\n");
    let r = clean(&b, &o, &t);
    assert!(r.contains("automatic retries"));
    assert!(r.contains("request timeouts"));
    assert!(r.contains("- Another."));
}
#[test]
fn one_sided_and_compatible_reordering() {
    let b = doc("- A.\n- B.\n- C.\n- D.\n");
    let o = doc("- B.\n- A.\n- C.\n- D.\n");
    let t = doc("- A.\n- B.\n- D.\n- C.\n");
    let r = clean(&b, &o, &t);
    assert!(r.find("- B.") < r.find("- A."));
    assert!(r.find("- D.") < r.find("- C."));
    let t = doc("- A.\n- B.\n- C edited.\n- D.\n");
    let r = clean(&b, &o, &t);
    assert!(r.contains("- C edited."));
    assert!(r.find("- B.") < r.find("- A."));
}
#[test]
fn rename_plus_edit_and_identical_renames() {
    let b = doc("- Base.\n- Anchor.\n");
    let o = b.replace("### Added", "### Features");
    let t = b.replace("- Base.", "- Edited.");
    let r = clean(&b, &o, &t);
    assert!(r.contains("### Features"));
    assert!(r.contains("- Edited."));
    let t = o.replace("- Anchor.", "- Anchor.\n- New.");
    let r = clean(&b, &o, &t);
    assert!(r.contains("### Features"));
    assert_eq!(r.matches("### Features").count(), 1);
}
#[test]
fn moved_entries_and_removed_source_categories() {
    let b = "## Unreleased\n### Added\n- Move me.\n### Fixed\n- Anchor.\n";
    let o = "## Unreleased\n### Fixed\n- Move me.\n- Anchor.\n";
    let t = b.replace("- Anchor.", "- Anchor updated.");
    let r = clean(b, o, &t);
    assert_eq!(r.matches("- Move me.").count(), 1);
    assert!(!r.contains("### Added"));
    assert!(r.contains("Anchor updated"));
}
#[test]
fn opaque_blocks_and_interleaved_references() {
    let b = "## Unreleased\n### Added\n- First.\n\n<!-- keep -->\n\nParagraph outside the list.\n\n[api]: https://example.com/api\n\n- Last.\n";
    let d = parse(b).unwrap();
    assert_eq!(d.releases[0].categories[0].entries.len(), 2);
    assert_eq!(d.releases[0].categories[0].blocks.len(), 3);
    let o = b.replace("- First.", "- First edited.");
    let t = b.replace("- Last.", "- Last edited.");
    let r = clean(b, &o, &t);
    assert!(r.contains(
        "<!-- keep -->\n\nParagraph outside the list.\n\n[api]: https://example.com/api"
    ));
}
#[test]
fn independent_opaque_lines_and_reference_definitions() {
    let b = "# Changelog\n\nFirst paragraph.\n\nSecond paragraph.\n\n## Unreleased\n### Added\n- Base.\n\n[a]: https://example.com/a\n[b]: https://example.com/b\n";
    let o = b
        .replace("First paragraph.", "First edited.")
        .replace("https://example.com/a", "https://example.com/a2");
    let t = b
        .replace("Second paragraph.", "Second edited.")
        .replace("https://example.com/b", "https://example.com/b2");
    let r = clean(b, &o, &t);
    assert!(r.contains("First edited."));
    assert!(r.contains("Second edited."));
    assert!(r.contains("/a2"));
    assert!(r.contains("/b2"));
    let o = format!("{b}[c]: https://example.com/c\n");
    let t = format!("{b}[d]: https://example.com/d\n");
    let r = clean(b, &o, &t);
    assert!(r.contains("[c]:"));
    assert!(r.contains("[d]:"));
}
#[test]
fn same_reference_conflicts_atomically() {
    let b = "## Unreleased\n\n[a]: https://example.com/a \"Title\"\n";
    let o = b.replace("/a ", "/changed ");
    let t = b.replace("Title", "Other title");
    let r = merge(b, &o, &t, &MergeOptions::default()).unwrap();
    assert!(!r.conflicts.is_empty());
    let b = "## Unreleased\n";
    let o = format!("{b}\n[a]: https://example.com/ours\n");
    let t = format!("{b}\n[a]: https://example.com/theirs\n");
    assert!(
        !merge(b, &o, &t, &MergeOptions::default())
            .unwrap()
            .conflicts
            .is_empty()
    );
}
#[test]
fn whitespace_edits_and_mixed_endings_survive() {
    let b = doc("- Base.\n- Anchor.\n");
    let o = b.replace("- Base.\n", "- Base.\n\n");
    let t = b.replace("- Anchor.", "- Anchor edited.");
    let r = clean(&b, &o, &t);
    assert!(r.contains("- Base.\n\n- Anchor edited."));
    let b = "# Changelog\r\n\r\n## Unreleased\n\n### Added\r\n\r\n- Base.\r\n- Anchor.\n";
    let o = b.replace("- Base.", "- Ours.\r\n- Base.");
    let t = b.replace("- Anchor.", "- Anchor edited.");
    let r = clean(b, &o, &t);
    assert!(r.starts_with("# Changelog\r\n\r\n## Unreleased\n\n### Added\r\n\r\n"));
    assert!(r.contains("- Base.\r\n"));
    assert!(r.contains("- Anchor edited.\n"));
}
#[test]
fn diagnostic_events_cover_changes_and_decisions() {
    let b = doc("- Update importer retries.\n- Delete.\n- Anchor.\n");
    let o = doc("- Update importer automatic retries.\n- Anchor.\n- Shared.\n");
    let t = doc("- Update importer retries.\n- Delete.\n- Anchor.\n- Shared.\n- Other.\n");
    let r = merge(&b, &o, &t, &MergeOptions::default()).unwrap();
    assert!(r.conflicts.is_empty());
    for kind in [
        EventKind::MatchedRelease,
        EventKind::MatchedCategory,
        EventKind::Modified,
        EventKind::Deleted,
        EventKind::Added,
        EventKind::Ordered,
        EventKind::Deduplicated,
    ] {
        assert!(
            r.diagnostics.iter().any(|e| e.kind == kind),
            "missing {kind:?}: {:?}",
            r.diagnostics
        );
    }
}
#[test]
fn two_cuts_with_different_metadata_do_not_select_a_branch_silently() {
    let b = doc("- Released.\n");
    let o = "## Unreleased\n\n## [1.0.0] - 2026-10-19\n\n### Added\n\n- Released.\n";
    let t = o.replace("2026-10-19", "2026-10-20");
    for (o, t) in [(o, &*t), (&*t, o)] {
        assert!(
            !merge(&b, o, t, &MergeOptions::default())
                .unwrap()
                .conflicts
                .is_empty()
        );
    }
}
#[test]
fn release_cut_preserves_multiple_categories_and_links() {
    let b = "# Changelog\n\n## Unreleased\n\n### Added\n\n- Feature.\n\n### Fixed\n\n- Bug.\n\n[Unreleased]: /compare/old...HEAD\n";
    let o = "# Changelog\n\n## Unreleased\n\n## [1.0.0] - 2026-10-19\n\n### Added\n\n- Feature.\n\n### Fixed\n\n- Bug.\n\n[Unreleased]: /compare/v1...HEAD\n[1.0.0]: /tag/v1\n";
    let t = b
        .replace("- Feature.", "- Post-cut.\n\n- Feature.")
        .replace("[Unreleased]:", "[new-api]: /api\n[Unreleased]:");
    let r = clean(b, o, &t);
    let d = parse(&r).unwrap();
    assert_eq!(
        d.releases[0].categories[0].entries[0].normalized,
        "- Post-cut."
    );
    assert_eq!(d.releases[1].categories.len(), 2);
    assert!(r.contains("/compare/v1...HEAD"));
    assert!(r.contains("[new-api]: /api"));
}
#[test]
fn edited_moves_competing_moves_and_move_deletion() {
    let b = "## Unreleased\n### Added\n- SSH importer automatic retries enabled.\n### Fixed\n- Anchor.\n";
    let moved = "## Unreleased\n### Added\n### Fixed\n- SSH importer automatic retries enabled.\n- Anchor.\n";
    for t in [
        b.replace("- SSH importer automatic retries enabled.\n", ""),
        "## Unreleased\n### Added\n### Fixed\n- Anchor.\n### Security\n- SSH importer automatic retries enabled.\n".to_owned(),
    ] {
        let r = merge(b, moved, &t, &MergeOptions::default()).unwrap();
        assert!(!r.conflicts.is_empty());
    }
    let o = moved.replace(
        "automatic retries enabled",
        "automatic retries enabled by default",
    );
    let t = b.replace("- Anchor.", "- Anchor.\n- Independent.");
    let r = clean(b, &o, &t);
    assert_eq!(r.matches("SSH importer").count(), 1);
    assert!(r.contains("by default"));
}
#[test]
fn multline_references_and_opaque_insertion_conflicts() {
    let b = "## Unreleased\n\n[a]: /a\n  \"First title\"\n[b]: /b\n  \"Second title\"\n";
    let o = b.replace("/a", "/a-new");
    let t = b.replace("Second title", "Second edited");
    let r = clean(b, &o, &t);
    assert!(r.contains("[a]: /a-new\n  \"First title\""));
    assert!(r.contains("Second edited"));
    let b = doc("- First.\n\n<!-- original -->\n\n- Last.\n");
    let o = b.replace("original", "ours");
    let t = b.replace("original", "theirs");
    assert!(
        !merge(&b, &o, &t, &MergeOptions::default())
            .unwrap()
            .conflicts
            .is_empty()
    );
}
#[test]
fn independent_moves_keep_each_branch_order() {
    let b = "## Unreleased\n### Added\n- A.\n- B.\n- C.\n- D.\n### Fixed\n";
    let o = "## Unreleased\n### Added\n- C.\n- D.\n### Fixed\n- A.\n- B.\n";
    let t = "## Unreleased\n### Added\n- A.\n- B.\n### Fixed\n- C.\n- D.\n";
    let r = clean(b, o, t);
    assert_eq!(r, clean(b, t, o));
    let d = parse(&r).unwrap();
    assert_eq!(d.releases[0].categories[1].entries.len(), 4);
    assert!(r.find("- A.") < r.find("- B."));
    assert!(r.find("- C.") < r.find("- D."));
}
#[test]
fn competing_unidentifiable_section_replacements_conflict() {
    let b = doc("- Original.\n");
    let o = b.replace("Added", "Features").replace("Original", "SSH");
    let t = b.replace("Added", "Enhancements").replace("Original", "S3");
    assert!(
        !merge(&b, &o, &t, &MergeOptions::default())
            .unwrap()
            .conflicts
            .is_empty()
    );
}
#[test]
fn interleaved_reference_edit_combines_with_neighboring_prose_edit() {
    let b = doc("- First.\n\n[api]: /old \"API title\"\n\nRead the API notes.\n\n- Last.\n");
    let o = b.replace("/old", "/new");
    let t = b.replace("Read the API notes.", "Read the revised API notes.");
    let r = clean(&b, &o, &t);
    assert!(r.contains("[api]: /new \"API title\""));
    assert!(r.contains("Read the revised API notes."));
}
#[test]
fn entry_to_opaque_rewrite_never_enables_internal_line_merging() {
    let b = doc("- Before.\n\n- Feature.\n\n  Existing detail.\n\n- After.\n");
    let o = b.replace("- Feature.", "Feature.");
    let t = b.replace("Existing detail.", "Revised detail.");
    let result = merge(&b, &o, &t, &MergeOptions::default()).unwrap();
    assert!(!result.conflicts.is_empty());
    assert!(result.content.contains("Existing detail."));
    assert!(result.content.contains("Revised detail."));
}
