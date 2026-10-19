use changelog_merge::parse;
#[test]
fn nested_markdown_and_source_ownership() {
    let s = include_str!("fixtures/casita.md");
    let d = parse(s).unwrap();
    assert_eq!(d.source, s);
    assert_eq!(d.releases.len(), 2);
    let c = &d.releases[0].categories[0];
    assert_eq!(c.entries.len(), 2);
    assert!(c.entries[0].span.text(s).contains("```rust"));
    assert!(c.entries[0].span.text(s).contains("continuation"));
    assert!(d.footer.unwrap().text(s).starts_with("[Unreleased]:"));
    let mut spans = vec![d.preamble];
    for r in &d.releases {
        spans.extend([r.heading, r.prefix]);
        for c in &r.categories {
            spans.extend([c.heading, c.prefix]);
            spans.extend(c.entries.iter().map(|e| e.span));
            spans.push(c.suffix);
        }
    }
    spans.extend(d.footer);
    let rebuilt: String = spans.iter().map(|p| p.text(s)).collect();
    assert_eq!(rebuilt, s);
}
#[test]
fn variants_and_duplicates() {
    for ending in ["\n", "\r\n"] {
        for final_newline in [false, true] {
            let s = format!("# Changelog\n\n<!-- note -->\n\n## Unreleased\n\n### CuStOm\n\n- café `名`.\n- café `名`.{}",if final_newline {"\n"} else {""}).replace('\n',ending);
            let d = parse(&s).unwrap();
            assert_eq!(d.releases[0].categories[0].id, "custom");
            assert_eq!(d.releases[0].categories[0].entries.len(), 2);
        }
    }
    for s in [
        "",
        "# Changelog\n",
        "## [Unreleased]\n",
        "## [1.0.0] - 2026-10-19\n",
    ] {
        parse(s).unwrap();
    }
}
#[test]
fn reject_ambiguity() {
    for s in [
        "## [Unreleased]\n## Unreleased\n",
        "## Unreleased\n### Added\n### added\n",
        "### Added\n- no release\n",
        "## Unreleased\n### Added\n#### Bad\n",
        "<<<<<<< ours\n",
        "## two words\n",
    ] {
        assert!(parse(s).is_err(), "{s}");
    }
}
#[test]
fn fenced_headings_are_not_sections() {
    let d = parse("## Unreleased\n### Fixed\n- Example:\n\n  ```md\n  ## [Fake]\n  - nested\n  ```\n\n- Next.\n").unwrap();
    assert_eq!(d.releases.len(), 1);
    assert_eq!(d.releases[0].categories[0].entries.len(), 2);
}
#[test]
fn custom_conflict_marker_sizes_are_rejected() {
    for size in [3, 7, 11, 256] {
        assert!(parse(&format!("{} ours\n- unresolved\n", "<".repeat(size))).is_err());
    }
}
#[test]
fn pathological_structure_is_bounded() {
    let headings: String = (0..10_001).map(|i| format!("## [release-{i}]\n")).collect();
    let Err(error) = parse(&headings) else {
        panic!("heading budget must be enforced");
    };
    assert!(error.message.contains("complexity budget"));
    let list = format!(
        "## Unreleased\n### Added\n{}",
        "- repeated\n".repeat(100_001)
    );
    let Err(error) = parse(&list) else {
        panic!("block budget must be enforced");
    };
    assert!(error.message.contains("complexity budget"));
}
#[test]
fn opaque_and_entry_spans_partition_the_source() {
    use changelog_merge::document::Block;
    let source = "# Changelog\n\n## Unreleased\n\n### Added\n\n- First.\n\n<!-- comment -->\n\n[api]: /api\n\nParagraph.\n\n- Last.\n\n[footer]: /footer\n";
    let d = parse(source).unwrap();
    let mut slices = vec![d.preamble.text(source)];
    for r in &d.releases {
        slices.extend([r.heading.text(source), r.prefix.text(source)]);
        for c in &r.categories {
            slices.extend([c.heading.text(source), c.prefix.text(source)]);
            for block in &c.blocks {
                slices.push(match block {
                    Block::Entry(i) => c.entries[*i].span.text(source),
                    Block::Opaque(s) => s.text(source),
                });
            }
            slices.push(c.suffix.text(source));
        }
    }
    if let Some(footer) = d.footer {
        slices.push(footer.text(source));
    }
    assert_eq!(slices.concat(), source);
}
