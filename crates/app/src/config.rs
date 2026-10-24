//! Engine configuration options.

use crate::lease::DEFAULT_CHUNK_SIZE;
use crate::queue::QueueLimits;

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
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_connections: 4,
            max_concurrent_tasks: 3,
            chunk_size: DEFAULT_CHUNK_SIZE,
            queue_limits: QueueLimits::default(),
        }
    }
}
