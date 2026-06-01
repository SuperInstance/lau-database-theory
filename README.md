# lau-database-theory

**Database theory implementations in pure Rust** — relational algebra, B-tree indexes, hash indexes, query optimization, ACID transactions, two-phase locking, concurrency control, ARIES-style recovery, normal forms, and agent state management.

> *The theory behind every database you've ever used.*

---

## What This Does

This library implements the core data structures and algorithms that power relational databases. Every module covers a fundamental concept from database theory, from the relational model itself through indexing, query planning, transaction processing, crash recovery, and schema normalization. The implementations are teaching-quality: clean abstractions, extensive tests, and real algorithms used in production systems.

The nine modules break down as:

| Module | What you get |
|---|---|
| `relational` | Relations, tuples, values, and the full relational algebra (select, project, join, union, difference, rename) |
| `btree` | B-tree with insert, search, range queries, and node splitting/merging |
| `hash_index` | Chained hash table (separate chaining) and extendible hash table (dynamic hashing) |
| `query_opt` | Query optimizer with cost estimation, selection pushdown, and DP-based join ordering |
| `transactions` | ACID transaction schedules, conflict detection, precedence graphs, conflict serializability |
| `concurrency` | Two-phase locking (2PL) with shared/exclusive locks, lock upgrade, deadlock detection |
| `recovery` | Write-ahead logging (WAL), ARIES-style redo/undo recovery, checkpoints, CLRs |
| `normal_forms` | Functional dependencies, attribute closure, 1NF/2NF/3NF/BCNF, lossless decomposition |
| `agent_state` | Agent state store applying database concepts (B-tree + hash indexes + relations) to AI agent data |

**Stats:** ~3,600 lines of source, 81 tests, zero unsafe code.

---

## Key Idea

Databases are everywhere, but their internals are surprisingly elegant. This library makes the theory tangible:

```
Relational Model (what data looks like)
  ├── Indexing (how to find it fast)
  │   ├── B-trees (range queries, ordered data)
  │   └── Hash tables (point lookups, dynamic resizing)
  ├── Query Optimization (how to answer questions efficiently)
  │   ├── Cost estimation (how much work will this take?)
  │   └── Join ordering (what order to combine tables?)
  ├── Transactions (how to keep data correct under concurrency)
  │   ├── ACID properties, serializability
  │   ├── Two-phase locking
  │   └── Write-ahead logging, ARIES recovery
  └── Normal Forms (how to design good schemas)
      ├── Functional dependencies
      └── Decomposition (1NF → BCNF)
```

The `agent_state` module demonstrates how these concepts apply to managing AI agent state — a practical bridge from theory to application.

---

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
lau-database-theory = "0.1"
```

### Dependencies

- **serde** — serialization of relations, indexes, WAL logs, schemas

---

## Quick Start

### Relational algebra

```rust
use lau_database_theory::{Relation, Tuple, Value};

// Create a relation (table)
let mut users = Relation::new(vec!["id".into(), "name".into(), "age".into()]);
users.insert(Tuple::new(vec![Value::Int(1), Value::Text("Alice".into()), Value::Int(30)]));
users.insert(Tuple::new(vec![Value::Int(2), Value::Text("Bob".into()), Value::Int(25)]));
users.insert(Tuple::new(vec![Value::Int(3), Value::Text("Carol".into()), Value::Int(30)]));

// Selection: find users where age = 30
let age_30 = users.select(|tuple, schema| {
    let age_idx = schema.iter().position(|s| s == "age").unwrap();
    tuple.values[age_idx] == Value::Int(30)
});
assert_eq!(age_30.len(), 2);

// Projection: just names
let names = users.project(&["name".into()]);
// Relation with schema ["name"] and tuples [("Alice"), ("Bob"), ("Carol")]

// Join: combine with another relation
let mut orders = Relation::new(vec!["user_id".into(), "product".into()]);
orders.insert(Tuple::new(vec![Value::Int(1), Value::Text("Widget".into())]));
let joined = users.join(&orders, "id", "user_id");
```

### B-tree indexing

```rust
use lau_database_theory::BTree;

let mut tree = BTree::with_order(4); // max 4 children per node
tree.insert(10, b"ten".to_vec());
tree.insert(20, b"twenty".to_vec());
tree.insert(5, b"five".to_vec());

assert_eq!(tree.search(&10), Some(&b"ten".to_vec()));
let range = tree.range_query(5, 15); // all entries with 5 ≤ key ≤ 15
```

### Hash indexing

```rust
use lau_database_theory::{ChainedHashTable, ExtendibleHashTable};

// Separate chaining
let mut ht = ChainedHashTable::new(16);
ht.insert(42, b"value".to_vec());
assert!(ht.contains(42));
println!("load factor: {:.2}", ht.load_factor());

// Extendible hashing (auto-resizes)
let mut eht = ExtendibleHashTable::new();
eht.insert(1, b"one".to_vec());
eht.insert(17, b"seventeen".to_vec()); // may cause a split
```

### Query optimization

```rust
use lau_database_theory::QueryOptimizer;

let optimizer = QueryOptimizer::new();
// Add relation statistics, define join conditions
// Optimizer uses DP to find the lowest-cost join order
// Supports: TableScan, IndexScan, Selection, Projection, NLJoin, HashJoin, SortMergeJoin
```

### ACID transactions and serializability

```rust
use lau_database_theory::{Schedule, Operation};

let schedule = Schedule::new(vec![
    Operation::read(1, "x"),   // T1 reads x
    Operation::write(2, "x"),  // T2 writes x
    Operation::read(2, "y"),   // T2 reads y
    Operation::write(1, "y"),  // T1 writes y
]);

let graph = schedule.precedence_graph();
let is_serializable = graph.is_acyclic();
println!("conflict serializable: {}", is_serializable);
```

### Two-phase locking

```rust
use lau_database_theory::{TwoPhaseLocking, LockType, LockResult};

let mut tpl = TwoPhaseLocking::new();
assert_eq!(tpl.lock(1, "x", LockType::Shared), LockResult::Granted);
assert_eq!(tpl.lock(1, "y", LockType::Exclusive), LockResult::Granted);
assert_eq!(tpl.lock(2, "x", LockType::Exclusive), LockResult::Denied); // blocked
tpl.unlock(1, "x"); // now in shrinking phase — can't acquire new locks
```

### Write-ahead logging and recovery

```rust
use lau_database_theory::{WAL, LogRecord};

let mut wal = WAL::new();
let lsn1 = wal.append(LogRecord::Begin { txn: 1 });
let lsn2 = wal.append(LogRecord::Update {
    lsn: 0, txn: 1, page_id: 0, offset: 0,
    before: b"old".to_vec(), after: b"new".to_vec(),
});
let lsn3 = wal.append(LogRecord::Commit { txn: 1 });

// After a crash, run ARIES recovery:
// Phase 1: Analysis — scan log to find dirty pages and active transactions
// Phase 2: Redo — replay all updates from the last checkpoint
// Phase 3: Undo — roll back uncommitted transactions
```

### Normal forms and functional dependencies

```rust
use lau_database_theory::{FD, closure, is_bcnf};

let fds = vec![
    FD::new(vec!["student_id"], vec!["student_name"]),
    FD::new(vec!["student_id", "course_id"], vec!["grade"]),
];
let all_attrs = HashSet::from(["student_id", "student_name", "course_id", "grade"]);

// Compute attribute closure
let key = HashSet::from(["student_id"]);
let closed = closure(&key, &fds);

// Check normal form violations
// (student_name) depends on (student_id) alone — not a superkey of the full relation
// → violates BCNF → decompose into Students(student_id, student_name) and Grades(student_id, course_id, grade)
```

### Agent state store

```rust
use lau_database_theory::AgentStateStore;

let mut store = AgentStateStore::new();
store.put("agent-007", "mood", b"happy".to_vec(), 1000);
store.put("agent-007", "location", b"london".to_vec(), 1001);

let mood = store.get("agent-007", "mood");
assert_eq!(mood, Some(b"happy".to_vec()));

// Uses B-tree index on agent_id + hash index on key + relational table
// — the full database indexing stack applied to agent state
```

---

## API Reference

### `relational`

| Type / Method | Description |
|---|---|
| `Value` | `Int(i64)`, `Float(f64)`, `Text(String)`, `Bool(bool)`, `Null` |
| `Tuple` | Ordered list of values |
| `Relation` | Named schema + collection of tuples |
| `.select(predicate)` | Horizontal filter (σ) |
| `.project(columns)` | Vertical filter (π) |
| `.join(other, left_key, right_key)` | Natural/equi-join (⋈) |
| `.union(other)` | Set union (∪) |
| `.difference(other)` | Set difference (−) |
| `.rename(mapping)` | Attribute renaming (ρ) |

### `btree`

| Type / Method | Description |
|---|---|
| `BTree::new()` | Create with default order (4) |
| `BTree::with_order(n)` | Create with custom order (≥ 3) |
| `.insert(key, value)` | Insert with automatic node splitting |
| `.search(key)` | Point lookup |
| `.contains(key)` | Existence check |
| `.range_query(low, high)` | Range scan |
| `.len()` / `.is_empty()` | Size queries |
| `BTreeNode` | `Internal { keys, children }` or `Leaf { keys, values }` |

### `hash_index`

| Type / Method | Description |
|---|---|
| `ChainedHashTable::new(capacity)` | Separate chaining hash table |
| `.insert` / `.get` / `.remove` / `.contains` | CRUD operations |
| `.load_factor()` | Current load factor |
| `ExtendibleHashTable::new()` | Dynamic hashing with global/local depth |
| `.insert` / `.get` / `.remove` | Auto-resizing operations |

### `query_opt`

| Type / Method | Description |
|---|---|
| `QueryOptimizer` | DP-based join order optimizer |
| `RelationStats` | Tuple count, page count, column statistics |
| `Cost { io_cost, cpu_cost }` | Estimated query plan cost |
| `PlanNode` | `TableScan`, `IndexScan`, `Selection`, `Projection`, `NestedLoopJoin`, `HashJoin`, `SortMergeJoin` |
| `.estimate_output_size()` | Cardinality estimation |
| `.estimate_cost()` | Cost estimation for a plan subtree |

### `transactions`

| Type / Method | Description |
|---|---|
| `Operation` | `Read { txn, item }` or `Write { txn, item }` |
| `conflicts(op1, op2)` | Check if two operations conflict (same item, different txn, one is write) |
| `Schedule` | Ordered sequence of operations |
| `.precedence_graph()` | Build serializability graph |
| `PrecedenceGraph::is_acyclic()` | Check conflict serializability |
| `.topological_sort()` | Find an equivalent serial schedule |

### `concurrency`

| Type / Method | Description |
|---|---|
| `LockType` | `Shared` (read) or `Exclusive` (write) |
| `LockResult` | `Granted`, `Denied(reason)`, `Deadlock` |
| `TwoPhaseLocking` | 2PL controller with growing/shrinking phases |
| `.lock(txn, item, type)` | Request a lock |
| `.unlock(txn, item)` | Release a lock (enters shrinking phase) |
| `detect_deadlock(wait_for_graph)` | Cycle detection in wait-for graph |

### `recovery`

| Type / Method | Description |
|---|---|
| `LogRecord` | `Begin`, `Update`, `Commit`, `Abort`, `Checkpoint`, `CLR` |
| `WAL` | Write-ahead log with LSN tracking |
| `.append(record)` | Append a log record |
| `.checkpoint(active_txns)` | Create a checkpoint |
| `ARIESRecovery` | Three-phase recovery: analysis → redo → undo |
| `.analyze(wal)` | Find dirty pages and active transactions |
| `.redo(wal, dirty_pages)` | Replay committed updates |
| `.undo(wal, active_txns)` | Roll back uncommitted transactions |

### `normal_forms`

| Type / Method | Description |
|---|---|
| `FD` | Functional dependency: lhs → rhs |
| `.is_trivial()` | Check if rhs ⊆ lhs |
| `closure(attrs, fds)` | Compute attribute closure X⁺ under FDs |
| `find_candidate_keys(fds, all_attrs)` | Find all candidate keys |
| `is_1nf` / `is_2nf` / `is_3nf` / `is_bcnf` | Normal form checks |
| `decompose_bcnf(relation, fds)` | BCNF decomposition (lossless, dependency-preserving where possible) |
| `is_lossless_decomposition(decomp, fds)` | Verify lossless join property |

### `agent_state`

| Type / Method | Description |
|---|---|
| `AgentStateStore` | Structured agent state with B-tree + hash + relation |
| `.put(agent_id, key, value, timestamp)` | Store a state value |
| `.get(agent_id, key)` | Retrieve latest value |
| `.query_by_agent(agent_id)` | Get all state for an agent |
| `AgentRecord` | Structured record with metadata |

---

## How It Works

### Relational Algebra

The relational model represents data as relations (tables) with named attributes (columns) and tuples (rows). The five fundamental operations are:

- **Selection** (σ): filter rows by a predicate
- **Projection** (π): pick specific columns
- **Join** (⋈): combine two relations on matching column values
- **Union** (∪): combine tuples from two compatible relations
- **Difference** (−): tuples in one but not the other

Every SQL query can be expressed as a composition of these operations.

### B-Tree Indexing

B-trees maintain sorted data in a balanced tree where every leaf is at the same depth. Internal nodes store separator keys and pointers to children. Insertion may cause a node to exceed its capacity, triggering a split that propagates up. This guarantees O(log n) search, insert, and range query performance.

### Hash Indexing

**Chained hashing**: hash the key to a bucket, store all colliding entries as a linked list. Simple but performance degrades with high load factors.

**Extendible hashing**: maintains a global directory that doubles when a bucket overflows. Only the overflowing bucket splits — other buckets are unaffected. Provides O(1) average lookup with dynamic resizing.

### Query Optimization

The optimizer estimates the cost of different query plans using statistics (tuple counts, distinct values, selectivity estimates). Join ordering is an NP-hard problem in general; this implementation uses dynamic programming over subsets of relations to find the optimal left-deep join tree. Selection pushdown moves filters as close to the data source as possible to reduce intermediate result sizes.

### ACID and Serializability

A schedule is **conflict serializable** if it's equivalent to some serial schedule (where transactions run one at a time). The test: build a precedence graph where edge T_i → T_j exists if T_i's operation precedes T_j's conflicting operation. If the graph is acyclic, the schedule is conflict serializable.

### Two-Phase Locking (2PL)

2PL guarantees serializability by requiring each transaction to have two phases:
1. **Growing phase**: acquire locks, never release
2. **Shrinking phase**: release locks, never acquire

Once a transaction releases any lock, it cannot acquire new locks. This ensures the lock point (the moment of transition) totally orders all transactions, guaranteeing serializability. Deadlocks can occur when transactions wait in a cycle.

### ARIES Recovery

The **Algorithm for Recovery and Isolation Exploiting Semantics** (ARIES) is the standard crash recovery protocol:

1. **Analysis phase**: scan the log from the last checkpoint to identify dirty pages and active transactions
2. **Redo phase**: replay all logged updates (even for uncommitted transactions) to restore the database to its crash-time state
3. **Undo phase**: roll back all updates from transactions that were active at crash time, using Compensation Log Records (CLRs) to handle failures during undo

The Write-Ahead Logging (WAL) rule ensures log records are flushed to disk before the corresponding data pages, guaranteeing no updates are lost.

### Normal Forms

**Functional dependencies** (FDs) capture the constraints of a schema: if X → Y, then knowing X determines Y. The **closure** X⁺ of a set of attributes X under a set of FDs is everything X functionally determines. The normal forms are:

- **1NF**: all values are atomic (no repeating groups)
- **2NF**: no partial dependencies (every non-key attribute depends on the whole key, not just part of it)
- **3NF**: no transitive dependencies (non-key attributes don't depend on other non-key attributes)
- **BCNF**: for every non-trivial FD X → Y, X must be a superkey

Decomposition replaces one relation with two or more that satisfy a higher normal form. **Lossless join** means the original relation can be reconstructed by joining the decomposed relations.

---

## The Math

### Relational Algebra Completeness
The five basic operations (σ, π, ⋈, ∪, −) are **complete**: any relational query can be expressed using only these five. Intersection, natural join, and division are derived operations.

### B-Tree Complexity
For a B-tree of order m with n keys:
- Height: h = O(log_m n)
- Search: O(h) = O(log n)
- Insert: O(h) with at most one split per level
- Range query over k keys: O(h + k)

### Hash Table Complexity
- Chained hash: O(1 + α) average where α = n/m (load factor)
- Extendible hash: O(1) average lookup, O(2^d) directory size where d = global depth

### Join Ordering
For n relations, there are O(n! · 4^n) possible join trees. DP-based optimization over left-deep trees reduces this to O(n² · 2^n), which is tractable for n ≤ 15.

### Conflict Serializability
A schedule S is conflict serializable ⟺ its precedence graph is acyclic ⟺ S is conflict-equivalent to some serial schedule. Testing acyclicity is O(V + E) via DFS.

### ARIES Correctness
ARIES guarantees:
- **Redo**: repeats all updates to bring the database to the crash-time state
- **Undo**: reverses only uncommitted transactions, preserving committed work
- **Idempotence**: re-executing redo or undo operations produces the same result

### BCNF Decomposition
Given relation R with FDs F, decompose by finding a FD X → Y that violates BCNF (X is not a superkey), then replace R with R₁ = X∪Y and R₂ = R−Y. Repeat until all relations are in BCNF. The decomposition is guaranteed to be lossless (preserves all data) but may not preserve all FDs.

---

## License

MIT
