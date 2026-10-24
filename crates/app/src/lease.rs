//! Lease-based dynamic range splitting scheduler for SAYL parallel transfers.
//!
//! Provides [`LeaseTable`], [`RangeLease`], and [`LeaseReport`] to partition byte
//! ranges into chunks, lease them out to concurrent workers, detect expired leases,
//! perform work-stealing with determinism, and verify non-overlapping full coverage.

use std::time::{Duration, Instant};

/// Default chunk size for range lease table: 4 MiB.
pub const DEFAULT_CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Default lease TTL: 10 seconds.
pub const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(10);

/// Identifier for worker tasks.
pub type WorkerId = usize;

/// A bounded byte range lease held by a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeLease {
    /// Start byte offset (inclusive).
    pub start: u64,
    /// End byte offset (inclusive).
    pub end_inclusive: u64,
    /// Current owner worker id if leased, or `None` if unclaimed.
    pub owner: Option<WorkerId>,
    /// Instant at which this lease expires.
    pub lease_expires_at: Instant,
}

impl RangeLease {
    /// Creates a new `RangeLease`.
    pub fn new(
        start: u64,
        end_inclusive: u64,
        owner: Option<WorkerId>,
        lease_expires_at: Instant,
    ) -> Self {
        Self {
            start,
            end_inclusive,
            owner,
            lease_expires_at,
        }
    }

    /// Length in bytes covered by this lease.
    #[inline]
    pub fn len(&self) -> u64 {
        if self.end_inclusive >= self.start {
            self.end_inclusive - self.start + 1
        } else {
            0
        }
    }

    /// Returns `true` if this lease covers 0 bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start > self.end_inclusive
    }
}

impl AsRef<RangeLease> for RangeLease {
    fn as_ref(&self) -> &RangeLease {
        self
    }
}

/// Lifecycle status of a chunk inside [`LeaseTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStatus {
    /// Not yet claimed, or released back into the pool.
    Unclaimed,
    /// Currently assigned to an active worker.
    Leased,
    /// Successfully downloaded and verified.
    Completed,
}

#[derive(Debug, Clone)]
struct TableEntry {
    start: u64,
    end_inclusive: u64,
    owner: Option<WorkerId>,
    lease_expires_at: Option<Instant>,
    status: ChunkStatus,
    generation: u64,
    attempts: usize,
}

/// Comprehensive report of lease scheduling outcome and invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseReport {
    /// Total number of chunks in the partitioned range.
    pub chunks_total: usize,
    /// Number of chunks successfully completed.
    pub chunks_completed: usize,
    /// Total number of steals performed across expired leases.
    pub steals_performed: usize,
    /// Total number of lease retries / releases.
    pub retries: usize,
    /// All completed byte ranges as `(start, end_inclusive)`.
    pub completed_ranges: Vec<(u64, u64)>,
    /// Expected total size in bytes.
    pub total_size: u64,
}

impl LeaseReport {
    /// Creates a new `LeaseReport`.
    pub fn new(
        chunks_total: usize,
        chunks_completed: usize,
        steals_performed: usize,
        retries: usize,
        completed_ranges: Vec<(u64, u64)>,
        total_size: u64,
    ) -> Self {
        Self {
            chunks_total,
            chunks_completed,
            steals_performed,
            retries,
            completed_ranges,
            total_size,
        }
    }

    /// Asserts that the completed ranges cover `[0, total)` exactly with no overlap and no gaps.
    /// Panics with a descriptive error message if the assertion fails.
    pub fn assert_full_coverage(&self, total: u64) {
        if let Err(err) = self.verify_coverage(total) {
            panic!("Coverage assertion failed: {err}");
        }
    }

    /// Alias for [`Self::assert_full_coverage`].
    pub fn assert_coverage(&self, total: u64) {
        self.assert_full_coverage(total);
    }

    /// Checks that the union of completed ranges equals `[0, total)` with no gaps and no overlap.
    pub fn verify_coverage(&self, total: u64) -> Result<(), String> {
        if total == 0 {
            if self.chunks_total != 0 {
                return Err(format!(
                    "expected 0 chunks for total 0, got {}",
                    self.chunks_total
                ));
            }
            if self.chunks_completed != 0 {
                return Err(format!(
                    "expected 0 completed chunks for total 0, got {}",
                    self.chunks_completed
                ));
            }
            if !self.completed_ranges.is_empty() {
                return Err("completed ranges must be empty for total 0".into());
            }
            return Ok(());
        }

        if self.chunks_completed != self.chunks_total {
            return Err(format!(
                "incomplete coverage: {} of {} chunks completed",
                self.chunks_completed, self.chunks_total
            ));
        }

        if self.completed_ranges.is_empty() {
            return Err(format!("no completed ranges recorded for total {total}"));
        }

        let mut ranges = self.completed_ranges.clone();
        ranges.sort_by_key(|r| r.0);

        if ranges[0].0 != 0 {
            return Err(format!(
                "first range does not start at 0: starts at {}",
                ranges[0].0
            ));
        }

        let mut covered_bytes = 0u64;

        for (i, r) in ranges.iter().enumerate() {
            if r.0 > r.1 {
                return Err(format!("invalid inverted range [{}, {}]", r.0, r.1));
            }
            let len = r.1 - r.0 + 1;
            covered_bytes += len;

            if i + 1 < ranges.len() {
                let next = &ranges[i + 1];
                if next.0 <= r.1 {
                    return Err(format!(
                        "range overlap detected: [{}, {}] overlaps with [{}, {}]",
                        r.0, r.1, next.0, next.1
                    ));
                }
                if next.0 > r.1 + 1 {
                    return Err(format!(
                        "range gap detected between [{}, {}] and [{}, {}]",
                        r.0, r.1, next.0, next.1
                    ));
                }
            }
        }

        let last_end = ranges.last().unwrap().1;
        if last_end != total - 1 {
            return Err(format!(
                "last range does not reach total - 1: ends at {last_end}, expected {}",
                total - 1
            ));
        }

        if covered_bytes != total {
            return Err(format!(
                "covered bytes sum {covered_bytes} does not match expected total {total}"
            ));
        }

        Ok(())
    }

    /// Returns `true` if any completed ranges overlap.
    pub fn has_overlap(&self) -> bool {
        let mut ranges = self.completed_ranges.clone();
        ranges.sort_by_key(|r| r.0);
        for i in 0..ranges.len().saturating_sub(1) {
            if ranges[i + 1].0 <= ranges[i].1 {
                return true;
            }
        }
        false
    }
}

/// Dynamic range splitting scheduler and lease table.
///
/// Splits `[0, total)` into chunks of `chunk_size` and tracks worker leases,
/// expiration, work stealing, and completions deterministically without global clock reads.
#[derive(Debug, Clone)]
pub struct LeaseTable {
    total_size: u64,
    chunk_size: usize,
    chunks: Vec<TableEntry>,
    completed_count: usize,
    steals_performed: usize,
    retries: usize,
    completed_ranges: Vec<(u64, u64)>,
}

impl LeaseTable {
    /// Creates a new `LeaseTable` splitting `[0, total_size)` into chunks of `chunk_size`.
    ///
    /// When `total_size == 0`, no chunks are created.
    /// When `chunk_size > total_size`, exactly one chunk `[0, total_size - 1]` is created.
    pub fn new(total_size: u64, chunk_size: usize) -> Self {
        let effective_chunk = chunk_size.max(1) as u64;
        let mut chunks = Vec::new();

        if total_size > 0 {
            let mut start = 0u64;
            while start < total_size {
                let chunk_len = (total_size - start).min(effective_chunk);
                let end_inclusive = start + chunk_len - 1;
                chunks.push(TableEntry {
                    start,
                    end_inclusive,
                    owner: None,
                    lease_expires_at: None,
                    status: ChunkStatus::Unclaimed,
                    generation: 0,
                    attempts: 0,
                });
                start += chunk_len;
            }
        }

        Self {
            total_size,
            chunk_size,
            chunks,
            completed_count: 0,
            steals_performed: 0,
            retries: 0,
            completed_ranges: Vec::new(),
        }
    }

    /// Creates a new `LeaseTable` using [`DEFAULT_CHUNK_SIZE`] (4 MiB).
    pub fn with_default_chunk_size(total_size: u64) -> Self {
        Self::new(total_size, DEFAULT_CHUNK_SIZE)
    }

    /// Total size in bytes.
    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Configured chunk size in bytes.
    #[inline]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Total number of chunks in the table.
    #[inline]
    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Number of completed chunks.
    #[inline]
    pub fn completed_chunks(&self) -> usize {
        self.completed_count
    }

    /// Number of successful steals performed.
    #[inline]
    pub fn steals_performed(&self) -> usize {
        self.steals_performed
    }

    /// Number of retries / releases.
    #[inline]
    pub fn retries(&self) -> usize {
        self.retries
    }

    /// Returns `true` if all chunks have completed.
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.completed_count == self.chunks.len()
    }

    /// Returns the number of currently active leases.
    pub fn active_leases_count(&self) -> usize {
        self.chunks
            .iter()
            .filter(|c| c.status == ChunkStatus::Leased)
            .count()
    }

    /// Returns the number of unclaimed chunks.
    pub fn unclaimed_count(&self) -> usize {
        self.chunks
            .iter()
            .filter(|c| c.status == ChunkStatus::Unclaimed)
            .count()
    }

    /// Claims the next available range lease.
    ///
    /// Priority:
    /// 1. Lowest unclaimed chunk (lowest index).
    /// 2. If no chunk is unclaimed, steals a chunk whose lease has expired
    ///    (longest-expired first, i.e. earliest `lease_expires_at`).
    /// 3. Returns `None` if no chunk is unclaimed or eligible to be stolen.
    ///
    /// Never returns a chunk held by another live unexpired worker, and never returns
    /// the same chunk to two workers at the same instant.
    pub fn claim_next(
        &mut self,
        worker: WorkerId,
        now: Instant,
        ttl: Duration,
    ) -> Option<RangeLease> {
        // 1. Find lowest unclaimed chunk
        for entry in self.chunks.iter_mut() {
            if entry.status == ChunkStatus::Unclaimed {
                let expires_at = now + ttl;
                entry.owner = Some(worker);
                entry.lease_expires_at = Some(expires_at);
                entry.status = ChunkStatus::Leased;
                entry.generation += 1;
                entry.attempts += 1;

                return Some(RangeLease {
                    start: entry.start,
                    end_inclusive: entry.end_inclusive,
                    owner: Some(worker),
                    lease_expires_at: expires_at,
                });
            }
        }

        // 2. Try to steal an expired lease (longest-expired first)
        let mut best_idx: Option<usize> = None;
        let mut earliest_expiry: Option<Instant> = None;

        for (idx, entry) in self.chunks.iter().enumerate() {
            if entry.status == ChunkStatus::Leased {
                if let Some(expiry) = entry.lease_expires_at {
                    if expiry <= now {
                        match earliest_expiry {
                            None => {
                                earliest_expiry = Some(expiry);
                                best_idx = Some(idx);
                            }
                            Some(cur_earliest) => {
                                if expiry < cur_earliest {
                                    earliest_expiry = Some(expiry);
                                    best_idx = Some(idx);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(idx) = best_idx {
            let entry = &mut self.chunks[idx];
            let expires_at = now + ttl;
            entry.owner = Some(worker);
            entry.lease_expires_at = Some(expires_at);
            entry.generation += 1;
            entry.attempts += 1;
            self.steals_performed += 1;

            return Some(RangeLease {
                start: entry.start,
                end_inclusive: entry.end_inclusive,
                owner: Some(worker),
                lease_expires_at: expires_at,
            });
        }

        None
    }

    /// Marks a chunk as completed.
    ///
    /// Returns `true` if completion was accepted, or `false` if the lease was stale
    /// (e.g. stolen by another worker or already completed).
    pub fn complete(&mut self, chunk: impl AsRef<RangeLease>) -> bool {
        let chunk = chunk.as_ref();
        for entry in self.chunks.iter_mut() {
            if entry.start == chunk.start && entry.end_inclusive == chunk.end_inclusive {
                if entry.status == ChunkStatus::Completed {
                    return false;
                }
                if entry.status != ChunkStatus::Leased {
                    return false;
                }
                // Check that caller is still the active owner and expiration matches
                if entry.owner != chunk.owner
                    || entry.lease_expires_at != Some(chunk.lease_expires_at)
                {
                    return false;
                }

                entry.status = ChunkStatus::Completed;
                entry.owner = None;
                entry.lease_expires_at = None;
                self.completed_count += 1;
                self.completed_ranges
                    .push((entry.start, entry.end_inclusive));
                return true;
            }
        }
        false
    }

    /// Releases a chunk lease so it becomes claimable again (e.g. on worker cancel or error).
    ///
    /// Returns `true` if the lease was released, or `false` if the lease was stale.
    pub fn release(&mut self, chunk: impl AsRef<RangeLease>) -> bool {
        let chunk = chunk.as_ref();
        for entry in self.chunks.iter_mut() {
            if entry.start == chunk.start && entry.end_inclusive == chunk.end_inclusive {
                if entry.status == ChunkStatus::Completed {
                    return false;
                }
                if entry.status != ChunkStatus::Leased {
                    return false;
                }
                if entry.owner != chunk.owner
                    || entry.lease_expires_at != Some(chunk.lease_expires_at)
                {
                    return false;
                }

                entry.status = ChunkStatus::Unclaimed;
                entry.owner = None;
                entry.lease_expires_at = None;
                self.retries += 1;
                return true;
            }
        }
        false
    }

    /// Releases all active leases currently held by a given worker.
    pub fn release_worker_leases(&mut self, worker: WorkerId) -> usize {
        let mut count = 0;
        for entry in self.chunks.iter_mut() {
            if entry.status == ChunkStatus::Leased && entry.owner == Some(worker) {
                entry.status = ChunkStatus::Unclaimed;
                entry.owner = None;
                entry.lease_expires_at = None;
                self.retries += 1;
                count += 1;
            }
        }
        count
    }

    /// Releases all active leases in the table (e.g. on overall task cancellation).
    pub fn release_all_active(&mut self) -> usize {
        let mut count = 0;
        for entry in self.chunks.iter_mut() {
            if entry.status == ChunkStatus::Leased {
                entry.status = ChunkStatus::Unclaimed;
                entry.owner = None;
                entry.lease_expires_at = None;
                self.retries += 1;
                count += 1;
            }
        }
        count
    }

    /// Produces a [`LeaseReport`] snapshot of the table's state.
    pub fn report(&self) -> LeaseReport {
        LeaseReport::new(
            self.chunks.len(),
            self.completed_count,
            self.steals_performed,
            self.retries,
            self.completed_ranges.clone(),
            self.total_size,
        )
    }

    /// Returns all completed ranges as a slice of `(start, end_inclusive)`.
    pub fn completed_ranges(&self) -> &[(u64, u64)] {
        &self.completed_ranges
    }

    /// Marks specific byte ranges as already completed, for resume support.
    ///
    /// This is used when recovering from a checkpoint that has already downloaded some ranges.
    /// The ranges should be non-overlapping and within [0, total_size).
    pub fn mark_completed_ranges(&mut self, ranges: &[(u64, u64)]) {
        for &(start, end_inclusive) in ranges {
            for entry in self.chunks.iter_mut() {
                if entry.start == start && entry.end_inclusive == end_inclusive {
                    if entry.status == ChunkStatus::Unclaimed {
                        entry.status = ChunkStatus::Completed;
                        self.completed_count += 1;
                        self.completed_ranges.push((start, end_inclusive));
                    }
                    break;
                }
            }
        }
    }
}
