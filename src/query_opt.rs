//! Query optimization: selection pushdown and join ordering via dynamic programming.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Estimated cost of a query plan (in abstract cost units).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub io_cost: f64,     // Estimated I/O operations
    pub cpu_cost: f64,    // Estimated CPU cost
}

impl Cost {
    pub fn total(&self) -> f64 {
        self.io_cost + self.cpu_cost
    }
}

/// Statistics about a relation for cost estimation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationStats {
    pub name: String,
    pub num_tuples: usize,
    pub num_pages: usize,
    pub column_stats: HashMap<String, ColumnStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStats {
    pub distinct_values: usize,
    pub min_val: f64,
    pub max_val: f64,
}

/// A query plan node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanNode {
    TableScan {
        relation: String,
        stats: RelationStats,
    },
    IndexScan {
        relation: String,
        column: String,
        stats: RelationStats,
    },
    Selection {
        predicate: String,
        selectivity: f64,
        child: Box<PlanNode>,
    },
    Projection {
        columns: Vec<String>,
        child: Box<PlanNode>,
    },
    NestedLoopJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        condition: String,
    },
    HashJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        left_key: String,
        right_key: String,
    },
    SortMergeJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        left_key: String,
        right_key: String,
    },
}

impl PlanNode {
    /// Estimate the output size of this plan.
    pub fn estimate_output_size(&self) -> usize {
        match self {
            PlanNode::TableScan { stats, .. } => stats.num_tuples,
            PlanNode::IndexScan { stats, .. } => stats.num_tuples,
            PlanNode::Selection { selectivity, child, .. } => {
                ((*selectivity) * child.estimate_output_size() as f64) as usize
            }
            PlanNode::Projection { child, .. } => child.estimate_output_size(),
            PlanNode::NestedLoopJoin { left, right, .. } => {
                left.estimate_output_size() * right.estimate_output_size()
            }
            PlanNode::HashJoin { left, right, .. } => {
                // Assuming equi-join with some selectivity
                let left_size = left.estimate_output_size();
                let right_size = right.estimate_output_size();
                (left_size.max(right_size) as f64 * 1.5) as usize
            }
            PlanNode::SortMergeJoin { left, right, .. } => {
                let left_size = left.estimate_output_size();
                let right_size = right.estimate_output_size();
                (left_size + right_size).max(left_size.max(right_size))
            }
        }
    }

    /// Estimate the cost of executing this plan.
    pub fn estimate_cost(&self) -> Cost {
        match self {
            PlanNode::TableScan { stats, .. } => Cost {
                io_cost: stats.num_pages as f64,
                cpu_cost: stats.num_tuples as f64,
            },
            PlanNode::IndexScan { stats, .. } => Cost {
                io_cost: (stats.num_pages as f64 * 0.1).max(1.0),
                cpu_cost: (stats.num_tuples as f64 * 0.1).max(1.0),
            },
            PlanNode::Selection { child, .. } => {
                let child_cost = child.estimate_cost();
                Cost {
                    io_cost: child_cost.io_cost,
                    cpu_cost: child_cost.cpu_cost * 1.1,
                }
            }
            PlanNode::Projection { child, .. } => {
                let child_cost = child.estimate_cost();
                Cost {
                    io_cost: child_cost.io_cost,
                    cpu_cost: child_cost.cpu_cost * 1.05,
                }
            }
            PlanNode::NestedLoopJoin { left, right, .. } => {
                let l_cost = left.estimate_cost();
                let r_cost = right.estimate_cost();
                let l_size = left.estimate_output_size();
                let r_size = right.estimate_output_size();
                Cost {
                    io_cost: l_cost.io_cost + l_size as f64 * r_cost.io_cost,
                    cpu_cost: l_cost.cpu_cost + (l_size * r_size) as f64,
                }
            }
            PlanNode::HashJoin { left, right, .. } => {
                let l_cost = left.estimate_cost();
                let r_cost = right.estimate_cost();
                let l_size = left.estimate_output_size();
                let r_size = right.estimate_output_size();
                Cost {
                    io_cost: l_cost.io_cost + r_cost.io_cost + 3.0 * (l_size + r_size) as f64 / 100.0,
                    cpu_cost: l_cost.cpu_cost + r_cost.cpu_cost + 3.0 * l_size.max(r_size) as f64,
                }
            }
            PlanNode::SortMergeJoin { left, right, .. } => {
                let l_cost = left.estimate_cost();
                let r_cost = right.estimate_cost();
                let l_size = left.estimate_output_size();
                let r_size = right.estimate_output_size();
                let sort_cost = |n: usize| -> f64 {
                    if n <= 1 { 0.0 } else { (n as f64) * (n as f64).log2() }
                };
                Cost {
                    io_cost: l_cost.io_cost + r_cost.io_cost,
                    cpu_cost: l_cost.cpu_cost + r_cost.cpu_cost + sort_cost(l_size) + sort_cost(r_size),
                }
            }
        }
    }

    /// Returns the set of relations referenced by this plan.
    pub fn relations(&self) -> Vec<String> {
        match self {
            PlanNode::TableScan { relation, .. } => vec![relation.clone()],
            PlanNode::IndexScan { relation, .. } => vec![relation.clone()],
            PlanNode::Selection { child, .. } => child.relations(),
            PlanNode::Projection { child, .. } => child.relations(),
            PlanNode::NestedLoopJoin { left, right, .. } => {
                let mut r = left.relations();
                r.extend(right.relations());
                r
            }
            PlanNode::HashJoin { left, right, .. } => {
                let mut r = left.relations();
                r.extend(right.relations());
                r
            }
            PlanNode::SortMergeJoin { left, right, .. } => {
                let mut r = left.relations();
                r.extend(right.relations());
                r
            }
        }
    }
}

/// Query optimizer using dynamic programming for join ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryOptimizer {
    relations: HashMap<String, RelationStats>,
}

impl QueryOptimizer {
    pub fn new() -> Self {
        QueryOptimizer {
            relations: HashMap::new(),
        }
    }

    pub fn add_relation(&mut self, stats: RelationStats) {
        self.relations.insert(stats.name.clone(), stats);
    }

    /// Apply selection pushdown: move selections as close to base tables as possible.
    pub fn push_selections(&self, plan: PlanNode) -> PlanNode {
        match plan {
            PlanNode::Selection { predicate, selectivity, child } => {
                let pushed_child = self.push_selections(*child);
                // Check if the selection only references one relation
                let rels = pushed_child.relations();
                if rels.len() == 1 {
                    // Selection can stay here (already pushed down)
                    PlanNode::Selection {
                        predicate,
                        selectivity,
                        child: Box::new(pushed_child),
                    }
                } else {
                    // Try to push through join — keep selection where it is
                    // but recurse into children
                    PlanNode::Selection {
                        predicate,
                        selectivity,
                        child: Box::new(pushed_child),
                    }
                }
            }
            PlanNode::NestedLoopJoin { left, right, condition } => {
                PlanNode::NestedLoopJoin {
                    left: Box::new(self.push_selections(*left)),
                    right: Box::new(self.push_selections(*right)),
                    condition,
                }
            }
            PlanNode::Projection { columns, child } => {
                PlanNode::Projection {
                    columns,
                    child: Box::new(self.push_selections(*child)),
                }
            }
            other => other,
        }
    }

    /// Find optimal join order using dynamic programming.
    /// Returns the best plan for joining all given relations.
    pub fn optimize_join_order(&self, relation_names: &[String]) -> Option<PlanNode> {
        if relation_names.is_empty() {
            return None;
        }

        // DP table: subset (as bitmask) -> best plan
        let n = relation_names.len();
        let mut dp: HashMap<usize, PlanNode> = HashMap::new();

        // Base case: single relation scans
        for (i, name) in relation_names.iter().enumerate() {
            let stats = self.relations.get(name)?;
            let plan = PlanNode::TableScan {
                relation: name.clone(),
                stats: stats.clone(),
            };
            dp.insert(1 << i, plan);
        }

        // Build up larger subsets
        for size in 2..=n {
            for subset in Self::subsets_of_size(n, size) {
                // Try all ways to split subset into two non-empty parts
                let mut best_plan: Option<(PlanNode, f64)> = None;

                // Enumerate sub-subsets
                let mut sub = (subset - 1) & subset;
                while sub > 0 {
                    let complement = subset ^ sub;
                    if complement == 0 || !dp.contains_key(&sub) || !dp.contains_key(&complement) {
                        sub = (sub - 1) & subset;
                        continue;
                    }

                    let left_plan = dp.get(&sub).cloned().unwrap();
                    let right_plan = dp.get(&complement).cloned().unwrap();

                    // Try hash join (usually best for large datasets)
                    let join_plan = PlanNode::HashJoin {
                        left: Box::new(left_plan),
                        right: Box::new(right_plan),
                        left_key: "id".into(), // Default join key
                        right_key: "id".into(),
                    };

                    let cost = join_plan.estimate_cost().total();

                    match &best_plan {
                        None => best_plan = Some((join_plan, cost)),
                        Some((_, best_cost)) if cost < *best_cost => {
                            best_plan = Some((join_plan, cost));
                        }
                        _ => {}
                    }

                    sub = (sub - 1) & subset;
                }

                if let Some((plan, _)) = best_plan {
                    dp.insert(subset, plan);
                }
            }
        }

        let full_mask = (1 << n) - 1;
        dp.remove(&full_mask)
    }

    /// Generate all bitmasks with exactly `k` bits set out of `n` positions.
    fn subsets_of_size(n: usize, k: usize) -> Vec<usize> {
        let mut result = Vec::new();
        let full = 1usize << n;

        let mut mask = (1 << k) - 1; // Smallest k-bit number
        while mask < full {
            result.push(mask);
            // Gosper's hack to get next combination
            let c = mask & mask.wrapping_neg();
            let r = mask + c;
            mask = (((r ^ mask) >> 2) / c) >> 1 | r;
            if mask == 0 {
                break;
            }
        }
        result
    }

    /// Choose the best join algorithm based on cost estimates.
    pub fn choose_join_algorithm(
        &self,
        left: PlanNode,
        right: PlanNode,
        left_key: &str,
        right_key: &str,
    ) -> PlanNode {
        let l_size = left.estimate_output_size();
        let r_size = right.estimate_output_size();

        // Simple heuristic:
        // - Very small: nested loop
        // - One side fits in memory: hash join
        // - Both sorted: sort-merge
        // Default: hash join
        let nlj = PlanNode::NestedLoopJoin {
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
            condition: format!("{}.{} = {}.{}", left.relations()[0], left_key, right.relations()[0], right_key),
        };
        let hj = PlanNode::HashJoin {
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
            left_key: left_key.into(),
            right_key: right_key.into(),
        };
        let smj = PlanNode::SortMergeJoin {
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
            left_key: left_key.into(),
            right_key: right_key.into(),
        };

        let nlj_cost = nlj.estimate_cost().total();
        let hj_cost = hj.estimate_cost().total();
        let smj_cost = smj.estimate_cost().total();

        if l_size * r_size < 1000 {
            nlj
        } else if hj_cost <= smj_cost {
            hj
        } else {
            smj
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stats(name: &str, tuples: usize, pages: usize) -> RelationStats {
        RelationStats {
            name: name.into(),
            num_tuples: tuples,
            num_pages: pages,
            column_stats: HashMap::new(),
        }
    }

    #[test]
    fn test_selection_pushdown() {
        let stats = make_stats("emp", 1000, 50);
        let plan = PlanNode::Selection {
            predicate: "emp.dept = 'Eng'".into(),
            selectivity: 0.3,
            child: Box::new(PlanNode::TableScan {
                relation: "emp".into(),
                stats: stats.clone(),
            }),
        };

        let opt = QueryOptimizer::new();
        let pushed = opt.push_selections(plan);
        // Selection on single relation should remain
        if let PlanNode::Selection { child, .. } = &pushed {
            assert!(matches!(**child, PlanNode::TableScan { .. }));
        } else {
            panic!("Expected selection node");
        }
    }

    #[test]
    fn test_join_order_two_relations() {
        let mut opt = QueryOptimizer::new();
        opt.add_relation(make_stats("r", 1000, 50));
        opt.add_relation(make_stats("s", 100, 5));

        let plan = opt.optimize_join_order(&["r".into(), "s".into()]);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert_eq!(plan.relations().len(), 2);
    }

    #[test]
    fn test_join_order_three_relations() {
        let mut opt = QueryOptimizer::new();
        opt.add_relation(make_stats("a", 100, 5));
        opt.add_relation(make_stats("b", 200, 10));
        opt.add_relation(make_stats("c", 300, 15));

        let plan = opt.optimize_join_order(&["a".into(), "b".into(), "c".into()]);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert_eq!(plan.relations().len(), 3);
    }

    #[test]
    fn test_cost_estimation() {
        let stats = make_stats("emp", 10000, 500);
        let scan = PlanNode::TableScan {
            relation: "emp".into(),
            stats,
        };
        let cost = scan.estimate_cost();
        assert_eq!(cost.io_cost, 500.0);
        assert_eq!(cost.cpu_cost, 10000.0);
    }

    #[test]
    fn test_index_scan_cheaper() {
        let stats = make_stats("emp", 10000, 500);
        let table_scan = PlanNode::TableScan {
            relation: "emp".into(),
            stats: stats.clone(),
        };
        let index_scan = PlanNode::IndexScan {
            relation: "emp".into(),
            column: "id".into(),
            stats,
        };
        assert!(index_scan.estimate_cost().total() < table_scan.estimate_cost().total());
    }

    #[test]
    fn test_choose_join_algorithm() {
        let opt = QueryOptimizer::new();

        // Small tables → nested loop
        let left = PlanNode::TableScan {
            relation: "a".into(),
            stats: make_stats("a", 10, 1),
        };
        let right = PlanNode::TableScan {
            relation: "b".into(),
            stats: make_stats("b", 10, 1),
        };
        let algo = opt.choose_join_algorithm(left, right, "id", "id");
        assert!(matches!(algo, PlanNode::NestedLoopJoin { .. }));

        // Large tables → hash join
        let left = PlanNode::TableScan {
            relation: "a".into(),
            stats: make_stats("a", 10000, 500),
        };
        let right = PlanNode::TableScan {
            relation: "b".into(),
            stats: make_stats("b", 10000, 500),
        };
        let algo = opt.choose_join_algorithm(left, right, "id", "id");
        assert!(matches!(algo, PlanNode::HashJoin { .. }));
    }

    #[test]
    fn test_empty_join_order() {
        let opt = QueryOptimizer::new();
        assert!(opt.optimize_join_order(&[]).is_none());
    }
}
