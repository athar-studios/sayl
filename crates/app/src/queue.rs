//! Queue limits, per-origin scheduling, and bandwidth throttling.
//!
//! The engine used to reject work as soon as a limit was reached. This module
//! provides the missing scheduler: tasks wait in a FIFO queue, are promoted only
//! when a global slot *and* a per-origin connection permit are free, and are
//! throttled by a token bucket that never reads a global clock.
//!
//! ```
//! use sayl_app::queue::{QueueLimits, Scheduler, TaskStatus};
//! use std::time::Instant;
//!
//! let mut scheduler = Scheduler::new(QueueLimits::default());
//! scheduler.enqueue(1).unwrap();
//! scheduler.set_origin(1, "https://example.com").unwrap();
//!
//! let started = scheduler.poll(Instant::now());
//! assert_eq!(started, vec![1]);
//! assert_eq!(scheduler.status(1), Some(TaskStatus::Running));
//! ```

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::{Duration, Instant};

/// Limits that govern how many tasks and connections may run at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueLimits {
    /// Maximum number of tasks downloading at the same time.
    pub max_concurrent_tasks: usize,
    /// Maximum connections a single task may use.
    pub max_connections_per_task: usize,
    /// Maximum concurrent connections to one origin.
    pub max_connections_per_origin: usize,
    /// Maximum number of tasks allowed to wait in the queue.
    pub max_queue_depth: usize,
    /// Optional global bandwidth cap in bytes per second.
    pub max_bandwidth_bytes_per_sec: Option<u64>,
    /// Maximum retries allowed for a single task.
    pub max_retries_per_task: u32,
}

impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 3,
            max_connections_per_task: 4,
            max_connections_per_origin: 4,
            max_queue_depth: 256,
            max_bandwidth_bytes_per_sec: None,
            max_retries_per_task: 5,
        }
    }
}

/// Reasons a scheduler call can be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// The waiting queue already holds `max_queue_depth` tasks.
    QueueFull,
    /// The task id is already known to the scheduler.
    AlreadyQueued,
    /// The task id was never enqueued.
    UnknownTask,
    /// Every retry allowed for this task has been used.
    RetryBudgetExhausted,
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => write!(f, "queue is full"),
            Self::AlreadyQueued => write!(f, "task is already queued"),
            Self::UnknownTask => write!(f, "unknown task"),
            Self::RetryBudgetExhausted => write!(f, "retry budget exhausted"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Lifecycle position of a task inside the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Waiting for a slot.
    Queued,
    /// Waiting specifically because its origin is saturated.
    WaitingForOrigin,
    /// Holding a slot and an origin permit.
    Running,
    /// Completed successfully.
    Finished,
    /// Ended in failure.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl TaskStatus {
    /// Terminal states never release permits twice.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Finished | Self::Failed | Self::Cancelled)
    }
}

/// Snapshot of scheduler occupancy, for the UI and CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueStats {
    /// Tasks waiting for a slot.
    pub queued: usize,
    /// Tasks currently downloading.
    pub running: usize,
    /// Tasks blocked only by per-origin saturation.
    pub waiting_for_origin: usize,
    /// Live connection count per origin, sorted by origin.
    pub per_origin_inflight: Vec<(String, usize)>,
}

#[derive(Debug)]
struct TaskEntry {
    status: TaskStatus,
    origin: String,
    retries_left: u32,
}

/// FIFO, origin-aware task scheduler.
#[derive(Debug)]
pub struct Scheduler {
    limits: QueueLimits,
    tasks: HashMap<u64, TaskEntry>,
    /// Waiting task ids in FIFO order.
    waiting: Vec<u64>,
    running: HashSet<u64>,
    origin_inflight: HashMap<String, usize>,
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new(limits: QueueLimits) -> Self {
        Self {
            limits,
            tasks: HashMap::new(),
            waiting: Vec::new(),
            running: HashSet::new(),
            origin_inflight: HashMap::new(),
        }
    }

    /// Returns the configured limits.
    pub fn limits(&self) -> &QueueLimits {
        &self.limits
    }

    /// Queues a task. The queue depth counts waiting tasks only.
    pub fn enqueue(&mut self, task: u64) -> Result<(), QueueError> {
        if self.tasks.contains_key(&task) {
            return Err(QueueError::AlreadyQueued);
        }
        if self.waiting.len() >= self.limits.max_queue_depth {
            return Err(QueueError::QueueFull);
        }
        self.tasks.insert(
            task,
            TaskEntry {
                status: TaskStatus::Queued,
                origin: String::new(),
                retries_left: self.limits.max_retries_per_task,
            },
        );
        self.waiting.push(task);
        Ok(())
    }

    /// Attaches an origin to a task so per-origin limits can apply.
    pub fn set_origin(&mut self, task: u64, origin: &str) -> Result<(), QueueError> {
        let entry = self.tasks.get_mut(&task).ok_or(QueueError::UnknownTask)?;
        entry.origin = origin.to_string();
        Ok(())
    }

    /// Promotes waiting tasks into free slots.
    ///
    /// Tasks are considered in FIFO order. A task whose origin is saturated keeps
    /// its place and is marked [`TaskStatus::WaitingForOrigin`]; later tasks from
    /// other origins may still start, so one busy host cannot stall the queue.
    pub fn poll(&mut self, _now: Instant) -> Vec<u64> {
        let mut started = Vec::new();
        if self.running.len() >= self.limits.max_concurrent_tasks {
            return started;
        }

        let mut index = 0;
        while index < self.waiting.len() {
            if self.running.len() >= self.limits.max_concurrent_tasks {
                break;
            }
            let task = self.waiting[index];
            let origin = match self.tasks.get(&task) {
                Some(entry) => entry.origin.clone(),
                None => {
                    self.waiting.remove(index);
                    continue;
                }
            };
            let inflight = self.origin_inflight.get(&origin).copied().unwrap_or(0);
            if inflight >= self.limits.max_connections_per_origin {
                if let Some(entry) = self.tasks.get_mut(&task) {
                    entry.status = TaskStatus::WaitingForOrigin;
                }
                index += 1;
                continue;
            }

            self.waiting.remove(index);
            *self.origin_inflight.entry(origin).or_insert(0) += 1;
            self.running.insert(task);
            if let Some(entry) = self.tasks.get_mut(&task) {
                entry.status = TaskStatus::Running;
            }
            started.push(task);
        }
        started
    }

    /// Releases a task's slot and origin permit. Idempotent for terminal tasks.
    pub fn finish(&mut self, task: u64, outcome: TaskStatus) -> Result<(), QueueError> {
        let entry = match self.tasks.get(&task) {
            Some(entry) => entry,
            None => return Err(QueueError::UnknownTask),
        };
        match entry.status {
            TaskStatus::Queued | TaskStatus::WaitingForOrigin => {
                if outcome.is_terminal() {
                    self.waiting.retain(|id| *id != task);
                    if let Some(entry) = self.tasks.get_mut(&task) {
                        entry.status = outcome;
                    }
                }
                Ok(())
            }
            TaskStatus::Running => {
                self.release_slot(task);
                if let Some(entry) = self.tasks.get_mut(&task) {
                    entry.status = if outcome.is_terminal() {
                        outcome
                    } else {
                        TaskStatus::Queued
                    };
                    if !outcome.is_terminal() {
                        self.waiting.push(task);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Cancels a task whether it is waiting or running.
    pub fn cancel(&mut self, task: u64) -> Result<(), QueueError> {
        let status = self.tasks.get(&task).ok_or(QueueError::UnknownTask)?.status;
        if status.is_terminal() {
            return Ok(());
        }
        if status == TaskStatus::Running {
            self.release_slot(task);
        } else {
            self.waiting.retain(|id| *id != task);
        }
        if let Some(entry) = self.tasks.get_mut(&task) {
            entry.status = TaskStatus::Cancelled;
        }
        Ok(())
    }

    fn release_slot(&mut self, task: u64) {
        if !self.running.remove(&task) {
            return;
        }
        let origin = self
            .tasks
            .get(&task)
            .map(|entry| entry.origin.clone())
            .unwrap_or_default();
        if let Some(count) = self.origin_inflight.get_mut(&origin) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.origin_inflight.remove(&origin);
            }
        }
    }

    /// Current status of a task.
    pub fn status(&self, task: u64) -> Option<TaskStatus> {
        self.tasks.get(&task).map(|entry| entry.status)
    }

    /// Occupancy snapshot.
    pub fn stats(&self) -> QueueStats {
        let waiting_for_origin = self
            .tasks
            .values()
            .filter(|entry| entry.status == TaskStatus::WaitingForOrigin)
            .count();
        let mut per_origin_inflight: Vec<(String, usize)> = self
            .origin_inflight
            .iter()
            .map(|(origin, count)| (origin.clone(), *count))
            .collect();
        per_origin_inflight.sort_by(|a, b| a.0.cmp(&b.0));
        QueueStats {
            queued: self.waiting.len(),
            running: self.running.len(),
            waiting_for_origin,
            per_origin_inflight,
        }
    }

    /// Retries still available to a task.
    pub fn retries_left(&self, task: u64) -> u32 {
        self.tasks
            .get(&task)
            .map(|entry| entry.retries_left)
            .unwrap_or(0)
    }

    /// Records one retry, returning the remaining budget.
    pub fn record_retry(&mut self, task: u64) -> Result<u32, QueueError> {
        let entry = self.tasks.get_mut(&task).ok_or(QueueError::UnknownTask)?;
        if entry.retries_left == 0 {
            return Err(QueueError::RetryBudgetExhausted);
        }
        entry.retries_left -= 1;
        Ok(entry.retries_left)
    }
}

/// Token bucket that throttles writes to a byte-per-second rate.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    rate: u64,
    capacity: f64,
    tokens: f64,
    last: Option<Instant>,
}

impl TokenBucket {
    /// Creates a bucket that refills at `bytes_per_sec`, with a one-second burst.
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            rate: bytes_per_sec.max(1),
            capacity: bytes_per_sec.max(1) as f64,
            tokens: bytes_per_sec.max(1) as f64,
            last: None,
        }
    }

    /// Returns how long the caller must wait before writing `bytes`.
    pub fn take(&mut self, bytes: u64, now: Instant) -> Duration {
        if let Some(last) = self.last {
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            self.tokens = (self.tokens + elapsed * self.rate as f64).min(self.capacity);
        }
        self.last = Some(now);

        let request = bytes as f64;
        if self.tokens >= request {
            self.tokens -= request;
            return Duration::ZERO;
        }
        let deficit = request - self.tokens;
        self.tokens = 0.0;
        Duration::from_secs_f64(deficit / self.rate as f64)
    }

    /// Configured rate in bytes per second.
    pub fn rate(&self) -> u64 {
        self.rate
    }

    /// Changes the refill rate in bytes per second, keeping the one-second burst.
    ///
    /// Used when the user edits a live task's speed limit: the next `take` call
    /// already applies the new rate.
    pub fn set_rate(&mut self, bytes_per_sec: u64) {
        let rate = bytes_per_sec.max(1);
        self.rate = rate;
        self.capacity = rate as f64;
        self.tokens = self.tokens.min(self.capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduler(limits: QueueLimits) -> Scheduler {
        Scheduler::new(limits)
    }

    #[test]
    fn default_limits_match_the_documented_values() {
        let limits = QueueLimits::default();
        assert_eq!(limits.max_concurrent_tasks, 3);
        assert_eq!(limits.max_connections_per_task, 4);
        assert_eq!(limits.max_connections_per_origin, 4);
        assert_eq!(limits.max_queue_depth, 256);
        assert_eq!(limits.max_bandwidth_bytes_per_sec, None);
        assert_eq!(limits.max_retries_per_task, 5);
    }

    #[test]
    fn queue_depth_limit_is_enforced() {
        let limits = QueueLimits {
            max_queue_depth: 2,
            ..QueueLimits::default()
        };
        let mut s = scheduler(limits);
        s.enqueue(1).unwrap();
        s.enqueue(2).unwrap();
        assert_eq!(s.enqueue(3), Err(QueueError::QueueFull));
        assert_eq!(s.stats().queued, 2);
    }

    #[test]
    fn duplicate_enqueue_is_rejected() {
        let mut s = scheduler(QueueLimits::default());
        s.enqueue(1).unwrap();
        assert_eq!(s.enqueue(1), Err(QueueError::AlreadyQueued));
    }

    #[test]
    fn promotion_is_fifo_and_respects_the_global_cap() {
        let limits = QueueLimits {
            max_concurrent_tasks: 2,
            ..QueueLimits::default()
        };
        let mut s = scheduler(limits);
        for id in 1..=4 {
            s.enqueue(id).unwrap();
            s.set_origin(id, "https://a.example").unwrap();
        }
        let now = Instant::now();
        assert_eq!(s.poll(now), vec![1, 2]);
        assert_eq!(s.stats().running, 2);
        assert_eq!(s.stats().queued, 2);

        s.finish(1, TaskStatus::Finished).unwrap();
        assert_eq!(s.poll(now), vec![3]);
        assert_eq!(s.status(3), Some(TaskStatus::Running));
    }

    #[test]
    fn per_origin_cap_marks_tasks_as_waiting_for_origin() {
        let limits = QueueLimits {
            max_concurrent_tasks: 10,
            max_connections_per_origin: 1,
            ..QueueLimits::default()
        };
        let mut s = scheduler(limits);
        s.enqueue(1).unwrap();
        s.set_origin(1, "https://a.example").unwrap();
        s.enqueue(2).unwrap();
        s.set_origin(2, "https://a.example").unwrap();

        let now = Instant::now();
        assert_eq!(s.poll(now), vec![1]);
        assert_eq!(s.status(2), Some(TaskStatus::WaitingForOrigin));
        assert_eq!(s.stats().waiting_for_origin, 1);

        // The saturated origin must not block a different host.
        s.enqueue(3).unwrap();
        s.set_origin(3, "https://b.example").unwrap();
        assert_eq!(s.poll(now), vec![3]);

        s.finish(1, TaskStatus::Finished).unwrap();
        assert_eq!(s.poll(now), vec![2]);
        assert_eq!(s.status(2), Some(TaskStatus::Running));
    }

    #[test]
    fn cancel_works_for_waiting_and_running_tasks() {
        let limits = QueueLimits {
            max_concurrent_tasks: 1,
            ..QueueLimits::default()
        };
        let mut s = scheduler(limits);
        s.enqueue(1).unwrap();
        s.set_origin(1, "https://a.example").unwrap();
        s.enqueue(2).unwrap();
        s.set_origin(2, "https://a.example").unwrap();
        let now = Instant::now();
        assert_eq!(s.poll(now), vec![1]);

        s.cancel(2).unwrap();
        assert_eq!(s.status(2), Some(TaskStatus::Cancelled));
        assert_eq!(s.stats().queued, 0);

        s.cancel(1).unwrap();
        assert_eq!(s.status(1), Some(TaskStatus::Cancelled));
        assert_eq!(s.stats().running, 0);
        assert!(s.stats().per_origin_inflight.is_empty());

        // Cancelling twice is a no-op.
        s.cancel(1).unwrap();
        assert_eq!(s.cancel(99), Err(QueueError::UnknownTask));
    }

    #[test]
    fn finish_releases_slots_once_and_is_idempotent() {
        let mut s = scheduler(QueueLimits::default());
        s.enqueue(1).unwrap();
        s.set_origin(1, "https://a.example").unwrap();
        let now = Instant::now();
        assert_eq!(s.poll(now), vec![1]);

        s.finish(1, TaskStatus::Finished).unwrap();
        assert_eq!(s.stats().running, 0);
        assert!(s.stats().per_origin_inflight.is_empty());

        // A second finish must not decrement another origin's counter.
        s.finish(1, TaskStatus::Failed).unwrap();
        assert_eq!(s.stats().running, 0);
        assert_eq!(
            s.finish(42, TaskStatus::Finished),
            Err(QueueError::UnknownTask)
        );
    }

    #[test]
    fn stats_reflect_occupancy_across_the_lifecycle() {
        let mut s = scheduler(QueueLimits::default());
        assert_eq!(s.stats().queued, 0);
        s.enqueue(1).unwrap();
        s.enqueue(2).unwrap();
        s.set_origin(1, "https://a.example").unwrap();
        s.set_origin(2, "https://b.example").unwrap();
        assert_eq!(s.stats().queued, 2);
        assert_eq!(s.stats().running, 0);

        let started = s.poll(Instant::now());
        assert_eq!(started.len(), 2);
        let stats = s.stats();
        assert_eq!(stats.running, 2);
        assert_eq!(stats.queued, 0);
        assert_eq!(stats.per_origin_inflight.len(), 2);

        s.finish(1, TaskStatus::Finished).unwrap();
        s.finish(2, TaskStatus::Finished).unwrap();
        assert_eq!(s.stats().running, 0);
    }

    #[test]
    fn retry_budget_is_bounded() {
        let limits = QueueLimits {
            max_retries_per_task: 2,
            ..QueueLimits::default()
        };
        let mut s = scheduler(limits);
        s.enqueue(1).unwrap();
        assert_eq!(s.retries_left(1), 2);
        assert_eq!(s.record_retry(1), Ok(1));
        assert_eq!(s.record_retry(1), Ok(0));
        assert_eq!(s.record_retry(1), Err(QueueError::RetryBudgetExhausted));
        assert_eq!(s.retries_left(1), 0);
        assert_eq!(s.record_retry(99), Err(QueueError::UnknownTask));
    }

    #[test]
    fn token_bucket_allows_the_initial_burst() {
        let mut bucket = TokenBucket::new(1024);
        let now = Instant::now();
        assert_eq!(bucket.take(512, now), Duration::ZERO);
        assert_eq!(bucket.take(512, now), Duration::ZERO);
        // Third call exceeds the one-second capacity.
        let wait = bucket.take(512, now);
        assert!(wait > Duration::ZERO, "expected throttling, got {wait:?}");
    }

    #[test]
    fn token_bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(1000);
        let start = Instant::now();
        assert_eq!(bucket.take(1000, start), Duration::ZERO);
        assert!(bucket.take(1000, start) > Duration::ZERO);
        // After a full second the bucket is full again.
        let later = start + Duration::from_secs(1);
        assert_eq!(bucket.take(1000, later), Duration::ZERO);
    }

    #[test]
    fn token_bucket_wait_matches_the_deficit() {
        let mut bucket = TokenBucket::new(1000);
        let start = Instant::now();
        let _ = bucket.take(1000, start);
        let wait = bucket.take(500, start);
        // 500 bytes at 1000 B/s is about half a second.
        assert!(
            wait >= Duration::from_millis(499) && wait <= Duration::from_millis(501),
            "{wait:?}"
        );
    }
}
