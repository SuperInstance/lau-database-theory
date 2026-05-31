//! Normal forms (1NF through BCNF), functional dependencies, and decomposition.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

/// A functional dependency X → Y.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FD {
    pub lhs: HashSet<String>, // Determinant
    pub rhs: HashSet<String>, // Dependent attributes
}

impl FD {
    pub fn new(lhs: Vec<&str>, rhs: Vec<&str>) -> Self {
        FD {
            lhs: lhs.into_iter().map(|s| s.to_string()).collect(),
            rhs: rhs.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Check if this FD is trivial (rhs ⊆ lhs).
    pub fn is_trivial(&self) -> bool {
        self.rhs.is_subset(&self.lhs)
    }
}

impl fmt::Display for FD {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lhs: Vec<&str> = self.lhs.iter().map(|s| s.as_str()).collect();
        lhs.sort();
        let mut rhs: Vec<&str> = self.rhs.iter().map(|s| s.as_str()).collect();
        rhs.sort();
        write!(f, "{{{}}} → {{{}}}", lhs.join(", "), rhs.join(", "))
    }
}

/// Compute the closure of a set of attributes under a set of FDs.
pub fn closure(attrs: &HashSet<String>, fds: &[FD]) -> HashSet<String> {
    let mut result = attrs.clone();

    loop {
        let mut changed = false;
        for fd in fds {
            if fd.lhs.is_subset(&result) {
                let new_attrs: HashSet<String> = fd.rhs.difference(&result).cloned().collect();
                if !new_attrs.is_empty() {
                    result.extend(new_attrs);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    result
}

/// Use BTreeSet for HashMap keys so they're hashable.
type AttrSet = BTreeSet<String>;

fn hashset_to_btreeset(s: &HashSet<String>) -> AttrSet {
    s.iter().cloned().collect()
}

fn btreeset_to_hashset(s: &AttrSet) -> HashSet<String> {
    s.iter().cloned().collect()
}

/// Compute all attribute closures (for key finding).
fn attribute_closures_btree(fds: &[FD], all_attrs: &HashSet<String>) -> HashMap<AttrSet, HashSet<String>> {
    let mut result = HashMap::new();
    let attrs: Vec<&String> = all_attrs.iter().collect();
    let n = attrs.len();
    for mask in 1u32..=(1 << n) - 1 {
        let subset: HashSet<String> = (0..n)
            .filter(|&i| mask & (1 << i) != 0)
            .map(|i| attrs[i].clone())
            .collect();
        let cl = closure(&subset, fds);
        result.insert(hashset_to_btreeset(&subset), cl);
    }
    result
}

/// Find all candidate keys for a relation given its FDs.
pub fn candidate_keys(fds: &[FD], all_attrs: &HashSet<String>) -> Vec<HashSet<String>> {
    let closures = attribute_closures_btree(fds, all_attrs);
    let mut keys: Vec<HashSet<String>> = Vec::new();

    for (subset, cl) in &closures {
        let subset_hs = btreeset_to_hashset(subset);
        if cl == all_attrs {
            // Check minimality: no proper subset is also a key
            let is_minimal = keys.iter().all(|k| !k.is_subset(&subset_hs));
            if is_minimal {
                // Remove any existing keys that this one subsumes
                keys.retain(|k| !subset_hs.is_subset(k));
                keys.push(subset_hs);
            }
        }
    }

    keys
}

/// Check if a relation is in 1NF.
pub fn is_1nf(schema: &[String]) -> bool {
    !schema.is_empty() && schema.iter().all(|s| !s.is_empty())
}

/// Check if a relation is in 2NF.
pub fn is_2nf(fds: &[FD], all_attrs: &HashSet<String>) -> bool {
    if !is_1nf(&all_attrs.iter().cloned().collect::<Vec<_>>()) {
        return false;
    }

    let keys = candidate_keys(fds, all_attrs);
    let prime_attrs: HashSet<String> = keys.iter().flat_map(|k| k.iter().cloned()).collect();

    for fd in fds {
        if fd.is_trivial() {
            continue;
        }
        for key in &keys {
            if fd.lhs.is_subset(key) && fd.lhs != *key {
                let non_prime: HashSet<String> = fd.rhs.difference(&prime_attrs).cloned().collect();
                if !non_prime.is_empty() {
                    return false;
                }
            }
        }
    }

    true
}

/// Check if a relation is in 3NF.
pub fn is_3nf(fds: &[FD], all_attrs: &HashSet<String>) -> bool {
    let keys = candidate_keys(fds, all_attrs);
    let superkeys: Vec<HashSet<String>> = {
        let closures = attribute_closures_btree(fds, all_attrs);
        closures
            .into_iter()
            .filter(|(_, cl)| cl == all_attrs)
            .map(|(subset, _)| btreeset_to_hashset(&subset))
            .collect()
    };
    let prime_attrs: HashSet<String> = keys.iter().flat_map(|k| k.iter().cloned()).collect();

    for fd in fds {
        if fd.is_trivial() {
            continue;
        }
        let x_is_superkey = superkeys.iter().any(|sk| *sk == fd.lhs);
        let y_is_prime = fd.rhs.is_subset(&prime_attrs);

        if !x_is_superkey && !y_is_prime {
            return false;
        }
    }

    true
}

/// Check if a relation is in BCNF.
pub fn is_bcnf(fds: &[FD], all_attrs: &HashSet<String>) -> bool {
    let superkeys: Vec<HashSet<String>> = {
        let closures = attribute_closures_btree(fds, all_attrs);
        closures
            .into_iter()
            .filter(|(_, cl)| cl == all_attrs)
            .map(|(subset, _)| btreeset_to_hashset(&subset))
            .collect()
    };

    for fd in fds {
        if fd.is_trivial() {
            continue;
        }
        if !superkeys.iter().any(|sk| *sk == fd.lhs) {
            return false;
        }
    }

    true
}

/// BCNF decomposition.
pub fn decompose_bcnf(
    all_attrs: &HashSet<String>,
    fds: &[FD],
) -> Vec<(HashSet<String>, Vec<FD>)> {
    let mut result = Vec::new();
    let mut queue = vec![(all_attrs.clone(), fds.to_vec())];

    while let Some((attrs, rel_fds)) = queue.pop() {
        let superkeys: Vec<HashSet<String>> = {
            let closures = attribute_closures_btree(&rel_fds, &attrs);
            closures
                .into_iter()
                .filter(|(_, cl)| cl == &attrs)
                .map(|(subset, _)| btreeset_to_hashset(&subset))
                .collect()
        };

        let violator = rel_fds.iter().find(|fd| {
            !fd.is_trivial() && !superkeys.iter().any(|sk| *sk == fd.lhs)
        });

        match violator {
            None => {
                result.push((attrs, rel_fds));
            }
            Some(fd) => {
                let r1: HashSet<String> = fd.lhs.union(&fd.rhs).cloned().collect();
                let r2: HashSet<String> = attrs.difference(&fd.rhs).cloned().collect();

                let r1_fds: Vec<FD> = rel_fds
                    .iter()
                    .filter(|f| f.lhs.is_subset(&r1) && f.rhs.is_subset(&r1))
                    .cloned()
                    .collect();
                let r2_fds: Vec<FD> = rel_fds
                    .iter()
                    .filter(|f| f.lhs.is_subset(&r2) && f.rhs.is_subset(&r2))
                    .cloned()
                    .collect();

                if !r1.is_empty() && !r2.is_empty() {
                    queue.push((r1, r1_fds));
                    queue.push((r2, r2_fds));
                } else {
                    result.push((attrs, rel_fds));
                }
            }
        }
    }

    result
}

/// 3NF synthesis.
pub fn decompose_3nf(
    all_attrs: &HashSet<String>,
    fds: &[FD],
) -> Vec<HashSet<String>> {
    let minimal_cover = compute_minimal_cover(fds);
    let mut result = Vec::new();

    let mut groups: HashMap<Vec<String>, HashSet<String>> = HashMap::new();
    for fd in &minimal_cover {
        let mut lhs: Vec<String> = fd.lhs.iter().cloned().collect();
        lhs.sort();
        groups
            .entry(lhs)
            .or_default()
            .extend(fd.rhs.iter().cloned());
    }

    for (lhs, rhs) in &groups {
        let mut attrs: HashSet<String> = HashSet::new();
        for l in lhs {
            attrs.insert(l.clone());
        }
        attrs.extend(rhs.iter().cloned());
        result.push(attrs);
    }

    let keys = candidate_keys(fds, all_attrs);
    let has_key = result.iter().any(|r| keys.iter().any(|k| k.is_subset(r)));

    if !has_key {
        if let Some(key) = keys.first() {
            result.push(key.clone());
        }
    }

    result
}

/// Compute a minimal cover of a set of FDs.
pub fn compute_minimal_cover(fds: &[FD]) -> Vec<FD> {
    // Step 1: Split RHS
    let mut result: Vec<FD> = Vec::new();
    for fd in fds {
        for attr in &fd.rhs {
            result.push(FD {
                lhs: fd.lhs.clone(),
                rhs: vec![attr.clone()].into_iter().collect(),
            });
        }
    }

    // Step 2: Remove extraneous attributes from LHS
    let mut changed = true;
    while changed {
        changed = false;
        let mut new_result = Vec::new();
        for fd in &result {
            if fd.lhs.len() > 1 {
                let mut found_reduction = None;
                for attr in fd.lhs.clone() {
                    let mut reduced_lhs = fd.lhs.clone();
                    reduced_lhs.remove(&attr);
                    let cl = closure(&reduced_lhs, &result);
                    let rhs_attr = fd.rhs.iter().next().unwrap().clone();
                    if cl.contains(&rhs_attr) {
                        found_reduction = Some(FD {
                            lhs: reduced_lhs,
                            rhs: fd.rhs.clone(),
                        });
                        break;
                    }
                }
                if let Some(reduced) = found_reduction {
                    new_result.push(reduced);
                    changed = true;
                } else {
                    new_result.push(fd.clone());
                }
            } else {
                new_result.push(fd.clone());
            }
        }
        result = new_result;
    }

    // Step 3: Remove redundant FDs
    let mut i = 0;
    while i < result.len() {
        let fd = result.remove(i);
        let rhs_attr = fd.rhs.iter().next().unwrap().clone();
        let cl = closure(&fd.lhs, &result);
        if cl.contains(&rhs_attr) {
            // Redundant
        } else {
            result.insert(i, fd);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_set(attrs: &[&str]) -> HashSet<String> {
        attrs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_closure_basic() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let cl = closure(&make_set(&["A"]), &fds);
        assert_eq!(cl, make_set(&["A", "B", "C"]));
    }

    #[test]
    fn test_closure_chain() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
            FD::new(vec!["C"], vec!["D"]),
        ];
        let cl = closure(&make_set(&["A"]), &fds);
        assert_eq!(cl, make_set(&["A", "B", "C", "D"]));
    }

    #[test]
    fn test_closure_no_change() {
        let fds = vec![FD::new(vec!["A"], vec!["B"])];
        let cl = closure(&make_set(&["C"]), &fds);
        assert_eq!(cl, make_set(&["C"]));
    }

    #[test]
    fn test_fd_trivial() {
        let fd = FD::new(vec!["A", "B"], vec!["A"]);
        assert!(fd.is_trivial());
        let fd2 = FD::new(vec!["A"], vec!["B"]);
        assert!(!fd2.is_trivial());
    }

    #[test]
    fn test_candidate_keys() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        let keys = candidate_keys(&fds, &all);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], make_set(&["A"]));
    }

    #[test]
    fn test_candidate_keys_composite() {
        let fds = vec![FD::new(vec!["A", "B"], vec!["C"])];
        let all = make_set(&["A", "B", "C"]);
        let keys = candidate_keys(&fds, &all);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], make_set(&["A", "B"]));
    }

    #[test]
    fn test_is_1nf() {
        assert!(is_1nf(&["A".into(), "B".into()]));
        assert!(!is_1nf(&[]));
    }

    #[test]
    fn test_is_2nf_violation() {
        let fds = vec![
            FD::new(vec!["A", "B"], vec!["C"]),
            FD::new(vec!["A"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        assert!(!is_2nf(&fds, &all));
    }

    #[test]
    fn test_is_2nf_ok() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["A"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        assert!(is_2nf(&fds, &all));
    }

    #[test]
    fn test_is_3nf_transitive() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        assert!(!is_3nf(&fds, &all));
    }

    #[test]
    fn test_is_3nf_ok() {
        let fds = vec![FD::new(vec!["A"], vec!["B"])];
        let all = make_set(&["A", "B"]);
        assert!(is_3nf(&fds, &all));
    }

    #[test]
    fn test_is_bcnf_yes() {
        let fds = vec![FD::new(vec!["A"], vec!["B"])];
        let all = make_set(&["A", "B"]);
        assert!(is_bcnf(&fds, &all));
    }

    #[test]
    fn test_is_bcnf_no() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        assert!(!is_bcnf(&fds, &all));
    }

    #[test]
    fn test_decompose_bcnf() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        let result = decompose_bcnf(&all, &fds);
        assert!(result.len() >= 2);
        for (attrs, rel_fds) in &result {
            assert!(is_bcnf(rel_fds, attrs));
        }
    }

    #[test]
    fn test_decompose_3nf() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let all = make_set(&["A", "B", "C"]);
        let result = decompose_3nf(&all, &fds);
        assert!(!result.is_empty());
        let keys = candidate_keys(&fds, &all);
        assert!(result.iter().any(|r| keys.iter().any(|k| k.is_subset(r))));
    }

    #[test]
    fn test_minimal_cover() {
        let fds = vec![
            FD::new(vec!["A"], vec!["B"]),
            FD::new(vec!["A"], vec!["C"]),
            FD::new(vec!["B"], vec!["C"]),
        ];
        let mc = compute_minimal_cover(&fds);
        assert!(mc.len() <= 2);
    }
}
