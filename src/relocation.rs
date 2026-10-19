//! Normalize unambiguous section renames and entry relocation before recursive merging.
use crate::{ConflictKind, DiagnosticEvent, EventKind, matching, merge::Node, render};
use std::collections::{BTreeMap, BTreeSet};
type Failure = (ConflictKind, &'static str);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Location {
    release: String,
    category: String,
    index: usize,
}
impl Location {
    fn section(&self) -> (&str, &str) {
        (&self.release, &self.category)
    }
}
fn category<'a>(doc: &'a Node, loc: &Location) -> Option<&'a Node> {
    doc.children
        .iter()
        .find(|r| r.id == loc.release)?
        .children
        .iter()
        .find(|c| c.id == loc.category)
}
fn leaf<'a>(doc: &'a Node, loc: &Location) -> Option<&'a Node> {
    category(doc, loc)?.children.get(loc.index)
}
fn locations(doc: &Node) -> BTreeMap<String, Vec<Location>> {
    let mut out: BTreeMap<String, Vec<Location>> = BTreeMap::new();
    for r in &doc.children {
        for c in &r.children {
            for (index, e) in c.children.iter().enumerate().filter(|(_, e)| !e.opaque) {
                out.entry(e.id.clone()).or_default().push(Location {
                    release: r.id.clone(),
                    category: c.id.clone(),
                    index,
                });
            }
        }
    }
    out
}
fn body(n: &Node) -> String {
    format!(
        "{}{}{}",
        n.prefix,
        n.children
            .iter()
            .map(|c| c.raw.as_str())
            .collect::<String>(),
        n.suffix
    )
    .replace("\r\n", "\n")
    .trim()
    .into()
}
struct Evidence {
    body: [u8; 32],
    meaningful: bool,
    children: BTreeSet<[u8; 32]>,
}
fn evidence(n: &Node) -> Evidence {
    let text = body(n);
    Evidence {
        body: *blake3::hash(text.as_bytes()).as_bytes(),
        meaningful: !text.is_empty(),
        children: n
            .children
            .iter()
            .map(|c| *blake3::hash(c.id.as_bytes()).as_bytes())
            .collect(),
    }
}
fn section_score(a: &Evidence, b: &Evidence) -> f64 {
    if a.meaningful && a.body == b.body {
        return 1.0;
    }
    let (aa, bb) = (&a.children, &b.children);
    if aa.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let (small, large) = if aa.len() < bb.len() {
        (aa, bb)
    } else {
        (bb, aa)
    };
    let shared = small.iter().filter(|id| large.contains(*id)).count();
    shared as f64 / (aa.len() + bb.len() - shared) as f64
}
fn rename_sections(base: &Node, branch: &mut Node, level: usize) -> Result<(), Failure> {
    let deleted: Vec<_> = base
        .children
        .iter()
        .filter(|b| !branch.children.iter().any(|n| n.id == b.id))
        .collect();
    let added: Vec<_> = branch
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| !base.children.iter().any(|b| b.id == n.id))
        .map(|(i, _)| i)
        .collect();
    if deleted.len().saturating_mul(added.len()) > 65_536 {
        return Err((
            ConflictKind::SectionRename,
            "section rename candidate budget exceeded",
        ));
    }
    let added_evidence: BTreeMap<_, _> = added
        .iter()
        .map(|&i| (i, evidence(&branch.children[i])))
        .collect();
    let mut claimed = BTreeSet::new();
    for old in deleted {
        let old_evidence = evidence(old);
        // Unreleased -> version is release membership, never a heading-only rename.
        if level == 0 && old.id == "unreleased" {
            continue;
        }
        let mut candidates: Vec<_> = added
            .iter()
            .map(|&i| (i, section_score(&old_evidence, &added_evidence[&i])))
            .filter(|(_, s)| *s >= 0.5)
            .collect();
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        if let Some(&(i, score)) = candidates.first() {
            if candidates
                .get(1)
                .is_some_and(|(_, next)| score - next < 0.2)
                || !claimed.insert(i)
            {
                return Err((ConflictKind::SectionRename, "ambiguous section rename"));
            }
            branch.children[i].id = old.id.clone();
        }
    }
    if level == 0 {
        for child in &mut branch.children {
            if let Some(b) = base.children.iter().find(|b| b.id == child.id) {
                rename_sections(b, child, 1)?;
            }
        }
    }
    Ok(())
}
fn identities(
    base: &Node,
    branch: &Node,
    unique: &BTreeMap<String, Vec<Location>>,
) -> Result<BTreeMap<String, Location>, Failure> {
    let exact = locations(branch);
    let mut out = BTreeMap::new();
    for r in &base.children {
        for c in &r.children {
            let origin = Location {
                release: r.id.clone(),
                category: c.id.clone(),
                index: 0,
            };
            if let Some(current) = category(branch, &origin) {
                let ids = |n: &Node| n.children.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
                if let Ok(alignment) = matching::align(&ids(c), &ids(current)) {
                    for (i, j) in alignment.matched.iter().enumerate() {
                        if let Some(j) = j
                            && !c.children[i].opaque
                            && unique[&c.children[i].id].len() == 1
                        {
                            out.insert(
                                c.children[i].id.clone(),
                                Location {
                                    index: *j,
                                    ..origin.clone()
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    for (key, locs) in unique {
        if locs.len() == 1
            && let Some(found) = exact.get(key)
            && found.len() == 1
        {
            out.insert(key.clone(), found[0].clone());
        }
    }
    let claimed: BTreeSet<_> = out.values().cloned().collect();
    let candidates: Vec<_> = exact
        .iter()
        .filter(|(key, _)| !unique.contains_key(*key))
        .flat_map(|(_, v)| v.iter())
        .filter(|loc| !claimed.contains(*loc))
        .collect();
    let missing: Vec<_> = unique
        .iter()
        .filter(|(key, locs)| locs.len() == 1 && !out.contains_key(*key))
        .collect();
    let missing_sections: BTreeSet<_> = missing.iter().map(|(_, loc)| loc[0].section()).collect();
    let candidate_sections: BTreeSet<_> = candidates.iter().map(|loc| loc.section()).collect();
    if missing_sections.len() == 1 && missing_sections == candidate_sections {
        return Ok(out);
    }
    if missing.len().saturating_mul(candidates.len()) > 65_536 {
        return Err((
            ConflictKind::ConflictingMove,
            "edited move candidate budget exceeded",
        ));
    }
    let mut proposals = Vec::new();
    for (key, origins) in missing {
        let mut choices: Vec<_> = candidates
            .iter()
            .filter(|loc| loc.section() != origins[0].section())
            .filter_map(|loc| {
                leaf(branch, loc).map(|e| ((*loc).clone(), matching::similarity(key, &e.id)))
            })
            .filter(|(_, score)| *score >= 0.6)
            .collect();
        choices.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        if let Some((loc, score)) = choices.first() {
            if choices.get(1).is_some_and(|(_, next)| score - next < 0.2) {
                return Err((ConflictKind::ConflictingMove, "ambiguous edited move"));
            }
            proposals.push((key.clone(), loc.clone()));
        }
    }
    let mut used = BTreeSet::new();
    for (key, loc) in proposals {
        if !used.insert(loc.clone()) {
            return Err((
                ConflictKind::ConflictingMove,
                "multiple entries match the same move destination",
            ));
        }
        out.insert(key, loc);
    }
    Ok(out)
}
#[derive(Clone)]
struct Plan {
    key: String,
    origin: Location,
    target: Location,
    ours: Location,
    theirs: Location,
    template: usize,
}
fn synthetic(n: &Node) -> Node {
    Node {
        children: Vec::new(),
        span: None,
        head_span: None,
        prefix_span: None,
        suffix_span: None,
        raw: String::new(),
        ..n.clone()
    }
}
fn destination<'a>(doc: &'a mut Node, loc: &Location, template: &Node) -> &'a mut Node {
    if !doc.children.iter().any(|r| r.id == loc.release) {
        let r = template
            .children
            .iter()
            .find(|r| r.id == loc.release)
            .expect("planned release exists");
        let before = template
            .children
            .iter()
            .skip_while(|n| n.id != r.id)
            .skip(1)
            .find_map(|n| doc.children.iter().position(|x| x.id == n.id))
            .unwrap_or(doc.children.len());
        doc.children.insert(before, synthetic(r));
    }
    let r = doc
        .children
        .iter_mut()
        .find(|r| r.id == loc.release)
        .expect("release inserted");
    if !r.children.iter().any(|c| c.id == loc.category) {
        let reference = category(template, loc).expect("planned category exists");
        let tr = template
            .children
            .iter()
            .find(|r| r.id == loc.release)
            .expect("template release exists");
        let before = tr
            .children
            .iter()
            .skip_while(|n| n.id != loc.category)
            .skip(1)
            .find_map(|n| r.children.iter().position(|x| x.id == n.id))
            .unwrap_or(r.children.len());
        r.children.insert(before, synthetic(reference));
    }
    r.children
        .iter_mut()
        .find(|c| c.id == loc.category)
        .expect("category inserted")
}
fn rebuild(doc: &mut Node, level: usize) {
    if level < 2 {
        for child in &mut doc.children {
            rebuild(child, level + 1);
        }
    }
    let mut raw = doc.head.clone();
    raw.push_str(&doc.prefix);
    for child in &doc.children {
        render::append(&mut raw, &child.raw);
    }
    render::append(&mut raw, &doc.suffix);
    doc.raw = raw;
}
fn relocate(
    doc: &mut Node,
    original: &Node,
    plans: &[Plan],
    templates: [&Node; 2],
    branch: Option<usize>,
    virtual_base: Option<&Node>,
) {
    let source = |p: &Plan| match branch {
        None => p.origin.clone(),
        Some(0) => p.ours.clone(),
        Some(_) => p.theirs.clone(),
    };
    let pending: Vec<_> = plans
        .iter()
        .filter(|p| source(p).section() != p.target.section())
        .collect();
    let removed: BTreeSet<_> = pending.iter().map(|p| source(p)).collect();
    for r in &mut doc.children {
        for c in &mut r.children {
            c.children = c
                .children
                .drain(..)
                .enumerate()
                .filter(|(index, _)| {
                    !removed.contains(&Location {
                        release: r.id.clone(),
                        category: c.id.clone(),
                        index: *index,
                    })
                })
                .map(|(_, e)| e)
                .collect();
        }
    }
    let mut groups: BTreeMap<(String, String), Vec<&Plan>> = BTreeMap::new();
    for p in pending {
        groups
            .entry((p.target.release.clone(), p.target.category.clone()))
            .or_default()
            .push(p);
    }
    for (_, group) in groups {
        let first = group[0];
        let template = templates[first.template];
        let existing_release = doc.children.iter().any(|r| r.id == first.target.release);
        let existing_category = category(doc, &first.target).is_some();
        let dest = destination(doc, &first.target, template);
        if branch.is_none() && !existing_category {
            dest.head.clear();
            dest.prefix.clear();
            dest.suffix.clear();
        }
        // Match repeated anchors by occurrence. Build all insertion positions once,
        // then splice in one pass instead of scanning the destination per entry.
        let mut slots: BTreeMap<usize, Vec<(&Plan, Node)>> = BTreeMap::new();
        for (template_index, template) in templates.iter().enumerate() {
            let items: Vec<_> = group
                .iter()
                .filter(|p| p.template == template_index)
                .collect();
            if items.is_empty() {
                continue;
            }
            let reference = virtual_base
                .and_then(|b| category(b, &first.target))
                .unwrap_or_else(|| {
                    category(template, &first.target).expect("validated destination")
                });
            let reference_positions: BTreeMap<_, _> = reference
                .children
                .iter()
                .enumerate()
                .map(|(i, e)| (e.id.as_str(), i))
                .collect();
            let mut positions: BTreeMap<&str, std::collections::VecDeque<usize>> = BTreeMap::new();
            for (i, e) in dest.children.iter().enumerate() {
                positions.entry(&e.id).or_default().push_back(i);
            }
            let mut anchors = vec![None; reference.children.len()];
            for (i, e) in reference.children.iter().enumerate() {
                anchors[i] = positions
                    .get_mut(e.id.as_str())
                    .and_then(std::collections::VecDeque::pop_front);
            }
            let mut next = dest.children.len();
            let mut before = vec![next; anchors.len()];
            for i in (0..anchors.len()).rev() {
                before[i] = next;
                if let Some(index) = anchors[i] {
                    next = index;
                }
            }
            for p in items {
                let item = leaf(original, &source(p))
                    .expect("validated source")
                    .clone();
                let reference_index = if virtual_base.is_some() {
                    *reference_positions
                        .get(p.key.as_str())
                        .expect("relocated base identity exists")
                } else {
                    p.target.index
                };
                slots
                    .entry(before[reference_index])
                    .or_default()
                    .push((p, item));
            }
        }
        for values in slots.values_mut() {
            if branch.is_none() && !existing_category {
                values.sort_by(|(a, _), (b, _)| a.origin.cmp(&b.origin));
            } else {
                values.sort_by(|(a, _), (b, _)| a.target.cmp(&b.target).then(a.key.cmp(&b.key)));
            }
        }
        let mut children = Vec::with_capacity(dest.children.len() + group.len());
        for (i, e) in dest.children.drain(..).enumerate() {
            if let Some(insertions) = slots.remove(&i) {
                children.extend(insertions.into_iter().map(|(_, e)| e));
            }
            children.push(e);
        }
        for (_, insertions) in slots {
            children.extend(insertions.into_iter().map(|(_, e)| e));
        }
        dest.children = children;
        if branch.is_none() && !existing_release {
            let r = doc
                .children
                .iter_mut()
                .find(|r| r.id == first.target.release)
                .expect("destination inserted");
            r.head.clear();
            r.prefix.clear();
            r.suffix.clear();
        }
    }
    // If the relocating branch removed the emptied source section, regard its
    // empty shell as removed from the virtual base too. New post-cut entries in
    // the other branch then form a new section, rather than a delete/edit pair.
    for p in plans {
        let template = templates[p.template];
        if category(template, &p.origin).is_none() {
            if let Some(r) = doc.children.iter_mut().find(|r| r.id == p.origin.release) {
                r.children.retain(|c| {
                    c.id != p.origin.category
                        || !c.children.is_empty()
                        || !c.prefix.trim().is_empty()
                        || !c.suffix.trim().is_empty()
                });
            }
            if !template.children.iter().any(|r| r.id == p.origin.release) {
                doc.children.retain(|r| {
                    r.id != p.origin.release
                        || !r.children.is_empty()
                        || !r.prefix.trim().is_empty()
                });
            }
        }
    }
    rebuild(doc, 0);
}
pub(crate) fn normalize(
    base: &mut Node,
    ours: &mut Node,
    theirs: &mut Node,
    events: &mut Vec<DiagnosticEvent>,
) -> Result<(), Failure> {
    rename_sections(base, ours, 0)?;
    rename_sections(base, theirs, 0)?;
    let known = locations(base);
    let ol = locations(ours);
    let tl = locations(theirs);
    let oi = identities(base, ours, &known)?;
    let ti = identities(base, theirs, &known)?;
    let mut plans = Vec::new();
    for (key, origins) in &known {
        if origins.len() != 1 {
            for branch in [&ol, &tl] {
                if branch.get(key).is_some_and(|v| {
                    v.iter()
                        .any(|l| !origins.iter().any(|b| b.section() == l.section()))
                }) {
                    return Err((
                        ConflictKind::ConflictingMove,
                        "duplicate base entries make relocation ambiguous",
                    ));
                }
            }
            continue;
        }
        let origin = &origins[0];
        let (o, t) = (oi.get(key), ti.get(key));
        let moved = |loc: Option<&Location>| loc.is_some_and(|l| l.section() != origin.section());
        if !moved(o) && !moved(t) {
            continue;
        }
        let cut = |loc: Option<&Location>| {
            origin.release == "unreleased" && loc.is_some_and(|l| l.release != "unreleased")
        };
        let kind = if cut(o) || cut(t) {
            ConflictKind::ReleaseCut
        } else {
            ConflictKind::ConflictingMove
        };
        if ol.get(key).is_some_and(|v| v.len() > 1) || tl.get(key).is_some_and(|v| v.len() > 1) {
            return Err((kind, "relocation competes with duplicate entry identities"));
        }
        let (Some(o), Some(t)) = (o, t) else {
            return Err((kind, "move versus deletion or ambiguous rewrite"));
        };
        if moved(Some(o)) && moved(Some(t)) && o.section() != t.section() {
            return Err((kind, "branches choose different move destinations"));
        }
        // Atomic move/edit conflicts remain visible, even when membership is clear.
        if leaf(ours, o).is_none_or(|e| e.id != *key) && moved(Some(t))
            || leaf(theirs, t).is_none_or(|e| e.id != *key) && moved(Some(o))
        {
            return Err((kind, "move versus concurrent modification"));
        }
        let (target, template) = if moved(Some(o)) {
            (o.clone(), 0)
        } else {
            (t.clone(), 1)
        };
        plans.push(Plan {
            key: key.clone(),
            origin: origin.clone(),
            target,
            ours: o.clone(),
            theirs: t.clone(),
            template,
        });
    }
    for (doc, identities) in [(&*ours, &oi), (&*theirs, &ti)] {
        let new_release = doc
            .children
            .iter()
            .any(|r| r.id != "unreleased" && !base.children.iter().any(|b| b.id == r.id));
        if new_release {
            for (key, locs) in &known {
                if locs.iter().any(|l| l.release == "unreleased") && identities.get(key).is_none() {
                    return Err((
                        ConflictKind::ReleaseCut,
                        "cannot determine released membership for a deleted or rewritten entry",
                    ));
                }
            }
        }
    }
    // Unknown replacements must not masquerade as independent new sections.
    // Exact relocation evidence can disambiguate removal of an emptied section.
    let mut section_totals = BTreeMap::new();
    for loc in known.values().flatten() {
        for key in [
            (loc.release.clone(), None),
            (loc.release.clone(), Some(loc.category.clone())),
        ] {
            *section_totals.entry(key).or_insert(0usize) += 1;
        }
    }
    for (index, branch) in [(&*ours), (&*theirs)].into_iter().enumerate() {
        let mut moved_totals = BTreeMap::new();
        for p in &plans {
            if (if index == 0 { &p.ours } else { &p.theirs }).section() != p.origin.section() {
                for key in [
                    (p.origin.release.clone(), None),
                    (p.origin.release.clone(), Some(p.origin.category.clone())),
                ] {
                    *moved_totals.entry(key).or_insert(0usize) += 1;
                }
            }
        }
        let moved_out = |r: &str, c: Option<&str>| {
            let key = (r.to_owned(), c.map(str::to_owned));
            section_totals
                .get(&key)
                .is_some_and(|count| *count > 0 && moved_totals.get(&key) == Some(count))
        };
        let added_release = branch
            .children
            .iter()
            .any(|r| !base.children.iter().any(|b| b.id == r.id));
        if added_release
            && base
                .children
                .iter()
                .any(|r| !branch.children.iter().any(|n| n.id == r.id) && !moved_out(&r.id, None))
        {
            return Err((ConflictKind::SectionRename, "ambiguous release replacement"));
        }
        for r in &base.children {
            if let Some(current) = branch.children.iter().find(|n| n.id == r.id) {
                let added_category = current
                    .children
                    .iter()
                    .any(|c| !r.children.iter().any(|b| b.id == c.id));
                if added_category
                    && r.children.iter().any(|c| {
                        !current.children.iter().any(|n| n.id == c.id)
                            && !moved_out(&r.id, Some(&c.id))
                    })
                {
                    return Err((
                        ConflictKind::SectionRename,
                        "ambiguous category replacement",
                    ));
                }
            }
        }
    }
    plans.sort_by(|a, b| a.target.cmp(&b.target).then(a.key.cmp(&b.key)));
    if !plans.is_empty() {
        let (b, o, t) = (base.clone(), ours.clone(), theirs.clone());
        for p in &plans {
            for (label, doc, loc) in [("ours", &o, &p.ours), ("theirs", &t, &p.theirs)] {
                if loc.section() != p.origin.section() {
                    events.push(DiagnosticEvent {
                        kind: EventKind::Moved,
                        branch: Some(label.into()),
                        release: Some(p.target.release.clone()),
                        category: Some(p.target.category.clone()),
                        base_span: leaf(&b, &p.origin).and_then(|e| e.span),
                        branch_span: leaf(doc, loc).and_then(|e| e.span),
                        message: format!(
                            "{}/{} -> {}/{}",
                            p.origin.release,
                            p.origin.category,
                            p.target.release,
                            p.target.category
                        ),
                        order: Vec::new(),
                    });
                }
            }
        }
        relocate(base, &b, &plans, [&o, &t], None, None);
        relocate(ours, &o, &plans, [&o, &t], Some(0), Some(base));
        relocate(theirs, &t, &plans, [&o, &t], Some(1), Some(base));
    }
    Ok(())
}
