//! Concurrency control: two-phase locking (2PL) and timestamp ordering basics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Lock type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LockType {
    Shared,  // Read lock
    Exclusive, // Write lock
}

/// State of a lock on an item.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockState {
    lock_type: LockType,
    holders: Vec<usize>, // Transaction IDs holding the lock
}

/// Two-Phase Locking (2PL) concurrency controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoPhaseLocking {
    locks: HashMap<String, LockState>,
    /// Tracks if each transaction is in growing phase (true) or shrinking phase (false).
    phase: HashMap<usize, bool>, // true = growing, false = shrinking
    /// Tracks locks held by each transaction.
    txn_locks: HashMap<usize, Vec<(String, LockType)>>,
}

impl TwoPhaseLocking {
    pub fn new() -> Self {
        TwoPhaseLocking {
            locks: HashMap::new(),
            phase: HashMap::new(),
            txn_locks: HashMap::new(),
        }
    }

    /// Request a lock for a transaction. Returns true if granted.
    pub fn lock(&mut self, txn: usize, item: &str, lock_type: LockType) -> LockResult {
        // Check if in shrinking phase
        if let Some(&false) = self.phase.get(&txn) {
            return LockResult::Denied("Transaction in shrinking phase".into());
        }

        // Mark as growing phase
        self.phase.entry(txn).or_insert(true);

        let current = self.locks.get(item).cloned();

        match current {
            None => {
                // No lock exists, grant it
                self.locks.insert(
                    item.to_string(),
                    LockState {
                        lock_type,
                        holders: vec![txn],
                    },
                );
                self.txn_locks
                    .entry(txn)
                    .or_default()
                    .push((item.to_string(), lock_type));
                LockResult::Granted
            }
            Some(state) => {
                // Already holding?
                if state.holders.contains(&txn) {
                    // Upgrade from shared to exclusive
                    if state.lock_type == LockType::Shared && lock_type == LockType::Exclusive {
                        if state.holders.len() == 1 {
                            // Can upgrade
                            self.locks.get_mut(item).unwrap().lock_type = LockType::Exclusive;
                            // Update txn_locks
                            if let Some(locks) = self.txn_locks.get_mut(&txn) {
                                for (_, lt) in locks.iter_mut().filter(|(it, _)| it == item) {
                                    *lt = LockType::Exclusive;
                                }
                            }
                            return LockResult::Granted;
                        } else {
                            return LockResult::Denied("Cannot upgrade: other shared holders".into());
                        }
                    }
                    // Already have sufficient lock
                    return LockResult::Granted;
                }

                // Different transaction
                match (&state.lock_type, &lock_type) {
                    (LockType::Shared, LockType::Shared) => {
                        // Can share
                        let state = self.locks.get_mut(item).unwrap();
                        state.holders.push(txn);
                        self.txn_locks
                            .entry(txn)
                            .or_default()
                            .push((item.to_string(), lock_type));
                        LockResult::Granted
                    }
                    (LockType::Shared, LockType::Exclusive)
                    | (LockType::Exclusive, LockType::Shared)
                    | (LockType::Exclusive, LockType::Exclusive) => {
                        LockResult::Denied(format!("Lock conflict on {}", item))
                    }
                }
            }
        }
    }

    /// Release all locks held by a transaction (enters shrinking phase).
    pub fn unlock_all(&mut self, txn: usize) {
        self.phase.insert(txn, false); // Enter shrinking phase
        if let Some(locks) = self.txn_locks.remove(&txn) {
            for (item, _) in locks {
                if let Some(state) = self.locks.get_mut(&item) {
                    state.holders.retain(|&t| t != txn);
                    if state.holders.is_empty() {
                        self.locks.remove(&item);
                    }
                }
            }
        }
    }

    /// Release a specific lock.
    pub fn unlock(&mut self, txn: usize, item: &str) -> LockResult {
        // Entering shrinking phase
        self.phase.insert(txn, false);

        if let Some(state) = self.locks.get_mut(item) {
            if state.holders.contains(&txn) {
                state.holders.retain(|&t| t != txn);
                if state.holders.is_empty() {
                    self.locks.remove(item);
                }
                if let Some(locks) = self.txn_locks.get_mut(&txn) {
                    locks.retain(|(it, _)| it != item);
                }
                LockResult::Granted
            } else {
                LockResult::Denied("Transaction does not hold this lock".into())
            }
        } else {
            LockResult::Denied("No lock on item".into())
        }
    }

    /// Check if a transaction holds a lock on an item.
    pub fn holds_lock(&self, txn: usize, item: &str) -> bool {
        self.locks
            .get(item)
            .map(|s| s.holders.contains(&txn))
            .unwrap_or(false)
    }

    /// Check if a transaction is 2PL compliant (never requests after releasing).
    pub fn is_compliant(&self, txn: usize) -> bool {
        // If in shrinking phase, no more lock requests allowed
        self.phase.get(&txn).copied().unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LockResult {
    Granted,
    Denied(String),
}

/// Timestamp ordering concurrency controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampOrdering {
    /// Timestamp assigned to each transaction.
    timestamps: HashMap<usize, u64>,
    /// Read timestamp for each item (max timestamp of transactions that read it).
    read_ts: HashMap<String, u64>,
    /// Write timestamp for each item (timestamp of transaction that wrote it).
    write_ts: HashMap<String, u64>,
    next_ts: u64,
}

impl TimestampOrdering {
    pub fn new() -> Self {
        TimestampOrdering {
            timestamps: HashMap::new(),
            read_ts: HashMap::new(),
            write_ts: HashMap::new(),
            next_ts: 1,
        }
    }

    /// Register a transaction with a timestamp.
    pub fn begin(&mut self, txn: usize) -> u64 {
        let ts = self.next_ts;
        self.next_ts += 1;
        self.timestamps.insert(txn, ts);
        ts
    }

    fn ts(&self, txn: usize) -> u64 {
        self.timestamps.get(&txn).copied().unwrap_or(0)
    }

    /// Try to read. Returns Ok(()) if allowed.
    pub fn read(&mut self, txn: usize, item: &str) -> Result<(), String> {
        let ts = self.ts(txn);
        let w_ts = self.write_ts.get(item).copied().unwrap_or(0);

        // Thomas Write Rule / basic check: read must be after write
        if ts < w_ts {
            return Err(format!(
                "Transaction {} (ts={}) trying to read {} written by ts={}: too late",
                txn, ts, item, w_ts
            ));
        }

        // Update read timestamp
        let r_ts = self.read_ts.get(item).copied().unwrap_or(0);
        self.read_ts.insert(item.to_string(), r_ts.max(ts));
        Ok(())
    }

    /// Try to write. Returns Ok(()) if allowed.
    pub fn write(&mut self, txn: usize, item: &str) -> Result<(), String> {
        let ts = self.ts(txn);
        let r_ts = self.read_ts.get(item).copied().unwrap_or(0);
        let w_ts = self.write_ts.get(item).copied().unwrap_or(0);

        if ts < r_ts {
            return Err(format!(
                "Transaction {} (ts={}) trying to write {} read by ts={}: too late",
                txn, ts, item, r_ts
            ));
        }

        if ts < w_ts {
            // Thomas Write Rule: silently ignore (the write is already obsolete)
            return Ok(());
        }

        self.write_ts.insert(item.to_string(), ts);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2pl_basic() {
        let mut cc = TwoPhaseLocking::new();
        assert_eq!(cc.lock(1, "A", LockType::Exclusive), LockResult::Granted);
        assert_eq!(cc.lock(2, "A", LockType::Shared), LockResult::Denied("Lock conflict on A".into()));
        assert!(cc.holds_lock(1, "A"));
        assert!(!cc.holds_lock(2, "A"));
    }

    #[test]
    fn test_2pl_shared_locks() {
        let mut cc = TwoPhaseLocking::new();
        assert_eq!(cc.lock(1, "A", LockType::Shared), LockResult::Granted);
        assert_eq!(cc.lock(2, "A", LockType::Shared), LockResult::Granted);
        assert!(cc.holds_lock(1, "A"));
        assert!(cc.holds_lock(2, "A"));
    }

    #[test]
    fn test_2pl_upgrade() {
        let mut cc = TwoPhaseLocking::new();
        assert_eq!(cc.lock(1, "A", LockType::Shared), LockResult::Granted);
        assert_eq!(cc.lock(1, "A", LockType::Exclusive), LockResult::Granted);
    }

    #[test]
    fn test_2pl_unlock_shrinking() {
        let mut cc = TwoPhaseLocking::new();
        assert_eq!(cc.lock(1, "A", LockType::Exclusive), LockResult::Granted);
        cc.unlock(1, "A");
        // Now in shrinking phase — can't acquire new locks
        assert_eq!(
            cc.lock(1, "B", LockType::Exclusive),
            LockResult::Denied("Transaction in shrinking phase".into())
        );
    }

    #[test]
    fn test_2pl_unlock_all() {
        let mut cc = TwoPhaseLocking::new();
        cc.lock(1, "A", LockType::Exclusive);
        cc.lock(1, "B", LockType::Shared);
        cc.unlock_all(1);
        assert!(!cc.holds_lock(1, "A"));
        assert!(!cc.holds_lock(1, "B"));
    }

    #[test]
    fn test_timestamp_ordering_basic() {
        let mut cc = TimestampOrdering::new();
        cc.begin(1);
        cc.begin(2);

        assert!(cc.write(1, "A").is_ok());
        assert!(cc.read(2, "A").is_ok());
    }

    #[test]
    fn test_timestamp_ordering_conflict() {
        let mut cc = TimestampOrdering::new();
        cc.begin(1); // ts=1
        cc.begin(2); // ts=2

        assert!(cc.write(2, "A").is_ok()); // ts=2 writes A
        assert!(cc.read(1, "A").is_err()); // ts=1 tries to read A (too late)
    }

    #[test]
    fn test_timestamp_ordering_thomas_write() {
        let mut cc = TimestampOrdering::new();
        cc.begin(1); // ts=1
        cc.begin(2); // ts=2

        assert!(cc.write(2, "A").is_ok()); // ts=2 writes A
        assert!(cc.write(1, "A").is_ok()); // ts=1 writes A (Thomas: silently ignored)
    }

    #[test]
    fn test_timestamp_read_after_write() {
        let mut cc = TimestampOrdering::new();
        cc.begin(1); // ts=1
        cc.begin(2); // ts=2

        assert!(cc.write(1, "X").is_ok());
        assert!(cc.read(2, "X").is_ok()); // ts=2 reads after ts=1 wrote — fine
    }
}
