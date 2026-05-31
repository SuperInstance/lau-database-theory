//! Agent state management — structured storage and querying of agent data.
//! Application of database theory for managing AI agent state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::relational::{Relation, Tuple, Value};
use crate::btree::BTree;
use crate::hash_index::ChainedHashTable;

/// An agent's state record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub agent_id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

/// Structured agent state store using database theory concepts.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentStateStore {
    /// Primary index on agent_id using B-tree.
    agent_index: BTree,
    /// Secondary index on key using chained hash.
    key_index: ChainedHashTable,
    /// All records stored as a relation.
    table: Relation,
    /// Auto-incrementing record ID.
    next_id: i64,
}

impl AgentStateStore {
    pub fn new() -> Self {
        AgentStateStore {
            agent_index: BTree::new(),
            key_index: ChainedHashTable::new(64),
            table: Relation::new(vec![
                "id".into(),
                "agent_id_hash".into(),
                "key".into(),
                "value".into(),
                "timestamp".into(),
            ]),
            next_id: 1,
        }
    }

    /// Store a state value for an agent.
    pub fn put(&mut self, agent_id: &str, key: &str, value: Vec<u8>, timestamp: u64) {
        let id = self.next_id;
        self.next_id += 1;

        let agent_hash = self.hash_string(agent_id);

        // Update B-tree index
        self.agent_index.insert(agent_hash, value.clone());

        // Update hash index
        self.key_index.insert(id, key.as_bytes().to_vec());

        // Insert into relation
        self.table.tuples.push(Tuple::new(vec![
            Value::Int(id),
            Value::Int(agent_hash),
            Value::Text(key.into()),
            Value::Text(base64_encode(&value)),
            Value::Int(timestamp as i64),
        ]));
    }

    /// Get the latest value for an agent's key.
    pub fn get(&self, agent_id: &str, key: &str) -> Option<Vec<u8>> {
        let agent_hash = self.hash_string(agent_id);

        // Query the relation
        let result = self.table.select(|t, _schema| {
            t.values[1] == Value::Int(agent_hash) && t.values[2] == Value::Text(key.into())
        });

        if result.tuples.is_empty() {
            None
        } else {
            // Return the latest (highest timestamp)
            let latest = result
                .tuples
                .iter()
                .max_by_key(|t| match &t.values[4] {
                    Value::Int(ts) => *ts,
                    _ => 0,
                })?;
            match &latest.values[3] {
                Value::Text(encoded) => Some(base64_decode(encoded)),
                _ => None,
            }
        }
    }

    /// Get all state entries for an agent.
    pub fn get_all(&self, agent_id: &str) -> Vec<(String, Vec<u8>, u64)> {
        let agent_hash = self.hash_string(agent_id);
        let result = self.table.select(|t, _schema| {
            t.values[1] == Value::Int(agent_hash)
        });

        result
            .tuples
            .iter()
            .filter_map(|t| {
                let key = match &t.values[2] {
                    Value::Text(k) => k.clone(),
                    _ => return None,
                };
                let value = match &t.values[3] {
                    Value::Text(encoded) => base64_decode(encoded),
                    _ => return None,
                };
                let ts = match &t.values[4] {
                    Value::Int(ts) => *ts as u64,
                    _ => return None,
                };
                Some((key, value, ts))
            })
            .collect()
    }

    /// Query agent state with a relational selection.
    pub fn query<F>(&self, predicate: F) -> Vec<AgentRecord>
    where
        F: Fn(&Tuple, &[String]) -> bool,
    {
        let result = self.table.select(predicate);
        result
            .tuples
            .iter()
            .map(|t| AgentRecord {
                agent_id: format!("agent_{}", match &t.values[1] {
                    Value::Int(h) => *h,
                    _ => 0,
                }),
                key: match &t.values[2] {
                    Value::Text(k) => k.clone(),
                    _ => String::new(),
                },
                value: match &t.values[3] {
                    Value::Text(encoded) => base64_decode(encoded),
                    _ => Vec::new(),
                },
                timestamp: match &t.values[4] {
                    Value::Int(ts) => *ts as u64,
                    _ => 0,
                },
                metadata: HashMap::new(),
            })
            .collect()
    }

    /// Project specific columns from the state store.
    pub fn project(&self, columns: &[&str]) -> Relation {
        self.table.project(columns)
    }

    /// Join agent state with another relation.
    pub fn join(&self, other: &Relation) -> Relation {
        self.table.join(other)
    }

    /// Get count of records.
    pub fn len(&self) -> usize {
        self.table.cardinality()
    }

    pub fn is_empty(&self) -> bool {
        self.table.cardinality() == 0
    }

    fn hash_string(&self, s: &str) -> i64 {
        let mut hash: i64 = 5381;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as i64);
        }
        hash.abs()
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(encoded: &str) -> Vec<u8> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let chars: Vec<u8> = encoded.bytes().filter(|&b| b != b'=').collect();

    for chunk in chars.chunks(4) {
        let mut acc: u32 = 0;
        let mut bits = 0;
        for &b in chunk {
            if let Some(pos) = CHARS.iter().position(|&c| c == b) {
                acc = (acc << 6) | pos as u32;
                bits += 6;
            }
        }
        while bits >= 8 {
            bits -= 8;
            result.push((acc >> bits) as u8);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_put_get() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "mood", vec![1, 2, 3], 100);
        let val = store.get("agent-1", "mood");
        assert_eq!(val, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_agent_latest_value() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "mood", vec![1], 100);
        store.put("agent-1", "mood", vec![2], 200);
        let val = store.get("agent-1", "mood");
        assert_eq!(val, Some(vec![2]));
    }

    #[test]
    fn test_agent_get_all() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "mood", vec![1], 100);
        store.put("agent-1", "energy", vec![2], 100);
        let all = store.get_all("agent-1");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_agent_query() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "mood", vec![1], 100);
        store.put("agent-1", "energy", vec![2], 100);
        store.put("agent-2", "mood", vec![3], 200);

        let results = store.query(|t, _schema| {
            t.values[2] == Value::Text("mood".into())
        });
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_agent_project() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "mood", vec![1], 100);
        let projected = store.project(&["key", "timestamp"]);
        assert_eq!(projected.schema, vec!["key", "timestamp"]);
        assert_eq!(projected.cardinality(), 1);
    }

    #[test]
    fn test_agent_isolation() {
        let mut store = AgentStateStore::new();
        store.put("agent-1", "data", vec![1], 100);
        store.put("agent-2", "data", vec![2], 100);
        assert_eq!(store.get("agent-1", "data"), Some(vec![1]));
        assert_eq!(store.get("agent-2", "data"), Some(vec![2]));
    }

    #[test]
    fn test_agent_len() {
        let mut store = AgentStateStore::new();
        assert!(store.is_empty());
        store.put("agent-1", "x", vec![1], 100);
        assert_eq!(store.len(), 1);
        store.put("agent-1", "y", vec![2], 100);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let encoded = base64_encode(&data);
        let decoded = base64_decode(&encoded);
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_base64_empty() {
        let data: Vec<u8> = vec![];
        let encoded = base64_encode(&data);
        let decoded = base64_decode(&encoded);
        assert_eq!(data, decoded);
    }
}
