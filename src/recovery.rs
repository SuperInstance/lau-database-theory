//! Recovery: Write-Ahead Logging (WAL) basics and ARIES-style redo/undo.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A log record in the WAL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogRecord {
    /// Transaction started.
    Begin { lsn: u64, txn: usize },
    /// Update operation.
    Update {
        lsn: u64,
        txn: usize,
        page_id: usize,
        offset: usize,
        before: Vec<u8>, // Undo information
        after: Vec<u8>,  // Redo information
    },
    /// Transaction committed.
    Commit { lsn: u64, txn: usize },
    /// Transaction aborted/rolled back.
    Abort { lsn: u64, txn: usize },
    /// Checkpoint.
    Checkpoint { lsn: u64, active_txns: Vec<usize> },
    /// Compensation log record (for ARIES undo).
    CLR {
        lsn: u64,
        txn: usize,
        undo_next_lsn: Option<u64>, // Next LSN to undo in ARIES
        page_id: usize,
        offset: usize,
        after: Vec<u8>, // Redo info for the compensation
    },
}

impl LogRecord {
    pub fn lsn(&self) -> u64 {
        match self {
            LogRecord::Begin { lsn, .. } => *lsn,
            LogRecord::Update { lsn, .. } => *lsn,
            LogRecord::Commit { lsn, .. } => *lsn,
            LogRecord::Abort { lsn, .. } => *lsn,
            LogRecord::Checkpoint { lsn, .. } => *lsn,
            LogRecord::CLR { lsn, .. } => *lsn,
        }
    }

    pub fn txn(&self) -> Option<usize> {
        match self {
            LogRecord::Begin { txn, .. } => Some(*txn),
            LogRecord::Update { txn, .. } => Some(*txn),
            LogRecord::Commit { txn, .. } => Some(*txn),
            LogRecord::Abort { txn, .. } => Some(*txn),
            LogRecord::CLR { txn, .. } => Some(*txn),
            LogRecord::Checkpoint { .. } => None,
        }
    }
}

/// Write-Ahead Log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WAL {
    records: Vec<LogRecord>,
    next_lsn: u64,
    /// Tracks the last LSN for each transaction.
    last_lsn: HashMap<usize, u64>,
    /// Page-level LSN tracking (for ARIES).
    page_lsn: HashMap<usize, u64>,
}

impl WAL {
    pub fn new() -> Self {
        WAL {
            records: Vec::new(),
            next_lsn: 1,
            last_lsn: HashMap::new(),
            page_lsn: HashMap::new(),
        }
    }

    fn alloc_lsn(&mut self) -> u64 {
        let lsn = self.next_lsn;
        self.next_lsn += 1;
        lsn
    }

    pub fn begin(&mut self, txn: usize) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::Begin { lsn, txn });
        self.last_lsn.insert(txn, lsn);
        lsn
    }

    pub fn update(
        &mut self,
        txn: usize,
        page_id: usize,
        offset: usize,
        before: Vec<u8>,
        after: Vec<u8>,
    ) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::Update {
            lsn,
            txn,
            page_id,
            offset,
            before,
            after,
        });
        self.last_lsn.insert(txn, lsn);
        self.page_lsn.insert(page_id, lsn);
        lsn
    }

    pub fn commit(&mut self, txn: usize) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::Commit { lsn, txn });
        self.last_lsn.insert(txn, lsn);
        lsn
    }

    pub fn abort(&mut self, txn: usize) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::Abort { lsn, txn });
        self.last_lsn.insert(txn, lsn);
        lsn
    }

    pub fn checkpoint(&mut self, active_txns: Vec<usize>) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::Checkpoint { lsn, active_txns });
        lsn
    }

    pub fn clr(
        &mut self,
        txn: usize,
        undo_next_lsn: Option<u64>,
        page_id: usize,
        offset: usize,
        after: Vec<u8>,
    ) -> u64 {
        let lsn = self.alloc_lsn();
        self.records.push(LogRecord::CLR {
            lsn,
            txn,
            undo_next_lsn,
            page_id,
            offset,
            after,
        });
        self.last_lsn.insert(txn, lsn);
        self.page_lsn.insert(page_id, lsn);
        lsn
    }

    pub fn records(&self) -> &[LogRecord] {
        &self.records
    }

    pub fn last_lsn(&self, txn: usize) -> Option<u64> {
        self.last_lsn.get(&txn).copied()
    }

    pub fn page_lsn(&self, page_id: usize) -> Option<u64> {
        self.page_lsn.get(&page_id).copied()
    }

    /// ARIES Analysis phase: identify dirty pages and active transactions at crash.
    pub fn analyze(&self) -> AnalysisResult {
        let mut active_txns: HashMap<usize, u64> = HashMap::new(); // txn -> last lsn
        let mut dirty_pages: HashMap<usize, u64> = HashMap::new(); // page_id -> rec_lsn
        let mut committed: HashSet<usize> = HashSet::new();
        let mut aborted: HashSet<usize> = HashSet::new();

        // Find last checkpoint
        let checkpoint_lsn = self
            .records
            .iter()
            .rev()
            .find_map(|r| match r {
                LogRecord::Checkpoint { lsn, active_txns: txns } => Some((*lsn, txns.clone())),
                _ => None,
            });

        let start_idx = if let Some((cp_lsn, cp_txns)) = &checkpoint_lsn {
            // Initialize active txns from checkpoint
            for &txn in cp_txns {
                active_txns.insert(txn, 0);
            }
            self.records.iter().position(|r| r.lsn() == *cp_lsn).unwrap_or(0)
        } else {
            0
        };

        use std::collections::HashSet;
        for record in &self.records[start_idx..] {
            match record {
                LogRecord::Begin { lsn, txn } => {
                    active_txns.insert(*txn, *lsn);
                }
                LogRecord::Update { lsn, txn, page_id, .. } => {
                    if active_txns.contains_key(txn) {
                        active_txns.insert(*txn, *lsn);
                        dirty_pages.entry(*page_id).or_insert(*lsn);
                    }
                }
                LogRecord::Commit { lsn, txn } => {
                    active_txns.remove(txn);
                    committed.insert(*txn);
                }
                LogRecord::Abort { lsn, txn } => {
                    active_txns.remove(txn);
                    aborted.insert(*txn);
                }
                LogRecord::CLR { lsn, txn, page_id, .. } => {
                    if active_txns.contains_key(txn) {
                        active_txns.insert(*txn, *lsn);
                        dirty_pages.entry(*page_id).or_insert(*lsn);
                    }
                }
                LogRecord::Checkpoint { .. } => {}
            }
        }

        AnalysisResult {
            active_txns,
            dirty_pages,
            committed,
            aborted,
        }
    }

    /// ARIES Redo phase: replay all updates for dirty pages.
    pub fn redo(&self, analysis: &AnalysisResult, pages: &mut HashMap<usize, Vec<u8>>) {
        let min_rec_lsn = analysis
            .dirty_pages
            .values()
            .min()
            .copied()
            .unwrap_or(0);

        for record in &self.records {
            if record.lsn() < min_rec_lsn {
                continue;
            }

            match record {
                LogRecord::Update {
                    lsn,
                    page_id,
                    offset,
                    after,
                    ..
                } => {
                    if analysis.dirty_pages.contains_key(page_id) {
                        let page = pages.entry(*page_id).or_insert_with(|| Vec::new());
                        // Ensure page is large enough
                        while page.len() < offset + after.len() {
                            page.push(0);
                        }
                        page[*offset..*offset + after.len()].copy_from_slice(after);
                    }
                }
                LogRecord::CLR {
                    lsn,
                    page_id,
                    offset,
                    after,
                    ..
                } => {
                    if analysis.dirty_pages.contains_key(page_id) {
                        let page = pages.entry(*page_id).or_insert_with(|| Vec::new());
                        while page.len() < offset + after.len() {
                            page.push(0);
                        }
                        page[*offset..*offset + after.len()].copy_from_slice(after);
                    }
                }
                _ => {}
            }
        }
    }

    /// ARIES Undo phase: reverse all updates for loser transactions.
    pub fn undo(&mut self, analysis: &AnalysisResult, pages: &mut HashMap<usize, Vec<u8>>) -> Vec<LogRecord> {
        let mut clrs = Vec::new();

        // Get loser transactions
        let losers: Vec<usize> = analysis
            .active_txns
            .keys()
            .copied()
            .collect();

        for txn in &losers {
            // Collect undo info first to avoid borrow conflict
            let undo_ops: Vec<(usize, usize, Vec<u8>)> = self
                .records
                .iter()
                .rev()
                .filter_map(|r| match r {
                    LogRecord::Update { page_id, offset, before, .. } if r.txn() == Some(*txn) => {
                        Some((*page_id, *offset, before.clone()))
                    }
                    _ => None
                })
                .collect();

            for (page_id, offset, before) in undo_ops {
                // Apply undo (restore before image)
                if let Some(page) = pages.get_mut(&page_id) {
                    if page.len() >= offset + before.len() {
                        page[offset..offset + before.len()].copy_from_slice(&before);
                    }
                }

                // Write a CLR
                let clr_lsn = self.next_lsn;
                self.next_lsn += 1;
                let clr = LogRecord::CLR {
                    lsn: clr_lsn,
                    txn: *txn,
                    undo_next_lsn: None,
                    page_id,
                    offset,
                    after: before,
                };
                clrs.push(clr.clone());
                self.records.push(clr);
            }

            // Write abort record
            let lsn = self.next_lsn;
            self.next_lsn += 1;
            self.records.push(LogRecord::Abort { lsn, txn: *txn });
        }

        clrs
    }
}

use std::collections::HashSet;

/// Result of the analysis phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub active_txns: HashMap<usize, u64>,
    pub dirty_pages: HashMap<usize, u64>,
    pub committed: HashSet<usize>,
    pub aborted: HashSet<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_basic() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![0], vec![42]);
        wal.commit(1);

        assert_eq!(wal.records().len(), 3);
        assert!(matches!(wal.records()[0], LogRecord::Begin { txn: 1, .. }));
        assert!(matches!(wal.records()[1], LogRecord::Update { .. }));
        assert!(matches!(wal.records()[2], LogRecord::Commit { txn: 1, .. }));
    }

    #[test]
    fn test_wal_abort() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![0, 0], vec![1, 2]);
        wal.abort(1);

        assert_eq!(wal.records().len(), 3);
    }

    #[test]
    fn test_analysis() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.begin(2);
        wal.update(1, 0, 0, vec![0], vec![1]); // T1 updates page 0
        wal.commit(1);
        wal.update(2, 1, 0, vec![0], vec![2]); // T2 updates page 1
        // T2 is still active (no commit)

        let analysis = wal.analyze();
        assert!(analysis.committed.contains(&1));
        assert!(analysis.active_txns.contains_key(&2));
        assert!(analysis.dirty_pages.contains_key(&1));
    }

    #[test]
    fn test_redo() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![0, 0, 0], vec![10, 20, 30]);
        wal.commit(1);

        let analysis = wal.analyze();
        let mut pages = HashMap::new();
        wal.redo(&analysis, &mut pages);

        assert_eq!(pages[&0], vec![10, 20, 30]);
    }

    #[test]
    fn test_undo() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![5, 5, 5], vec![10, 20, 30]);
        // T1 not committed (loser)

        let analysis = wal.analyze();
        let mut pages = HashMap::new();
        // First redo
        wal.redo(&analysis, &mut pages);
        assert_eq!(pages[&0], vec![10, 20, 30]);

        // Then undo
        let clrs = wal.undo(&analysis, &mut pages);
        assert_eq!(pages[&0], vec![5, 5, 5]); // Restored
        assert!(!clrs.is_empty());
    }

    #[test]
    fn test_checkpoint() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![0], vec![1]);
        wal.checkpoint(vec![1]); // T1 is active
        wal.commit(1);

        let analysis = wal.analyze();
        assert!(analysis.committed.contains(&1));
    }

    #[test]
    fn test_clr_written() {
        let mut wal = WAL::new();
        wal.begin(1);
        wal.update(1, 0, 0, vec![1, 2], vec![3, 4]);
        // T1 is a loser

        let analysis = wal.analyze();
        let mut pages = HashMap::new();
        wal.redo(&analysis, &mut pages);
        let clrs = wal.undo(&analysis, &mut pages);

        // Verify CLR was written
        let has_clr = wal.records().iter().any(|r| matches!(r, LogRecord::CLR { .. }));
        assert!(has_clr);
    }
}
