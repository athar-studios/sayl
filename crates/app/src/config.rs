//! Engine configuration options.

use crate::lease::DEFAULT_CHUNK_SIZE;

/// Configuration options for the download engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum concurrent network connections per task (used for parallel ranged downloads).
    pub max_connections: usize,
    /// Maximum concurrent download tasks actively running at any time.
    pub max_concurrent_tasks: usize,
    /// Range chunk size in bytes (defaults to 4 MiB).
    pub chunk_size: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_connections: 4,
            max_concurrent_tasks: 3,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}
