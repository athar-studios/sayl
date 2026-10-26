//! Engine configuration options.

use std::time::Duration;

use crate::lease::DEFAULT_CHUNK_SIZE;
use crate::queue::QueueLimits;

/// Default duration of stall watchdog without progress (15 seconds).
pub const DEFAULT_STALL_TIMEOUT: Duration = Duration::from_secs(15);

/// Configuration options for the download engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum concurrent network connections per task (used for parallel ranged downloads).
    pub max_connections: usize,
    /// Maximum concurrent download tasks actively running at any time.
    pub max_concurrent_tasks: usize,
    /// Range chunk size in bytes (defaults to 4 MiB).
    pub chunk_size: usize,
    /// Queue and scheduling limits (global concurrency, per-origin, queue depth, retries, bandwidth).
    pub queue_limits: QueueLimits,
    /// Duration of stall watchdog without byte progress before aborting and retrying (defaults to 15s).
    pub stall_timeout: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_connections: 4,
            max_concurrent_tasks: 3,
            chunk_size: DEFAULT_CHUNK_SIZE,
            queue_limits: QueueLimits::default(),
            stall_timeout: DEFAULT_STALL_TIMEOUT,
        }
    }
}
