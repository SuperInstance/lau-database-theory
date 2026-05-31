//! B-tree implementation with insertion, search, and range queries.

use serde::{Deserialize, Serialize};

const DEFAULT_ORDER: usize = 4; // Max children per internal node (max keys = order-1)

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BTreeNode {
    Internal {
        keys: Vec<i64>,
        children: Vec<Box<BTreeNode>>,
    },
    Leaf {
        keys: Vec<i64>,
        values: Vec<Vec<u8>>,
    },
}

impl BTreeNode {
    pub fn is_leaf(&self) -> bool {
        matches!(self, BTreeNode::Leaf { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BTree {
    pub root: Option<Box<BTreeNode>>,
    pub order: usize, // Maximum number of children
}

impl BTree {
    pub fn new() -> Self {
        BTree {
            root: None,
            order: DEFAULT_ORDER,
        }
    }

    pub fn with_order(order: usize) -> Self {
        assert!(order >= 3, "B-tree order must be at least 3");
        BTree {
            root: None,
            order,
        }
    }

    /// Search for a key. Returns the associated value if found.
    pub fn search(&self, key: i64) -> Option<&Vec<u8>> {
        self.root.as_ref().and_then(|node| Self::search_node(node, key))
    }

    fn search_node(node: &BTreeNode, key: i64) -> Option<&Vec<u8>> {
        match node {
            BTreeNode::Leaf { keys, values } => {
                match keys.binary_search(&key) {
                    Ok(i) => Some(&values[i]),
                    Err(_) => None,
                }
            }
            BTreeNode::Internal { keys, children } => {
                let child_idx = keys.iter().position(|k| key < *k).unwrap_or(keys.len());
                Self::search_node(&children[child_idx], key)
            }
        }
    }

    /// Check if a key exists.
    pub fn contains(&self, key: i64) -> bool {
        self.search(key).is_some()
    }

    /// Insert a key-value pair.
    pub fn insert(&mut self, key: i64, value: Vec<u8>) {
        if self.root.is_none() {
            self.root = Some(Box::new(BTreeNode::Leaf {
                keys: vec![key],
                values: vec![value],
            }));
            return;
        }

        let root = self.root.take().unwrap();
        let max_keys = self.order - 1;

        match Self::insert_node(root, key, value, max_keys) {
            InsertResult::Done(node) => {
                self.root = Some(node);
            }
            InsertResult::Split(left, mid_key, mid_val, right) => {
                self.root = Some(Box::new(BTreeNode::Internal {
                    keys: vec![mid_key],
                    children: vec![left, right],
                }));
                let _ = mid_val;
            }
        }
    }

    /// Range query: returns all key-value pairs in [start, end].
    pub fn range_query(&self, start: i64, end: i64) -> Vec<(i64, Vec<u8>)> {
        let mut results = Vec::new();
        if let Some(root) = &self.root {
            Self::range_node(root, start, end, &mut results);
        }
        results
    }

    fn range_node(node: &BTreeNode, start: i64, end: i64, results: &mut Vec<(i64, Vec<u8>)>) {
        match node {
            BTreeNode::Leaf { keys, values } => {
                for (i, k) in keys.iter().enumerate() {
                    if *k >= start && *k <= end {
                        results.push((*k, values[i].clone()));
                    }
                }
            }
            BTreeNode::Internal { keys, children } => {
                // children has len = keys.len() + 1
                // child[i] covers keys < keys[i], child[last] covers keys >= keys[last-1]
                for (i, child) in children.iter().enumerate() {
                    let lower_bound = if i == 0 { i64::MIN } else { keys[i - 1] };
                    let upper_bound = if i >= keys.len() { i64::MAX } else { keys[i] };

                    // Only recurse if subtree could overlap [start, end]
                    // child[i] has keys in [lower_bound, upper_bound)
                    if lower_bound <= end && (upper_bound == i64::MAX || upper_bound > start) {
                        Self::range_node(child, start, end, results);
                    }
                }
            }
        }
    }

    /// Returns all keys in sorted order.
    pub fn keys(&self) -> Vec<i64> {
        let mut result = Vec::new();
        if let Some(root) = &self.root {
            Self::collect_keys(root, &mut result);
        }
        result
    }

    fn collect_keys(node: &BTreeNode, keys: &mut Vec<i64>) {
        match node {
            BTreeNode::Leaf { keys: node_keys, .. } => {
                keys.extend(node_keys.iter().copied());
            }
            BTreeNode::Internal { keys: _, children } => {
                for child in children {
                    Self::collect_keys(child, keys);
                }
            }
        }
    }

    fn insert_node(
        node: Box<BTreeNode>,
        key: i64,
        value: Vec<u8>,
        max_keys: usize,
    ) -> InsertResult {
        match *node {
            BTreeNode::Leaf { mut keys, mut values } => {
                match keys.binary_search(&key) {
                    Ok(i) => {
                        values[i] = value;
                        InsertResult::Done(Box::new(BTreeNode::Leaf { keys, values }))
                    }
                    Err(i) => {
                        keys.insert(i, key);
                        values.insert(i, value);

                        if keys.len() > max_keys {
                            // Split leaf
                            let mid = keys.len() / 2;
                            let left_keys = keys[..mid].to_vec();
                            let left_vals = values[..mid].to_vec();
                            let right_keys = keys[mid..].to_vec();
                            let right_vals = values[mid..].to_vec();
                            let mid_key = right_keys[0];

                            InsertResult::Split(
                                Box::new(BTreeNode::Leaf { keys: left_keys, values: left_vals }),
                                mid_key,
                                right_vals[0].clone(),
                                Box::new(BTreeNode::Leaf { keys: right_keys, values: right_vals }),
                            )
                        } else {
                            InsertResult::Done(Box::new(BTreeNode::Leaf { keys, values }))
                        }
                    }
                }
            }
            BTreeNode::Internal { mut keys, mut children } => {
                let child_idx = keys.iter().position(|k| key < *k).unwrap_or(keys.len());
                let child = children.remove(child_idx);

                match Self::insert_node(child, key, value, max_keys) {
                    InsertResult::Done(new_child) => {
                        children.insert(child_idx, new_child);
                        InsertResult::Done(Box::new(BTreeNode::Internal { keys, children }))
                    }
                    InsertResult::Split(left, mid_key, _mid_val, right) => {
                        // Insert the split result
                        keys.insert(child_idx, mid_key);
                        children.insert(child_idx, left);
                        // The old child at child_idx was removed, now put right at child_idx+1
                        // But we need to be careful: after remove(child_idx), we inserted left at child_idx
                        // Now insert right at child_idx+1
                        children.insert(child_idx + 1, right);

                        if keys.len() > max_keys {
                            // Split internal node
                            let mid = keys.len() / 2;
                            let up_key = keys[mid];
                            let left_keys = keys[..mid].to_vec();
                            let left_children = children[..=mid].to_vec();
                            let right_keys = keys[mid + 1..].to_vec();
                            let right_children = children[mid + 1..].to_vec();

                            InsertResult::Split(
                                Box::new(BTreeNode::Internal { keys: left_keys, children: left_children }),
                                up_key,
                                Vec::new(),
                                Box::new(BTreeNode::Internal { keys: right_keys, children: right_children }),
                            )
                        } else {
                            InsertResult::Done(Box::new(BTreeNode::Internal { keys, children }))
                        }
                    }
                }
            }
        }
    }
}

enum InsertResult {
    Done(Box<BTreeNode>),
    Split(Box<BTreeNode>, i64, Vec<u8>, Box<BTreeNode>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let mut tree = BTree::new();
        tree.insert(10, vec![1]);
        tree.insert(20, vec![2]);
        tree.insert(5, vec![3]);

        assert_eq!(tree.search(10), Some(&vec![1]));
        assert_eq!(tree.search(20), Some(&vec![2]));
        assert_eq!(tree.search(5), Some(&vec![3]));
        assert_eq!(tree.search(99), None);
    }

    #[test]
    fn test_contains() {
        let mut tree = BTree::new();
        tree.insert(1, vec![]);
        tree.insert(2, vec![]);
        assert!(tree.contains(1));
        assert!(!tree.contains(3));
    }

    #[test]
    fn test_insert_causes_split() {
        let mut tree = BTree::with_order(3);
        // Order 3 = max 2 keys per node
        for i in 1..=10 {
            tree.insert(i as i64, vec![i as u8]);
        }
        for i in 1..=10 {
            assert!(tree.contains(i as i64), "Missing key {}", i);
        }
        let keys = tree.keys();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_range_query() {
        let mut tree = BTree::new();
        for i in 1..=20 {
            tree.insert(i, vec![i as u8]);
        }
        let range = tree.range_query(5, 10);
        assert_eq!(range.len(), 6);
        for (i, (k, _)) in range.iter().enumerate() {
            assert_eq!(*k, (i + 5) as i64);
        }
    }

    #[test]
    fn test_range_query_empty() {
        let mut tree = BTree::new();
        for i in 1..=5 {
            tree.insert(i, vec![]);
        }
        let range = tree.range_query(10, 20);
        assert!(range.is_empty());
    }

    #[test]
    fn test_update_existing() {
        let mut tree = BTree::new();
        tree.insert(1, vec![1]);
        tree.insert(1, vec![2]);
        assert_eq!(tree.search(1), Some(&vec![2]));
    }

    #[test]
    fn test_large_insertion() {
        let mut tree = BTree::with_order(4);
        for i in 0..100 {
            tree.insert(i, format!("val_{}", i).into_bytes());
        }
        for i in 0..100 {
            assert!(tree.contains(i));
        }
        let keys = tree.keys();
        assert_eq!(keys.len(), 100);
        for i in 0..99 {
            assert!(keys[i] < keys[i + 1]);
        }
    }

    #[test]
    fn test_reverse_insertion() {
        let mut tree = BTree::with_order(3);
        for i in (1..=10).rev() {
            tree.insert(i, vec![i as u8]);
        }
        let keys = tree.keys();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }
}
