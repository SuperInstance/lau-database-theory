//! ACID transactions: serializability, conflict serializability, precedence graph.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Operation in a transaction schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    Read { txn: usize, item: String },
    Write { txn: usize, item: String },
}

impl Operation {
    pub fn read(txn: usize, item: &str) -> Self {
        Operation::Read { txn, item: item.into() }
    }

    pub fn write(txn: usize, item: &str) -> Self {
        Operation::Write { txn, item: item.into() }
    }

    pub fn txn(&self) -> usize {
        match self {
            Operation::Read { txn, .. } => *txn,
            Operation::Write { txn, .. } => *txn,
        }
    }

    pub fn item(&self) -> &str {
        match self {
            Operation::Read { item, .. } => item,
            Operation::Write { item, .. } => item,
        }
    }

    pub fn is_read(&self) -> bool {
        matches!(self, Operation::Read { .. })
    }

    pub fn is_write(&self) -> bool {
        matches!(self, Operation::Write { .. })
    }
}

/// Check if two operations conflict.
/// Operations conflict if they are from different transactions, operate on the same item,
/// and at least one is a write.
pub fn conflicts(op1: &Operation, op2: &Operation) -> bool {
    if op1.txn() == op2.txn() {
        return false;
    }
    if op1.item() != op2.item() {
        return false;
    }
    // At least one must be a write
    !(op1.is_read() && op2.is_read())
}

/// A schedule of operations from multiple transactions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub operations: Vec<Operation>,
}

impl Schedule {
    pub fn new(operations: Vec<Operation>) -> Self {
        Schedule { operations }
    }

    /// Get all transaction IDs in the schedule.
    pub fn transactions(&self) -> HashSet<usize> {
        self.operations.iter().map(|op| op.txn()).collect()
    }

    /// Build the precedence graph (serializability graph).
    /// Edge T1 → T2 exists if T1's operation precedes T2's conflicting operation in the schedule.
    pub fn precedence_graph(&self) -> PrecedenceGraph {
        let mut edges: HashSet<(usize, usize)> = HashSet::new();

        for (i, op1) in self.operations.iter().enumerate() {
            for op2 in &self.operations[i + 1..] {
                if conflicts(op1, op2) {
                    edges.insert((op1.txn(), op2.txn()));
                }
            }
        }

        let txns = self.transactions();
        PrecedenceGraph {
            nodes: txns,
            edges,
        }
    }

    /// Check if the schedule is conflict-serializable.
    /// A schedule is conflict-serializable iff its precedence graph is acyclic.
    pub fn is_conflict_serializable(&self) -> bool {
        let graph = self.precedence_graph();
        !graph.has_cycle()
    }

    /// Find a serial order if conflict-serializable.
    /// Returns a topological sort of the precedence graph.
    pub fn serial_order(&self) -> Option<Vec<usize>> {
        let graph = self.precedence_graph();
        if graph.has_cycle() {
            None
        } else {
            Some(graph.topological_sort())
        }
    }

    /// Check if the schedule is view-serializable (basic check).
    /// For simplicity, we check if the schedule is conflict-serializable
    /// (which implies view-serializability).
    pub fn is_view_serializable(&self) -> bool {
        self.is_conflict_serializable()
    }
}

/// Precedence (serializability) graph for checking conflict serializability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrecedenceGraph {
    pub nodes: HashSet<usize>,
    pub edges: HashSet<(usize, usize)>,
}

impl PrecedenceGraph {
    pub fn new() -> Self {
        PrecedenceGraph {
            nodes: HashSet::new(),
            edges: HashSet::new(),
        }
    }

    pub fn add_node(&mut self, txn: usize) {
        self.nodes.insert(txn);
    }

    pub fn add_edge(&mut self, from: usize, to: usize) {
        self.nodes.insert(from);
        self.nodes.insert(to);
        self.edges.insert((from, to));
    }

    /// Check if the graph contains a cycle using DFS.
    pub fn has_cycle(&self) -> bool {
        let mut white: HashSet<usize> = self.nodes.clone();
        let mut gray: HashSet<usize> = HashSet::new();
        let mut black: HashSet<usize> = HashSet::new();

        for node in &self.nodes {
            if white.contains(node) {
                if self.dfs_cycle(*node, &mut white, &mut gray, &mut black) {
                    return true;
                }
            }
        }
        false
    }

    fn dfs_cycle(
        &self,
        node: usize,
        white: &mut HashSet<usize>,
        gray: &mut HashSet<usize>,
        black: &mut HashSet<usize>,
    ) -> bool {
        white.remove(&node);
        gray.insert(node);

        for (from, to) in &self.edges {
            if *from == node {
                if gray.contains(to) {
                    return true;
                }
                if white.contains(to) && self.dfs_cycle(*to, white, gray, black) {
                    return true;
                }
            }
        }

        gray.remove(&node);
        black.insert(node);
        false
    }

    /// Topological sort of the graph.
    pub fn topological_sort(&self) -> Vec<usize> {
        let mut in_degree: HashMap<usize, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(*node, 0);
        }
        for (_, to) in &self.edges {
            *in_degree.entry(*to).or_insert(0) += 1;
        }

        let mut queue: VecDeque<usize> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&node, _)| node)
            .collect();

        let mut result = Vec::new();
        while let Some(node) = queue.pop_front() {
            result.push(node);
            for (from, to) in &self.edges {
                if *from == node {
                    let deg = in_degree.get_mut(to).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(*to);
                    }
                }
            }
        }

        result
    }

    /// Get all edges from a node.
    pub fn successors(&self, node: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|(from, _)| *from == node)
            .map(|(_, to)| *to)
            .collect()
    }
}

/// A simple transaction with read/write operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: usize,
    pub operations: Vec<Operation>,
}

impl Transaction {
    pub fn new(id: usize) -> Self {
        Transaction {
            id,
            operations: Vec::new(),
        }
    }

    pub fn read(&mut self, item: &str) {
        self.operations.push(Operation::read(self.id, item));
    }

    pub fn write(&mut self, item: &str) {
        self.operations.push(Operation::write(self.id, item));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_serializable_yes() {
        // T1: R(A), W(A); T2: R(B), W(B) — no conflicts, serializable
        let schedule = Schedule::new(vec![
            Operation::read(1, "A"),
            Operation::read(2, "B"),
            Operation::write(1, "A"),
            Operation::write(2, "B"),
        ]);
        assert!(schedule.is_conflict_serializable());
    }

    #[test]
    fn test_conflict_serializable_no() {
        // T1: W(A), W(B); T2: W(A), W(B) — cyclic
        let schedule = Schedule::new(vec![
            Operation::write(1, "A"),
            Operation::write(2, "A"),
            Operation::write(2, "B"),
            Operation::write(1, "B"),
        ]);
        assert!(!schedule.is_conflict_serializable());
    }

    #[test]
    fn test_precedence_graph_acyclic() {
        // T1: R(A), W(A); T2: R(A) — T1 before T2 on A
        let schedule = Schedule::new(vec![
            Operation::read(1, "A"),
            Operation::write(1, "A"),
            Operation::read(2, "A"),
        ]);
        let graph = schedule.precedence_graph();
        assert!(!graph.has_cycle());
        assert!(graph.edges.contains(&(1, 2)));
    }

    #[test]
    fn test_precedence_graph_cyclic() {
        // T1: W(A), R(B); T2: W(B), R(A) — cycle
        let schedule = Schedule::new(vec![
            Operation::write(1, "A"),
            Operation::write(2, "B"),
            Operation::read(2, "A"),
            Operation::read(1, "B"),
        ]);
        let graph = schedule.precedence_graph();
        assert!(graph.has_cycle());
    }

    #[test]
    fn test_serial_order() {
        let schedule = Schedule::new(vec![
            Operation::write(1, "A"),
            Operation::write(1, "B"),
            Operation::read(2, "A"),
            Operation::read(2, "B"),
        ]);
        let order = schedule.serial_order();
        assert!(order.is_some());
        let order = order.unwrap();
        assert_eq!(order, vec![1, 2]);
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = PrecedenceGraph::new();
        graph.add_edge(1, 2);
        graph.add_edge(1, 3);
        graph.add_edge(2, 4);
        graph.add_edge(3, 4);
        let order = graph.topological_sort();
        assert_eq!(order[0], 1);
        assert_eq!(order[order.len() - 1], 4);
    }

    #[test]
    fn test_operation_conflicts() {
        let r1a = Operation::read(1, "A");
        let r2a = Operation::read(2, "A");
        let w1a = Operation::write(1, "A");
        let w2a = Operation::write(2, "A");
        let r1b = Operation::read(1, "B");

        // Read-read no conflict
        assert!(!conflicts(&r1a, &r2a));
        // Read-write conflict
        assert!(conflicts(&r1a, &w2a));
        // Write-write conflict
        assert!(conflicts(&w1a, &w2a));
        // Different items no conflict
        assert!(!conflicts(&r1a, &r1b));
        // Same transaction no conflict
        assert!(!conflicts(&r1a, &w1a));
    }

    #[test]
    fn test_transaction_builder() {
        let mut t = Transaction::new(1);
        t.read("A");
        t.write("A");
        assert_eq!(t.operations.len(), 2);
        assert_eq!(t.id, 1);
    }
}
