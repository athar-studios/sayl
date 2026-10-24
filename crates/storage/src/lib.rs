//! # sayl-storage
//!
//! Crash-safe task store and checkpoint persistence for the SAYL Download Manager.
//!
//! Designed for environments without a C compiler (pure Rust), combining
//! atomic snapshots and append-only write-ahead journals.
//!
//! ## Overview
//! - [`record`]: Core domain persistence models ([`TaskRecord`], [`Checkpoint`]).
//! - [`journal`]: Append-only JSON-Lines write-ahead journal ([`Journal`], [`Entry`], [`ReplayStats`]).
//! - [`store`]: High-level persistence API ([`Store`], [`StoreStats`]) supporting atomic snapshots,
//!   single-writer concurrency locking, and crash recovery.
//! - [`error`]: Storage error taxonomy ([`StoreError`], [`Result`]) with mapping to [`sayl_core::CoreError`].
//!
//! ## Quick Example
//! ```
//! use sayl_storage::{Store, TaskRecord, Checkpoint};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let store = Store::open_in_memory()?;
//!
//! let task = TaskRecord::new("https://example.com/file.zip", "C:\\downloads", "file.zip");
//! let task_id = task.id;
//!
//! store.upsert_task(&task)?;
//! assert_eq!(store.get_task(task_id)?.unwrap().url, "https://example.com/file.zip");
//!
//! let checkpoint = Checkpoint::new(task_id, 1024, Some(4096), vec![(0, 1024)]);
//! store.save_checkpoint(&checkpoint)?;
//! assert_eq!(store.load_checkpoint(task_id)?.unwrap().written, 1024);
//!
//! store.delete_task(task_id)?;
//! assert!(store.get_task(task_id)?.is_none());
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod journal;
pub mod record;
pub mod store;

pub use error::{Result, StoreError};
pub use journal::{Entry, Journal, JournalOp, ReplayStats};
pub use record::{Checkpoint, TaskRecord};
pub use store::{Store, StoreStats};
