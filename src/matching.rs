use std::collections::{HashMap, HashSet, VecDeque};
const MAX_CANDIDATE_PAIRS: usize = 65_536;
#[derive(Debug)]
pub(crate) struct Alignment {
    pub matched: Vec<Option<usize>>,
    /// Branch positions not matched to a base identity.
    pub additions: Vec<usize>,
    pub inferred: Vec<usize>,
}
/// Token overlap identifies candidates, never resolves competing entry edits.
fn tokens(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}
pub(crate) fn similarity(a: &str, b: &str) -> f64 {
    if a.len().saturating_add(b.len()) > 65_536 {
        return 0.0;
    }
    let (a, b) = (tokens(a), tokens(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.intersection(&b).count();
    shared as f64 / (a.len() + b.len() - shared) as f64
}
/// Duplicate occurrences have positional identities. Exact matches may reorder;
/// an LCS supplies stable anchors for conservative edit inference.
pub(crate) fn align(base: &[String], branch: &[String]) -> Result<Alignment, ()> {
    let mut positions: HashMap<&str, VecDeque<usize>> = HashMap::new();
    for (i, s) in base.iter().enumerate() {
        positions.entry(s).or_default().push_back(i);
    }
    let mut matched = vec![None; base.len()];
    let mut used = vec![false; branch.len()];
    let mut exact = Vec::new();
    for (j, s) in branch.iter().enumerate() {
        if let Some(i) = positions.get_mut(s.as_str()).and_then(VecDeque::pop_front) {
            matched[i] = Some(j);
            used[j] = true;
            exact.push((i, j));
        }
    }
    let anchors = lcs(&exact);
    let mut inferred = Vec::new();
    let (mut bi, mut si) = (0, 0);
    for (b, s) in anchors
        .into_iter()
        .chain(std::iter::once((base.len(), branch.len())))
    {
        let bs: Vec<_> = (bi..b).filter(|i| matched[*i].is_none()).collect();
        let ss: Vec<_> = (si..s).filter(|i| !used[*i]).collect();
        if !bs.is_empty() && !ss.is_empty() {
            if bs.len() == 1 && ss.len() == 1 {
                matched[bs[0]] = Some(ss[0]);
                used[ss[0]] = true;
                inferred.push(bs[0]);
            } else {
                let work = bs
                    .iter()
                    .map(|&i| base[i].len())
                    .sum::<usize>()
                    .saturating_mul(ss.len())
                    .saturating_add(
                        ss.iter()
                            .map(|&j| branch[j].len())
                            .sum::<usize>()
                            .saturating_mul(bs.len()),
                    );
                if bs.len().saturating_mul(ss.len()) > MAX_CANDIDATE_PAIRS || work > 8 * 1024 * 1024
                {
                    return Err(());
                }
                let mut candidates = Vec::new();
                for &i in &bs {
                    let mut choices: Vec<_> = ss
                        .iter()
                        .map(|&j| (j, similarity(&base[i], &branch[j])))
                        .filter(|(_, score)| *score >= 0.5)
                        .collect();
                    choices.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                    if let Some(&(j, score)) = choices.first() {
                        if choices
                            .get(1)
                            .is_some_and(|(_, second)| score - second < 0.15)
                        {
                            return Err(());
                        }
                        candidates.push((i, j));
                    }
                }
                let mut targets = HashSet::new();
                for (i, j) in candidates {
                    if !targets.insert(j) {
                        return Err(());
                    }
                    matched[i] = Some(j);
                    used[j] = true;
                    inferred.push(i);
                }
                if bs.iter().any(|&i| matched[i].is_none()) && ss.iter().any(|&j| !used[j]) {
                    return Err(());
                }
            }
        }
        bi = b + 1;
        si = s + 1;
    }
    Ok(Alignment {
        matched,
        additions: (0..branch.len()).filter(|j| !used[*j]).collect(),
        inferred,
    })
}
/// LCS for paired occurrence IDs, implemented as LIS in O(n log n).
fn lcs(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; pairs.len()];
    for (i, &(base, _)) in pairs.iter().enumerate() {
        let k = tails.partition_point(|p| pairs[*p].0 < base);
        if k > 0 {
            previous[i] = Some(tails[k - 1]);
        }
        if k == tails.len() {
            tails.push(i);
        } else {
            tails[k] = i;
        }
    }
    let mut out = Vec::new();
    let mut current = tails.last().copied();
    while let Some(i) = current {
        out.push(pairs[i]);
        current = previous[i];
    }
    out.reverse();
    out
}
