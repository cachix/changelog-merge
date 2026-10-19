//! Conservative line merging with reference definitions as atomic keyed units.
use pulldown_cmark::Parser;
use similar::{Algorithm, DiffTag, capture_diff_slices};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Token {
    Text(String),
    Reference(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Hunk {
    start: usize,
    end: usize,
    replacement: Vec<Token>,
}
type References = BTreeMap<String, String>;
pub(crate) fn definitions(source: &str) -> Vec<(String, std::ops::Range<usize>)> {
    if !source.contains("]:") {
        return Vec::new();
    }
    let parser = Parser::new(source);
    let mut defs: Vec<_> = parser
        .reference_definitions()
        .iter()
        .map(|(key, d)| {
            let mut end = d.span.end;
            if source[end..].starts_with("\r\n") {
                end += 2;
            } else if source[end..].starts_with('\n') {
                end += 1;
            }
            (key.to_lowercase(), d.span.start..end)
        })
        .collect();
    defs.sort_by_key(|(_, r)| r.start);
    defs
}
fn tokens(source: &str) -> (Vec<Token>, References) {
    let mut out = Vec::new();
    let mut refs = BTreeMap::new();
    let mut pos = 0;
    for (key, range) in definitions(source) {
        out.extend(
            source[pos..range.start]
                .split_inclusive('\n')
                .map(|s| Token::Text(s.into())),
        );
        out.push(Token::Reference(key.clone()));
        refs.insert(key, source[range.clone()].into());
        pos = range.end;
    }
    out.extend(
        source[pos..]
            .split_inclusive('\n')
            .map(|s| Token::Text(s.into())),
    );
    (out, refs)
}
fn hunks(base: &[Token], branch: &[Token]) -> Vec<Hunk> {
    capture_diff_slices(Algorithm::Patience, base, branch)
        .into_iter()
        .filter(|op| op.tag() != DiffTag::Equal)
        .map(|op| {
            let old = op.old_range();
            Hunk {
                start: old.start,
                end: old.end,
                replacement: branch[op.new_range()].to_vec(),
            }
        })
        .collect()
}
fn added_definitions(a: &[Token], b: &[Token]) -> Option<Vec<Token>> {
    let mut keys = BTreeSet::new();
    let mut leading = Vec::new();
    let mut trailing = Vec::new();
    for sequence in [a, b] {
        let mut found = false;
        let mut prefix = String::new();
        let mut suffix = String::new();
        for token in sequence {
            match token {
                Token::Text(s) if s.trim().is_empty() => {
                    if found {
                        suffix.push_str(s);
                    } else {
                        prefix.push_str(s);
                    }
                }
                Token::Reference(key) => {
                    found = true;
                    keys.insert(key.clone());
                    suffix.clear();
                }
                _ => return None,
            }
        }
        if !found {
            return None;
        }
        leading.push(prefix);
        trailing.push(suffix);
    }
    let mut out = vec![Token::Text(leading.into_iter().min().unwrap_or_default())];
    out.extend(keys.into_iter().map(Token::Reference));
    out.push(Token::Text(trailing.into_iter().min().unwrap_or_default()));
    Some(out)
}
pub(crate) fn merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    if ours == theirs || base == theirs {
        return Some(ours.into());
    }
    if base == ours {
        return Some(theirs.into());
    }
    let ((b, br), (o, or), (t, tr)) = (tokens(base), tokens(ours), tokens(theirs));
    if b.len() + o.len() + t.len() > 65_536 {
        return None;
    }
    let mut references = BTreeMap::new();
    for key in br
        .keys()
        .chain(or.keys())
        .chain(tr.keys())
        .collect::<BTreeSet<_>>()
    {
        let (b, o, t) = (br.get(key), or.get(key), tr.get(key));
        let value = if o == t || t == b {
            o
        } else if o == b {
            t
        } else {
            return None;
        };
        if let Some(value) = value {
            references.insert(key.clone(), value.clone());
        }
    }
    let mut changes = hunks(&b, &o);
    changes.extend(hunks(&b, &t));
    changes.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(a.end.cmp(&b.end))
            .then(a.replacement.cmp(&b.replacement))
    });
    let mut selected = Vec::new();
    let mut pos = 0;
    let mut i = 0;
    while i < changes.len() {
        let h = &changes[i];
        if h.start < pos {
            return None;
        }
        selected.extend_from_slice(&b[pos..h.start]);
        let mut next = i + 1;
        let mut value = h.replacement.clone();
        while next < changes.len()
            && (changes[next].start < h.end || changes[next].start == h.start)
        {
            let other = &changes[next];
            if h == other {
                next += 1;
                continue;
            }
            if h.start == h.end && other.start == other.end && h.start == other.start {
                value = added_definitions(&value, &other.replacement)?;
            } else {
                return None;
            }
            next += 1;
        }
        selected.extend(value);
        pos = h.end;
        i = next;
    }
    selected.extend_from_slice(&b[pos..]);
    let mut out = String::new();
    let mut seen = BTreeSet::new();
    let mut last_reference = false;
    for token in selected {
        match token {
            Token::Text(s) => {
                out.push_str(&s);
                if !s.is_empty() {
                    last_reference = false;
                }
            }
            Token::Reference(key) => {
                if !seen.insert(key.clone()) {
                    return None;
                }
                let raw = references.get(&key)?;
                if last_reference {
                    crate::render::append(&mut out, raw);
                } else {
                    out.push_str(raw);
                }
                last_reference = true;
            }
        }
    }
    if seen.len() != references.len() {
        return None;
    }
    Some(out)
}
