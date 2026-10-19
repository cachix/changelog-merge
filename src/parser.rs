use crate::{document::*, error::ParseError, identity};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::HashSet;
const MAX_INPUT: usize = 32 * 1024 * 1024;
pub fn parse(source: &str) -> Result<Document, ParseError> {
    if source.len() > MAX_INPUT {
        return Err(ParseError::new("input exceeds 32 MiB"));
    }
    if source.lines().any(|line| {
        let Some(first) = line.as_bytes().first() else {
            return false;
        };
        if !b"<|=>".contains(first) {
            return false;
        }
        let count = line.bytes().take_while(|b| b == first).count();
        count >= 3 && (line[count..].is_empty() || line[count..].starts_with(char::is_whitespace))
    }) {
        return Err(ParseError::new("existing conflict markers"));
    }
    let mut headings = Vec::new();
    let mut items = Vec::new();
    let mut depth = 0;
    let mut item = None;
    let mut heading = None;
    let mut nesting = 0;
    for (event_index, (event, range)) in Parser::new(source).into_offset_iter().enumerate() {
        if event_index >= 2_000_000
            || items.len() + headings.len() > 100_000
            || headings.len() > 10_000
        {
            return Err(ParseError::new(
                "document exceeds the structural complexity budget",
            ));
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) if nesting == 0 => {
                heading = Some((level, range.start, range.end, String::new()));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(h) = &mut heading {
                    h.3.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, end, text)) = heading.take() {
                    headings.push((level, Span { start, end }, text));
                }
            }
            Event::Start(Tag::List(_)) => {
                depth += 1;
                nesting += 1;
            }
            Event::End(TagEnd::List(_)) => {
                depth -= 1;
                nesting -= 1;
            }
            Event::Start(Tag::BlockQuote(_)) => nesting += 1,
            Event::End(TagEnd::BlockQuote(_)) => nesting -= 1,
            Event::Start(Tag::Item) if depth == 1 && nesting == 1 => item = Some(range.start),
            Event::End(TagEnd::Item) if depth == 1 && nesting == 1 => {
                if let Some(start) = item.take() {
                    items.push(Span {
                        start,
                        end: range.end,
                    });
                }
            }
            _ => (),
        }
    }
    if items.len() + headings.len() > 100_000 || headings.len() > 10_000 {
        return Err(ParseError::new(
            "document exceeds the structural complexity budget",
        ));
    }
    let releases: Vec<_> = headings
        .iter()
        .filter(|h| h.0 == HeadingLevel::H2)
        .collect();
    let first = releases.first().map_or(source.len(), |h| h.1.start);
    // Only trailing definitions form the document footer. Earlier definitions
    // remain in their owning opaque region (or within an atomic list entry).
    let structure_end = headings
        .iter()
        .map(|h| h.1.end)
        .chain(items.iter().map(|s| s.end))
        .max()
        .unwrap_or(source.len());
    let parser = Parser::new(source);
    let footer_start = parser
        .reference_definitions()
        .iter()
        .map(|(_, d)| d.span.start)
        .filter(|p| *p >= first && *p >= structure_end)
        .min()
        .unwrap_or(source.len());
    let mut doc = Document {
        source: source.into(),
        preamble: Span {
            start: 0,
            end: first.min(footer_start),
        },
        releases: Vec::new(),
        footer: (footer_start < source.len()).then_some(Span {
            start: footer_start,
            end: source.len(),
        }),
        opaque_blocks: Vec::new(),
    };
    let mut seen = HashSet::new();
    for (i, h) in releases.iter().enumerate() {
        let id =
            identity::release(&h.2).ok_or_else(|| ParseError::new("ambiguous release identity"))?;
        if !seen.insert(id.clone()) {
            return Err(ParseError::new("duplicate release identity"));
        }
        let end = releases.get(i + 1).map_or(footer_start, |n| n.1.start);
        let from = headings.partition_point(|c| c.1.start <= h.1.start);
        let to = headings.partition_point(|c| c.1.start < end);
        let cats: Vec<_> = headings[from..to]
            .iter()
            .filter(|c| c.0 == HeadingLevel::H3)
            .collect();
        let prefix = Span {
            start: h.1.end,
            end: cats.first().map_or(end, |c| c.1.start),
        };
        let mut release = Release {
            id,
            heading: h.1,
            categories: Vec::new(),
            span: Span {
                start: h.1.start,
                end,
            },
            prefix,
        };
        let mut seen = HashSet::new();
        for (j, c) in cats.iter().enumerate() {
            let id = c.2.trim().to_lowercase();
            if id.is_empty() || !seen.insert(id.clone()) {
                return Err(ParseError::new("empty or duplicate category identity"));
            }
            let end = cats.get(j + 1).map_or(end, |n| n.1.start);
            let hfrom = headings.partition_point(|x| x.1.start <= c.1.start);
            let hto = headings.partition_point(|x| x.1.start < end);
            if hto > hfrom {
                return Err(ParseError::new("unsupported heading hierarchy in category"));
            }
            let from = items.partition_point(|s| s.start < c.1.end);
            let to = items.partition_point(|s| s.start < end);
            let selected: Vec<_> = items[from..to]
                .iter()
                .filter(|s| s.end <= end)
                .copied()
                .collect();
            let mut entries = Vec::new();
            let mut blocks = Vec::new();
            for (k, s) in selected.iter().enumerate() {
                let next = selected.get(k + 1).map_or(s.end, |n| n.start);
                let opaque = !source[s.end..next].trim().is_empty();
                let span = Span {
                    start: s.start,
                    end: if opaque { s.end } else { next },
                };
                let normalized = identity::normalized(span.text(source));
                entries.push(Entry {
                    span,
                    fingerprint: *blake3::hash(normalized.as_bytes()).as_bytes(),
                    normalized,
                    anchor: EntryAnchor { position: k },
                });
                blocks.push(Block::Entry(k));
                if opaque {
                    let span = Span {
                        start: s.end,
                        end: next,
                    };
                    blocks.push(Block::Opaque(span));
                    doc.opaque_blocks.push(OpaqueBlock { span });
                }
            }
            let prefix = Span {
                start: c.1.end,
                end: selected.first().map_or(end, |s| s.start),
            };
            let suffix = Span {
                start: selected.last().map_or(end, |s| s.end),
                end,
            };
            doc.opaque_blocks.push(OpaqueBlock { span: prefix });
            doc.opaque_blocks.push(OpaqueBlock { span: suffix });
            release.categories.push(Category {
                id,
                heading: c.1,
                entries,
                span: Span {
                    start: c.1.start,
                    end,
                },
                prefix,
                suffix,
                blocks,
            });
        }
        doc.opaque_blocks.push(OpaqueBlock { span: prefix });
        doc.releases.push(release);
    }
    if headings
        .iter()
        .any(|h| h.0 == HeadingLevel::H3 && h.1.start < first)
    {
        return Err(ParseError::new("category outside a release"));
    }
    Ok(doc)
}
