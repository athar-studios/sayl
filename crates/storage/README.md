# sayl-storage

Crash-safe task persistence and multi-range checkpoint store for the SAYL Download Manager, implemented entirely in pure Rust.

## Architecture

This crate provides durable persistence without relying on C compilers or embedded SQLite libraries (`rusqlite`, `libsqlite3-sys`), making it portable and buildable in environments lacking a C toolchain or Windows SDK.

The storage engine implements an append-only write-ahead log (WAL) paired with atomic snapshot compaction:

```text
[ Mutations ]
     |
     v
[ journal.jsonl ] (Append-only JSON-Lines, flushed per operation)
     |
     v (compact)
[ snapshot.json.tmp ] -> fsync -> atomic rename -> [ snapshot.json ]
     |
     +--> truncate [ journal.jsonl ] to 0 bytes
```

### Crash-Safety Guarantees
1. **Atomic Snapshots**: Snapshot files are written to a unique temporary file, flushed with `sync_all` (fsync), and atomically renamed over `snapshot.json`.
2. **Write-Ahead Journal**: Mutations (`upsert_task`, `delete_task`, `save_checkpoint`) are sequenced and written to `journal.jsonl`.
3. **Truncated Tail Tolerance**: If a process termination or power loss occurs mid-write, the corrupted/partial last line of `journal.jsonl` is ignored on restart, logged with a warning, and pruned back to the last valid byte offset. Corruptions in the middle of the journal are detected and rejected.
4. **Crash Recovery**: On `Store::open(dir)`, the latest snapshot is loaded first, followed by journal replay skipping any entries already folded into the snapshot (`entry.seq <= snapshot.last_seq`).
5. **Single-Writer Exclusivity**: A directory lock file (`store.lock`) using OS-level exclusive file locking (`share_mode(0)` on Windows) combined with an in-process registry ensures multiple `Store` handles on the same directory fail cleanly with `StoreError::Locked` rather than corrupting data.

## API Overview

```rust
use sayl_storage::{Store, TaskRecord, Checkpoint};

// Open persistent or in-memory store
let store = Store::open("C:/data/sayl_store")?;
// or for testing:
let mem_store = Store::open_in_memory()?;

// Tasks
let task = TaskRecord::new("https://example.com/arch.iso", "C:/Downloads", "arch.iso");
store.upsert_task(&task)?;
let loaded = store.get_task(task.id)?;
let all_tasks = store.list_tasks()?;
store.delete_task(task.id)?;

// Checkpoints (for parallel range resumption)
let checkpoint = Checkpoint::new(task.id, 1024, Some(4096), vec![(0, 1024)]);
store.save_checkpoint(&checkpoint)?;
let loaded_cp = store.load_checkpoint(task.id)?;

// Maintenance
store.flush()?;   // fsync barrier
store.compact()?; // Fold journal into atomic snapshot
let stats = store.stats(); // records and journal bytes
```
