use std::collections::{BTreeMap, BTreeSet};
/// Stable topological ordering. Source order is a constraint, canonical bytes break ties.
pub(crate) fn reconcile(sequences: &[Vec<String>]) -> Option<Vec<String>> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut degree: BTreeMap<String, usize> = BTreeMap::new();
    for seq in sequences {
        for id in seq {
            degree.entry(id.clone()).or_default();
        }
        for pair in seq.windows(2) {
            if pair[0] != pair[1]
                && edges
                    .entry(pair[0].clone())
                    .or_default()
                    .insert(pair[1].clone())
            {
                *degree.entry(pair[1].clone()).or_default() += 1;
            }
        }
    }
    let mut ready: BTreeSet<_> = degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut out = Vec::new();
    while let Some(id) = ready.pop_first() {
        if let Some(next) = edges.get(&id) {
            for n in next {
                let d = degree.get_mut(n)?;
                *d -= 1;
                if *d == 0 {
                    ready.insert(n.clone());
                }
            }
        }
        out.push(id);
    }
    (out.len() == degree.len()).then_some(out)
}

/// Keep a one-sided reorder while dropping superseded base-order edges from the
/// other branch. Incompatible changed edges still form a visible cycle.
pub(crate) fn three_way(
    base: &[String],
    ours: &[String],
    theirs: &[String],
) -> Option<Vec<String>> {
    let index = |seq: &[String]| {
        seq.iter()
            .enumerate()
            .map(|(i, s)| (s.clone(), i))
            .collect::<BTreeMap<_, _>>()
    };
    let (bp, op, tp) = (index(base), index(ours), index(theirs));
    let mut constraints = Vec::new();
    for (sequence, other) in [(ours, &tp), (theirs, &op)] {
        for id in sequence {
            constraints.push(vec![id.clone()]);
        }
        for pair in sequence.windows(2) {
            if let (Some(bu), Some(bv), Some(ou), Some(ov)) = (
                bp.get(&pair[0]),
                bp.get(&pair[1]),
                other.get(&pair[0]),
                other.get(&pair[1]),
            ) && bu < bv
                && ou > ov
            {
                continue;
            }
            constraints.push(pair.to_vec());
        }
    }
    reconcile(&constraints)
}
