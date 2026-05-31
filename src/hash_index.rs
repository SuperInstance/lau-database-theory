//! Hash indexes: chained hashing and extendible hashing.

use serde::{Deserialize, Serialize};

/// Chained hash table with separate chaining for collision resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainedHashTable {
    buckets: Vec<Vec<(i64, Vec<u8>)>>,
    capacity: usize,
    size: usize,
}

impl ChainedHashTable {
    pub fn new(capacity: usize) -> Self {
        ChainedHashTable {
            buckets: vec![Vec::new(); capacity],
            capacity,
            size: 0,
        }
    }

    fn hash(&self, key: i64) -> usize {
        (key as usize) % self.capacity
    }

    pub fn insert(&mut self, key: i64, value: Vec<u8>) {
        let idx = self.hash(key);
        // Check if key exists (update)
        for entry in &mut self.buckets[idx] {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }
        self.buckets[idx].push((key, value));
        self.size += 1;
    }

    pub fn get(&self, key: i64) -> Option<&Vec<u8>> {
        let idx = self.hash(key);
        self.buckets[idx]
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: i64) -> bool {
        let idx = self.hash(key);
        let bucket = &mut self.buckets[idx];
        if let Some(pos) = bucket.iter().position(|(k, _)| *k == key) {
            bucket.remove(pos);
            self.size -= 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, key: i64) -> bool {
        self.get(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the load factor (size / capacity).
    pub fn load_factor(&self) -> f64 {
        self.size as f64 / self.capacity as f64
    }
}

/// Extendible hash table using global and local depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendibleHashTable {
    global_depth: usize,
    directory: Vec<usize>, // indices into buckets
    buckets: Vec<ExtendibleBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtendibleBucket {
    local_depth: usize,
    entries: Vec<(i64, Vec<u8>)>,
}

impl ExtendibleHashTable {
    const BUCKET_CAPACITY: usize = 3;

    pub fn new() -> Self {
        let bucket = ExtendibleBucket {
            local_depth: 0,
            entries: Vec::new(),
        };
        ExtendibleHashTable {
            global_depth: 0,
            directory: vec![0],
            buckets: vec![bucket],
        }
    }

    fn hash(&self, key: i64) -> usize {
        // Use lower bits of hash
        let h = (key as usize).wrapping_mul(2654435761); // Knuth's multiplicative hash
        let mask = (1 << self.global_depth) - 1;
        h & mask
    }

    fn bucket_index_for_key(&self, key: i64) -> usize {
        let h = self.hash(key);
        self.directory[h]
    }

    pub fn insert(&mut self, key: i64, value: Vec<u8>) {
        let dir_idx = self.hash(key);
        let bucket_idx = self.directory[dir_idx];

        // Check for update
        for entry in &mut self.buckets[bucket_idx].entries {
            if entry.0 == key {
                entry.1 = value;
                return;
            }
        }

        if self.buckets[bucket_idx].entries.len() < Self::BUCKET_CAPACITY {
            self.buckets[bucket_idx].entries.push((key, value));
        } else {
            // Need to split
            self.split_and_insert(dir_idx, key, value);
        }
    }

    fn split_and_insert(&mut self, dir_idx: usize, key: i64, value: Vec<u8>) {
        let bucket_idx = self.directory[dir_idx];
        let local_depth = self.buckets[bucket_idx].local_depth;

        if local_depth == self.global_depth {
            // Double the directory
            let old_len = self.directory.len();
            self.directory.reserve(old_len);
            for i in 0..old_len {
                self.directory.push(self.directory[i]);
            }
            self.global_depth += 1;
        }

        // Create new bucket
        let new_depth = local_depth + 1;
        let old_entries: Vec<(i64, Vec<u8>)> = self.buckets[bucket_idx].entries.drain(..).collect();
        self.buckets[bucket_idx].local_depth = new_depth;

        let new_bucket = ExtendibleBucket {
            local_depth: new_depth,
            entries: Vec::new(),
        };
        let new_bucket_idx = self.buckets.len();
        self.buckets.push(new_bucket);

        // Update directory: entries where the new_depth-th bit is 1 point to new bucket
        let mask = 1 << (new_depth - 1);
        for i in 0..self.directory.len() {
            if self.directory[i] == bucket_idx {
                // Check if this directory entry should point to new bucket
                if (i & mask) != 0 {
                    self.directory[i] = new_bucket_idx;
                }
            }
        }

        // Rehash old entries
        for (k, v) in old_entries {
            let idx = self.hash(k);
            let bidx = self.directory[idx];
            self.buckets[bidx].entries.push((k, v));
        }

        // Insert the new entry
        let idx = self.hash(key);
        let bidx = self.directory[idx];
        if self.buckets[bidx].entries.len() < Self::BUCKET_CAPACITY {
            self.buckets[bidx].entries.push((key, value));
        } else {
            // Recursive split
            self.split_and_insert(idx, key, value);
        }
    }

    pub fn get(&self, key: i64) -> Option<&Vec<u8>> {
        let dir_idx = self.hash(key);
        let bucket_idx = self.directory[dir_idx];
        self.buckets[bucket_idx]
            .entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    pub fn contains(&self, key: i64) -> bool {
        self.get(key).is_some()
    }

    pub fn global_depth(&self) -> usize {
        self.global_depth
    }

    pub fn num_buckets(&self) -> usize {
        self.buckets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chained_hash_insert_get() {
        let mut ht = ChainedHashTable::new(4);
        ht.insert(1, vec![1]);
        ht.insert(2, vec![2]);
        ht.insert(5, vec![5]); // Collides with 1 in mod-4

        assert_eq!(ht.get(1), Some(&vec![1]));
        assert_eq!(ht.get(2), Some(&vec![2]));
        assert_eq!(ht.get(5), Some(&vec![5]));
        assert_eq!(ht.get(99), None);
    }

    #[test]
    fn test_chained_hash_remove() {
        let mut ht = ChainedHashTable::new(4);
        ht.insert(1, vec![1]);
        assert!(ht.remove(1));
        assert!(!ht.contains(1));
        assert!(!ht.remove(1)); // Already removed
    }

    #[test]
    fn test_chained_hash_update() {
        let mut ht = ChainedHashTable::new(4);
        ht.insert(1, vec![1]);
        ht.insert(1, vec![2]);
        assert_eq!(ht.get(1), Some(&vec![2]));
        assert_eq!(ht.len(), 1);
    }

    #[test]
    fn test_chained_hash_load_factor() {
        let mut ht = ChainedHashTable::new(10);
        for i in 0..5 {
            ht.insert(i, vec![]);
        }
        let lf = ht.load_factor();
        assert!((lf - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_extendible_hash_basic() {
        let mut ht = ExtendibleHashTable::new();
        ht.insert(1, vec![1]);
        ht.insert(2, vec![2]);
        ht.insert(3, vec![3]);

        assert_eq!(ht.get(1), Some(&vec![1]));
        assert_eq!(ht.get(2), Some(&vec![2]));
        assert_eq!(ht.get(3), Some(&vec![3]));
    }

    #[test]
    fn test_extendible_hash_growth() {
        let mut ht = ExtendibleHashTable::new();
        // Insert enough to trigger splits
        for i in 0..20 {
            ht.insert(i, format!("v{}", i).into_bytes());
        }
        // All should be findable
        for i in 0..20 {
            assert!(ht.contains(i), "Missing key {}", i);
        }
        assert!(ht.num_buckets() > 1);
    }

    #[test]
    fn test_extendible_hash_update() {
        let mut ht = ExtendibleHashTable::new();
        ht.insert(1, vec![1]);
        ht.insert(1, vec![2]);
        assert_eq!(ht.get(1), Some(&vec![2]));
    }
}
