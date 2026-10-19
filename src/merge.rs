use crate::{
    conflict::*, diagnostics::*, document::*, error::MergeError, identity, matching, ordering,
    parse, render,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    pub strict: bool,
    pub conflict_style: ConflictStyle,
}
#[derive(Debug, Clone)]
pub struct MergeOutcome {
    pub content: String,
    pub conflicts: Vec<MergeConflict>,
    pub diagnostics: Vec<DiagnosticEvent>,
}
#[derive(Clone, Default)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) raw: String,
    pub(crate) head: String,
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) children: Vec<Node>,
    pub(crate) span: Option<Span>,
    pub(crate) opaque: bool,
    // Relocated counterparts carry content identity, but no destination ordering intent.
    // Containers propagate this flag so raw equality cannot bypass order reconciliation.
    pub(crate) virtual_order: bool,
    pub(crate) head_span: Option<Span>,
    pub(crate) prefix_span: Option<Span>,
    pub(crate) suffix_span: Option<Span>,
}
fn entry(e: &Entry, s: &str) -> Node {
    Node {
        id: e.normalized.clone(),
        raw: e.span.text(s).into(),
        span: Some(e.span),
        ..Node::default()
    }
}
fn category(c: &Category, s: &str) -> Node {
    Node {
        id: c.id.clone(),
        raw: c.span.text(s).into(),
        head: c.heading.text(s).into(),
        prefix: c.prefix.text(s).into(),
        suffix: c.suffix.text(s).into(),
        children: c
            .blocks
            .iter()
            .map(|block| match block {
                Block::Entry(i) => entry(&c.entries[*i], s),
                Block::Opaque(span) => Node {
                    id: identity::normalized(span.text(s)),
                    raw: span.text(s).into(),
                    span: Some(*span),
                    opaque: true,
                    ..Node::default()
                },
            })
            .collect(),
        span: Some(c.span),
        opaque: false,
        head_span: Some(c.heading),
        prefix_span: Some(c.prefix),
        suffix_span: Some(c.suffix),
        virtual_order: false,
    }
}
fn document(d: &Document) -> Node {
    let s = &d.source;
    Node {
        raw: s.clone(),
        prefix: d.preamble.text(s).into(),
        prefix_span: Some(d.preamble),
        suffix_span: d.footer,
        suffix: d.footer.map_or("", |f| f.text(s)).into(),
        children: d
            .releases
            .iter()
            .map(|r| Node {
                id: r.id.clone(),
                raw: r.span.text(s).into(),
                head: r.heading.text(s).into(),
                prefix: r.prefix.text(s).into(),
                children: r.categories.iter().map(|c| category(c, s)).collect(),
                span: Some(r.span),
                head_span: Some(r.heading),
                prefix_span: Some(r.prefix),
                ..Node::default()
            })
            .collect(),
        ..Node::default()
    }
}
struct Engine<'a> {
    options: &'a MergeOptions,
    conflicts: Vec<MergeConflict>,
    release: Option<String>,
    category: Option<String>,
    selected: Vec<(String, String, String)>,
    references: Vec<(String, String)>,
    headings: Vec<String>,
    diagnostics: Vec<DiagnosticEvent>,
}
impl Engine<'_> {
    fn event(&mut self, kind: EventKind, message: impl Into<String>, order: Vec<String>) {
        self.diagnostics.push(DiagnosticEvent {
            kind,
            branch: None,
            release: self.release.clone(),
            category: self.category.clone(),
            base_span: None,
            branch_span: None,
            message: message.into(),
            order,
        });
    }
    fn fragment(&mut self, text: &str) {
        if text.contains("]:") {
            self.references.extend(
                crate::text_merge::definitions(text)
                    .into_iter()
                    .map(|(key, span)| (key, identity::normalized(&text[span]))),
            );
        }
    }
    fn selected_entry(&mut self, text: &str) {
        self.selected.push((
            self.release.clone().unwrap_or_default(),
            self.category.clone().unwrap_or_default(),
            identity::normalized(text),
        ));
        self.fragment(text);
    }
    fn select(&mut self, n: &Node, level: usize) {
        if level == 3 {
            if !n.raw.is_empty() && !n.opaque {
                self.selected_entry(&n.raw);
            } else {
                self.fragment(&n.raw);
            }
        } else {
            let (old_release, old_category) = (self.release.clone(), self.category.clone());
            if !n.head.is_empty() {
                self.headings.push(identity::normalized(&n.head));
                if level == 1 {
                    self.release = Some(heading_id(&n.head, level));
                }
                if level == 2 {
                    self.category = Some(heading_id(&n.head, level));
                }
            }
            let order = n
                .children
                .iter()
                .map(|child| {
                    if level == 2 {
                        entry_key(child)
                    } else {
                        heading_id(&child.head, level + 1)
                    }
                })
                .collect();
            self.event(EventKind::Ordered, "preserved selected source order", order);
            self.fragment(&n.prefix);
            self.fragment(&n.suffix);
            for child in &n.children {
                self.select(child, level + 1);
            }
            self.release = old_release;
            self.category = old_category;
        }
    }
    fn conflict(
        &mut self,
        kind: ConflictKind,
        b: &Node,
        o: &Node,
        t: &Node,
        message: &str,
    ) -> String {
        self.event(EventKind::Conflict, message, Vec::new());
        self.conflicts.push(MergeConflict {
            kind,
            release: self.release.clone(),
            category: self.category.clone(),
            base_span: b.span,
            ours_span: o.span,
            theirs_span: t.span,
            message: message.into(),
        });
        markers(&b.raw, &o.raw, &t.raw, &self.options.conflict_style)
    }
    fn scalar(&mut self, b: &str, o: &str, t: &str) -> String {
        self.scalar_at(b, o, t, [None; 3])
    }
    fn scalar_at(&mut self, b: &str, o: &str, t: &str, spans: [Option<Span>; 3]) -> String {
        if let Some(result) = crate::text_merge::merge(b, o, t) {
            if o != b && t != b && o != t {
                self.event(
                    if crate::text_merge::definitions(&result).is_empty() {
                        EventKind::OpaqueMerged
                    } else {
                        EventKind::ReferenceMerged
                    },
                    "reconciled independent opaque changes",
                    Vec::new(),
                );
            }
            self.fragment(&result);
            return result;
        }
        self.conflict(
            ConflictKind::OpaqueContent,
            &Node {
                raw: b.into(),
                span: spans[0],
                ..Node::default()
            },
            &Node {
                raw: o.into(),
                span: spans[1],
                ..Node::default()
            },
            &Node {
                raw: t.into(),
                span: spans[2],
                ..Node::default()
            },
            "overlapping changes to opaque content or reference definitions",
        )
    }
    fn node(&mut self, b: &Node, o: &Node, t: &Node, level: usize) -> String {
        let preserve_order = level < 3 && (b.virtual_order || o.virtual_order || t.virtual_order);
        if !preserve_order && (o.raw == t.raw || t.raw == b.raw) {
            self.select(o, level);
            return o.raw.clone();
        }
        if !preserve_order && o.raw == b.raw {
            self.select(t, level);
            return t.raw.clone();
        }
        if level == 3 {
            if b.opaque {
                return self.scalar_at(&b.raw, &o.raw, &t.raw, [b.span, o.span, t.span]);
            }
            if o.opaque || t.opaque {
                return self.conflict(
                    ConflictKind::ConcurrentModification,
                    b,
                    o,
                    t,
                    "entry-to-opaque rewrite competes with another entry edit",
                );
            }
            let split = |raw: &str| {
                let end = raw.trim_end_matches(['\r', '\n']).len();
                (raw[..end].to_owned(), raw[end..].to_owned())
            };
            let (bc, oc, tc) = (split(&b.raw), split(&o.raw), split(&t.raw));
            let (bn, on, tn) = (
                identity::normalized(&bc.0),
                identity::normalized(&oc.0),
                identity::normalized(&tc.0),
            );
            let body = if on == tn || bn == tn {
                Some(oc.0.as_str())
            } else if bn == on {
                Some(tc.0.as_str())
            } else {
                None
            };
            if let Some(body) = body {
                let mut text = body.to_owned();
                if !body.is_empty() {
                    text.push_str(&self.scalar(&bc.1, &oc.1, &tc.1));
                    self.selected_entry(&text);
                }
                return text;
            }
            let kind = if o.raw.is_empty() || t.raw.is_empty() {
                ConflictKind::DeleteModify
            } else {
                ConflictKind::ConcurrentModification
            };
            return self.conflict(kind, b, o, t, "incompatible changes to an existing entry");
        }
        if !b.raw.is_empty() && (o.raw.is_empty() || t.raw.is_empty()) {
            return self.conflict(
                ConflictKind::SectionDeletion,
                b,
                o,
                t,
                "section deleted while the other branch changed it",
            );
        }
        if o.head != b.head && t.head != b.head && o.head != t.head && !b.head.is_empty() {
            return self.conflict(
                ConflictKind::SectionRename,
                b,
                o,
                t,
                "both branches changed the section heading differently",
            );
        }
        let mut out = self.scalar_at(
            &b.head,
            &o.head,
            &t.head,
            [b.head_span, o.head_span, t.head_span],
        );
        if !out.is_empty() {
            self.headings.push(identity::normalized(&out));
        }
        let (old_release, old_category) = (self.release.clone(), self.category.clone());
        if level == 1 {
            self.release = Some(heading_id(&out, level));
        }
        if level == 2 {
            self.category = Some(heading_id(&out, level));
        }
        let children = if level == 2 {
            self.entries(b, o, t)
        } else {
            self.sections(b, o, t, level)
        };
        let children = match children {
            Ok(children) => children,
            Err((kind, message)) => {
                let text = self.conflict(kind, b, o, t, message);
                self.release = old_release;
                self.category = old_category;
                return text;
            }
        };
        out.push_str(&self.scalar_at(
            &b.prefix,
            &o.prefix,
            &t.prefix,
            [b.prefix_span, o.prefix_span, t.prefix_span],
        ));
        render::append(&mut out, &children);
        let suffix = self.scalar_at(
            &b.suffix,
            &o.suffix,
            &t.suffix,
            [b.suffix_span, o.suffix_span, t.suffix_span],
        );
        render::append(&mut out, &suffix);
        self.release = old_release;
        self.category = old_category;
        out
    }
    fn sections(
        &mut self,
        b: &Node,
        o: &Node,
        t: &Node,
        level: usize,
    ) -> Result<String, (ConflictKind, &'static str)> {
        let ids = |n: &Node| n.children.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let get = |n: &Node, id: &str| {
            n.children
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .unwrap_or_default()
        };
        let Some(order) = ordering::three_way(&ids(b), &ids(o), &ids(t)) else {
            return Err((ConflictKind::Ordering, "contradictory section order"));
        };
        self.event(EventKind::Ordered, "resolved section order", order.clone());
        let mut out = String::new();
        for id in order {
            if level == 0 {
                self.release = Some(id.clone());
            } else {
                self.category = Some(id.clone());
            }
            let text = self.node(&get(b, &id), &get(o, &id), &get(t, &id), level + 1);
            render::append(&mut out, &text);
            if level == 0 {
                self.release = None;
            } else {
                self.category = None;
            }
        }
        Ok(out)
    }
    fn entries(
        &mut self,
        b: &Node,
        o: &Node,
        t: &Node,
    ) -> Result<String, (ConflictKind, &'static str)> {
        let ids = |n: &Node| n.children.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        let (Ok(oa), Ok(ta)) = (
            matching::align(&ids(b), &ids(o)),
            matching::align(&ids(b), &ids(t)),
        ) else {
            return Err((
                ConflictKind::AmbiguousEntryIdentity,
                "ambiguous rewrite or entry reordering",
            ));
        };
        if self.options.strict && (!oa.inferred.is_empty() || !ta.inferred.is_empty()) {
            return Err((
                ConflictKind::AmbiguousEntryIdentity,
                "strict mode rejects inferred entry edits",
            ));
        }
        let base_id = |i: usize| format!("b:{:020}", b.children[i].span.map_or(i, |s| s.start));
        let mut values = BTreeMap::new();
        let mut selected = BTreeSet::new();
        for i in 0..b.children.len() {
            let empty = Node::default();
            let on = oa.matched[i].map_or(&empty, |j| &o.children[j]);
            let tn = ta.matched[i].map_or(&empty, |j| &t.children[j]);
            let value = self.node(&b.children[i], on, tn, 3);
            if !value.is_empty() {
                let id = base_id(i);
                selected.insert(id.clone());
                values.insert(id, value);
            }
        }
        let mut sequences = Vec::new();
        let mut additions: BTreeMap<String, (String, bool)> = BTreeMap::new();
        for (branch, alignment) in [(o, &oa), (t, &ta)] {
            let reverse: BTreeMap<_, _> = alignment
                .matched
                .iter()
                .enumerate()
                .filter_map(|(i, j)| j.map(|j| (j, i)))
                .collect();
            let mut occurrence = BTreeMap::new();
            let mut sequence = Vec::new();
            let mut before = vec![None; branch.children.len()];
            let mut previous = None;
            for (j, slot) in before.iter_mut().enumerate() {
                *slot = previous;
                if let Some(i) = reverse.get(&j) {
                    previous = Some(*i);
                }
            }
            let mut after = vec![None; branch.children.len()];
            let mut next = None;
            for j in (0..branch.children.len()).rev() {
                after[j] = next;
                if let Some(i) = reverse.get(&j) {
                    next = Some(*i);
                }
            }
            for (j, child) in branch.children.iter().enumerate() {
                let id = if let Some(i) = reverse.get(&j) {
                    base_id(*i)
                } else {
                    let canonical = child.raw.replace("\r\n", "\n");
                    let anchor = format!("{:?}:{:?}", before[j], after[j]);
                    let key = (anchor.clone(), canonical.clone(), child.opaque);
                    let count = occurrence.entry(key).or_insert(0usize);
                    let id = format!("a:{canonical}\0{anchor}\0{count:020}");
                    *count += 1;
                    if let Some((raw, _)) = additions.get_mut(&id) {
                        self.event(
                            EventKind::Deduplicated,
                            format!("identical addition at {anchor}"),
                            vec![entry_key(child)],
                        );
                        if child.raw < *raw {
                            *raw = child.raw.clone();
                        }
                    } else {
                        additions.insert(id.clone(), (child.raw.clone(), child.opaque));
                    }
                    selected.insert(id.clone());
                    id
                };
                if selected.contains(&id) && !child.virtual_order {
                    sequence.push(id);
                }
            }
            sequences.push(sequence);
        }
        let base_order: Vec<_> = (0..b.children.len())
            .filter(|&i| !b.children[i].virtual_order)
            .map(base_id)
            .filter(|id| selected.contains(id))
            .collect();
        let Some(order) = ordering::three_way(&base_order, &sequences[0], &sequences[1]) else {
            return Err((
                ConflictKind::Ordering,
                "concurrent entry order constraints form a cycle",
            ));
        };
        let display_order = order
            .iter()
            .map(|id| {
                if let Some((raw, _)) = additions.get(id) {
                    blake3::hash(identity::normalized(raw).as_bytes())
                        .to_hex()
                        .to_string()
                } else {
                    id.clone()
                }
            })
            .collect();
        self.event(EventKind::Ordered, "resolved entry order", display_order);
        for (id, (raw, opaque)) in additions {
            if !opaque {
                self.selected_entry(&raw);
            } else {
                self.fragment(&raw);
            }
            values.insert(id, raw);
        }
        let mut out = String::new();
        for id in order {
            render::append(&mut out, &values[&id]);
        }
        Ok(out)
    }
}
fn entry_key(n: &Node) -> String {
    blake3::hash(n.id.as_bytes()).to_hex().to_string()
}

pub fn merge(
    base: &str,
    ours: &str,
    theirs: &str,
    options: &MergeOptions,
) -> Result<MergeOutcome, MergeError> {
    let style = &options.conflict_style;
    if !(3..=256).contains(&style.marker_size)
        || [&style.base_label, &style.ours_label, &style.theirs_label]
            .iter()
            .any(|s| s.contains(['\n', '\r']))
    {
        return Err(MergeError::Validation(
            "invalid conflict marker size or label".into(),
        ));
    }
    let (bd, od, td) = (parse(base)?, parse(ours)?, parse(theirs)?);
    let (mut b, mut o, mut t) = (document(&bd), document(&od), document(&td));
    let mut engine = Engine {
        options,
        conflicts: Vec::new(),
        release: None,
        category: None,
        selected: Vec::new(),
        references: Vec::new(),
        headings: Vec::new(),
        diagnostics: Vec::new(),
    };
    if ours == theirs || base == theirs || base == ours {
        audit(&b, &o, "ours", &mut engine.diagnostics);
        audit(&b, &t, "theirs", &mut engine.diagnostics);
        engine.select(if base == ours { &t } else { &o }, 0);
        return Ok(MergeOutcome {
            content: if base == ours { theirs } else { ours }.into(),
            conflicts: Vec::new(),
            diagnostics: engine.diagnostics,
        });
    }
    let hazard = crate::relocation::normalize(&mut b, &mut o, &mut t, &mut engine.diagnostics);
    normalize_insertions(&b, &mut o, render::ending(base), 0);
    normalize_insertions(&b, &mut t, render::ending(base), 0);
    audit(&b, &o, "ours", &mut engine.diagnostics);
    audit(&b, &t, "theirs", &mut engine.diagnostics);
    let content = if let Err((kind, message)) = hazard {
        engine.conflict(
            kind,
            &document(&bd),
            &document(&od),
            &document(&td),
            message,
        )
    } else {
        engine.node(&b, &o, &t, 0)
    };
    if engine.conflicts.is_empty() {
        let result = parse(&content).map_err(|e| MergeError::Validation(e.to_string()))?;
        let mut actual_headings: Vec<_> = result
            .releases
            .iter()
            .flat_map(|r| std::iter::once(r.heading).chain(r.categories.iter().map(|c| c.heading)))
            .map(|s| identity::normalized(s.text(&result.source)))
            .collect();
        actual_headings.sort();
        engine.headings.sort();
        if actual_headings != engine.headings {
            return Err(MergeError::Validation(
                "rendered headings differ from selected sections".into(),
            ));
        }
        let mut actual: Vec<_> = result
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
        actual.sort();
        engine.selected.sort();
        let mut actual_references: Vec<_> = crate::text_merge::definitions(&content)
            .into_iter()
            .map(|(key, span)| (key, identity::normalized(&content[span])))
            .collect();
        actual_references.sort();
        engine.references.sort();
        if actual_references != engine.references {
            return Err(MergeError::Validation(
                "rendered reference definitions differ from selected definitions".into(),
            ));
        }
        if actual != engine.selected {
            return Err(MergeError::Validation(
                "rendered entry inventory differs from selected entries".into(),
            ));
        }
    }
    Ok(MergeOutcome {
        content,
        conflicts: engine.conflicts,
        diagnostics: engine.diagnostics,
    })
}

fn audit(b: &Node, branch: &Node, label: &str, events: &mut Vec<DiagnosticEvent>) {
    let mut event = |kind, r: &str, c: Option<&str>, b, s, message| {
        events.push(DiagnosticEvent {
            kind,
            branch: Some(label.into()),
            release: (!r.is_empty()).then(|| r.into()),
            category: c.map(str::to_owned),
            base_span: b,
            branch_span: s,
            message,
            order: Vec::new(),
        });
    };
    for (kind, bs, ns, message) in opaque_changes(b, branch) {
        event(kind, "", None, bs, ns, message.into());
    }
    for r in &branch.children {
        let br = b.children.iter().find(|n| n.id == r.id);
        event(
            if br.is_some_and(|n| !n.head.is_empty()) {
                EventKind::MatchedRelease
            } else {
                EventKind::Added
            },
            &r.id,
            None,
            br.and_then(|r| r.span),
            r.span,
            format!("release {}", r.id),
        );
        if br.is_some_and(|n| !n.head.is_empty() && n.head != r.head) {
            event(
                if br.is_some_and(|n| heading_id(&n.head, 1) == heading_id(&r.head, 1)) {
                    EventKind::Modified
                } else {
                    EventKind::Renamed
                },
                &r.id,
                None,
                br.and_then(|r| r.span),
                r.span,
                r.head.trim().into(),
            );
        }
        let empty_release = Node::default();
        for (kind, bs, ns, message) in opaque_changes(br.unwrap_or(&empty_release), r) {
            event(kind, &r.id, None, bs, ns, message.into());
        }
        for c in &r.children {
            let bc = br.and_then(|r| r.children.iter().find(|n| n.id == c.id));
            event(
                if bc.is_some_and(|n| !n.head.is_empty()) {
                    EventKind::MatchedCategory
                } else {
                    EventKind::Added
                },
                &r.id,
                Some(&c.id),
                bc.and_then(|c| c.span),
                c.span,
                format!("category {}", c.id),
            );
            if bc.is_some_and(|n| !n.head.is_empty() && n.head != c.head) {
                event(
                    EventKind::Renamed,
                    &r.id,
                    Some(&c.id),
                    bc.and_then(|c| c.span),
                    c.span,
                    c.head.trim().into(),
                );
            }
            let empty = Node::default();
            let bc = bc.unwrap_or(&empty);
            for (kind, bs, ns, message) in opaque_changes(bc, c) {
                event(kind, &r.id, Some(&c.id), bs, ns, message.into());
            }
            let ids = |n: &Node| n.children.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
            if let Ok(alignment) = matching::align(&ids(bc), &ids(c)) {
                for (i, j) in alignment.matched.iter().enumerate() {
                    if j.is_none_or(|j| bc.children[i].raw != c.children[j].raw) {
                        let kind = if j.is_none() {
                            EventKind::Deleted
                        } else if j.is_some_and(|j| bc.children[i].id == c.children[j].id) {
                            EventKind::FormattingSelected
                        } else {
                            EventKind::Modified
                        };
                        event(
                            kind,
                            &r.id,
                            Some(&c.id),
                            bc.children[i].span,
                            j.and_then(|j| c.children[j].span),
                            entry_description(&bc.children[i]),
                        );
                    }
                }
                for j in alignment.additions {
                    event(
                        EventKind::Added,
                        &r.id,
                        Some(&c.id),
                        None,
                        c.children[j].span,
                        entry_description(&c.children[j]),
                    );
                }
            }
        }
    }
    for r in &b.children {
        if !branch.children.iter().any(|n| n.id == r.id) {
            event(
                EventKind::Deleted,
                &r.id,
                None,
                r.span,
                None,
                format!("release {}", r.id),
            );
        }
        for c in &r.children {
            if !branch
                .children
                .iter()
                .any(|n| n.id == r.id && n.children.iter().any(|n| n.id == c.id))
            {
                event(
                    EventKind::Deleted,
                    &r.id,
                    Some(&c.id),
                    c.span,
                    None,
                    format!("category {}", c.id),
                );
                for e in &c.children {
                    event(
                        EventKind::Deleted,
                        &r.id,
                        Some(&c.id),
                        e.span,
                        None,
                        entry_description(e),
                    );
                }
            }
        }
    }
}

fn normalize_insertions(base: &Node, branch: &mut Node, template: &str, level: usize) {
    fn convert(n: &mut Node, template: &str) {
        n.raw = render::line_endings(&n.raw, template);
        n.head = render::line_endings(&n.head, template);
        n.prefix = render::line_endings(&n.prefix, template);
        n.suffix = render::line_endings(&n.suffix, template);
        for child in &mut n.children {
            convert(child, template);
        }
    }
    if base.head.is_empty() && !branch.head.is_empty() {
        convert(branch, template);
        return;
    }
    if level == 2 {
        let ids = |n: &Node| n.children.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        if let Ok(alignment) = matching::align(&ids(base), &ids(branch)) {
            for j in alignment.additions {
                convert(&mut branch.children[j], template);
            }
        }
    } else {
        for child in &mut branch.children {
            if let Some(b) = base.children.iter().find(|b| b.id == child.id) {
                normalize_insertions(b, child, template, level + 1);
            } else {
                convert(child, template);
            }
        }
    }
    let mut raw = branch.head.clone();
    raw.push_str(&branch.prefix);
    for child in &branch.children {
        render::append(&mut raw, &child.raw);
    }
    render::append(&mut raw, &branch.suffix);
    branch.raw = raw;
}

fn heading_id(raw: &str, level: usize) -> String {
    let mut text = String::new();
    for event in pulldown_cmark::Parser::new(raw) {
        if let pulldown_cmark::Event::Text(s) | pulldown_cmark::Event::Code(s) = event {
            text.push_str(&s);
        }
    }
    if level == 1 {
        identity::release(&text).unwrap_or(text)
    } else {
        text.trim().to_lowercase()
    }
}

fn entry_description(n: &Node) -> String {
    let preview: String = n
        .raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    format!("{} {preview}", entry_key(n))
}

fn opaque_changes(
    base: &Node,
    branch: &Node,
) -> Vec<(EventKind, Option<Span>, Option<Span>, &'static str)> {
    let mut out = Vec::new();
    for (b, n, bs, ns, name) in [
        (
            &base.prefix,
            &branch.prefix,
            base.prefix_span,
            branch.prefix_span,
            "opaque prefix changed",
        ),
        (
            &base.suffix,
            &branch.suffix,
            base.suffix_span,
            branch.suffix_span,
            "opaque suffix/reference footer changed",
        ),
    ] {
        if b != n {
            out.push((
                if b.trim() == n.trim() {
                    EventKind::FormattingSelected
                } else {
                    EventKind::Modified
                },
                bs,
                ns,
                name,
            ));
        }
    }
    out
}
