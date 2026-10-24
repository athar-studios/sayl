//! sayl-app: Application services, orchestration, and download engine.
//!
//! Ties together `sayl-core`, `sayl-http`, and `sayl-storage` into a complete,
//! crash-safe, resumable download manager engine.

pub mod config;
pub mod engine;
pub mod io_util;
pub mod lease;
pub mod parallel;
pub mod sequential;

pub use config::EngineConfig;
pub use engine::Engine;
pub use lease::{LeaseReport, LeaseTable, RangeLease, WorkerId, DEFAULT_CHUNK_SIZE, DEFAULT_LEASE_TTL};
pub use sequential::DownloadOutcome;
