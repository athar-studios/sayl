//! Core event subsystem, bounded event bus, and sequence sequencing.
//!
//! Provides [`Event`], [`EventKind`], [`EventPayload`], and bounded channel
//! primitives ([`EventSender`], [`EventReceiver`], [`event_channel`]) for
//! publishing download events to UI, storage, or logging sinks without blocking the engine.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::mpsc::{self, error::SendError, error::TryRecvError, error::TrySendError};

use crate::id::TaskId;
use crate::state::DownloadState;

#[cfg(feature = "chrono")]
pub use chrono::{DateTime, Utc};

#[cfg(not(feature = "chrono"))]
/// Marker struct representing the UTC timezone in absence of the `chrono` feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Utc;

#[cfg(not(feature = "chrono"))]
/// Timestamp representation compatible with UTC time points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DateTime<T = Utc> {
    /// Seconds since Unix epoch.
    pub secs: i64,
    /// Nanosecond fraction within the second.
    pub nanos: u32,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: std::marker::PhantomData<T>,
}

#[cfg(not(feature = "chrono"))]
impl DateTime<Utc> {
    /// Returns the current UTC timestamp based on system time.
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            secs: duration.as_secs() as i64,
            nanos: duration.subsec_nanos(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Creates a timestamp from raw seconds and nanoseconds.
    pub const fn from_secs_and_nanos(secs: i64, nanos: u32) -> Self {
        Self {
            secs,
            nanos,
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(not(feature = "chrono"))]
impl fmt::Display for DateTime<Utc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:09}Z", self.secs, self.nanos)
    }
}

/// The eight discrete event kinds emitted during download task progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventKind {
    /// Task transitioned to a new state.
    StateChanged,
    /// Progress update with downloaded bytes and current transfer speed.
    Progress,
    /// Periodic durable checkpoint committed to persistent storage.
    Checkpoint,
    /// A segment was split to allocate work to another connection.
    Split,
    /// Retry attempt initiated after a transient failure.
    Retry,
    /// An error occurred during download execution.
    Error,
    /// Task was explicitly cancelled.
    Cancelled,
    /// Download finished and verified successfully.
    Completed,
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateChanged => write!(f, "StateChanged"),
            Self::Progress => write!(f, "Progress"),
            Self::Checkpoint => write!(f, "Checkpoint"),
            Self::Split => write!(f, "Split"),
            Self::Retry => write!(f, "Retry"),
            Self::Error => write!(f, "Error"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Completed => write!(f, "Completed"),
        }
    }
}

/// Strongly typed payload corresponding to an [`EventKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventPayload {
    /// State change detail.
    StateChanged {
        /// Previous state.
        from: DownloadState,
        /// New destination state.
        to: DownloadState,
    },
    /// Live throughput and transfer progress.
    Progress {
        /// Total bytes downloaded so far across all segments.
        downloaded_bytes: u64,
        /// Expected content length in bytes, if known.
        total_bytes: Option<u64>,
        /// Instantaneous transfer rate in bytes per second.
        speed_bps: u64,
    },
    /// Durable disk and SQLite sync barrier event.
    Checkpoint {
        /// Bytes synced and flushed durably to disk.
        durable_bytes: u64,
        /// Total expected bytes.
        total_bytes: Option<u64>,
    },
    /// Subrequest tail split event for worker work-stealing.
    Split {
        /// Parent segment that yielded work.
        parent_segment_id: u32,
        /// Newly generated child segment identifier.
        child_segment_id: u32,
        /// Start offset (inclusive) of the newly allocated range.
        range_start: u64,
        /// End offset (exclusive) of the newly allocated range.
        range_end: u64,
    },
    /// Backoff retry trigger.
    Retry {
        /// Current retry attempt number (1-based).
        attempt: u32,
        /// Configured maximum attempts.
        max_attempts: u32,
        /// Scheduled delay before next attempt in milliseconds.
        delay_ms: u64,
        /// Failure reason prompting the retry.
        reason: String,
    },
    /// Failure or error description.
    Error {
        /// Description of the error.
        error: String,
        /// Whether the error was classified as retryable.
        retryable: bool,
    },
    /// Task cancellation confirmation.
    Cancelled {
        /// Cause of cancellation.
        reason: String,
    },
    /// Task completion summary.
    Completed {
        /// Final verified content size in bytes.
        total_bytes: u64,
        /// Total elapsed duration in milliseconds.
        duration_ms: u64,
    },
    /// Empty payload for events without associated parameters.
    Empty,
}

/// Core event record emitted by the download engine.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event {
    /// Monotonically increasing sequence number across all emitted events.
    pub id: u64,
    /// Task identifier associated with this event.
    pub task_id: TaskId,
    /// Event category.
    pub kind: EventKind,
    /// Timestamp when the event was produced.
    pub timestamp: DateTime<Utc>,
    /// Associated payload data.
    pub payload: EventPayload,
}

impl Event {
    /// Creates a new event record with the specified sequence number and timestamp.
    pub fn new(
        id: u64,
        task_id: TaskId,
        kind: EventKind,
        timestamp: DateTime<Utc>,
        payload: EventPayload,
    ) -> Self {
        Self {
            id,
            task_id,
            kind,
            timestamp,
            payload,
        }
    }
}

/// Sending handle for emitting sequential events into a bounded channel.
#[derive(Debug, Clone)]
pub struct EventSender {
    sender: mpsc::Sender<Event>,
    sequence_counter: Arc<AtomicU64>,
}

impl EventSender {
    /// Creates a new sender wrapping an mpsc sender and shared sequence counter.
    pub fn new(sender: mpsc::Sender<Event>, sequence_counter: Arc<AtomicU64>) -> Self {
        Self {
            sender,
            sequence_counter,
        }
    }

    /// Emits an event with the next monotonic sequence number, waiting if the channel is full.
    pub async fn emit(
        &self,
        task_id: TaskId,
        kind: EventKind,
        payload: EventPayload,
    ) -> Result<Event, SendError<Event>> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        let event = Event::new(seq, task_id, kind, DateTime::now(), payload);
        self.sender.send(event.clone()).await.map(|_| event)
    }

    /// Attempts to emit an event without waiting, failing if channel capacity is exhausted.
    pub fn try_emit(
        &self,
        task_id: TaskId,
        kind: EventKind,
        payload: EventPayload,
    ) -> Result<Event, TrySendError<Event>> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        let event = Event::new(seq, task_id, kind, DateTime::now(), payload);
        self.sender.try_send(event.clone()).map(|_| event)
    }

    /// Returns the current value of the sequence counter.
    pub fn current_sequence(&self) -> u64 {
        self.sequence_counter.load(Ordering::SeqCst)
    }
}

/// Receiving handle for reading events from the core event bus.
#[derive(Debug)]
pub struct EventReceiver {
    receiver: mpsc::Receiver<Event>,
    last_sequence: Option<u64>,
}

impl EventReceiver {
    /// Creates a new receiver.
    pub fn new(receiver: mpsc::Receiver<Event>) -> Self {
        Self {
            receiver,
            last_sequence: None,
        }
    }

    /// Asynchronously waits for and returns the next event from the channel.
    pub async fn recv(&mut self) -> Option<Event> {
        let event = self.receiver.recv().await?;
        self.last_sequence = Some(event.id);
        Some(event)
    }

    /// Attempts to read the next event without waiting.
    pub fn try_recv(&mut self) -> Result<Event, TryRecvError> {
        let event = self.receiver.try_recv()?;
        self.last_sequence = Some(event.id);
        Ok(event)
    }

    /// Returns the last sequence number observed by this receiver.
    pub fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }
}

/// Gap detector used by consumers to verify sequence continuity.
#[derive(Debug, Default, Clone)]
pub struct SequenceGapDetector {
    expected_sequence: Option<u64>,
}

impl SequenceGapDetector {
    /// Creates a new gap detector with no initial sequence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the observation of an event sequence number.
    ///
    /// Returns `Some((expected, actual))` if a sequence gap is detected.
    pub fn record(&mut self, seq: u64) -> Option<(u64, u64)> {
        match self.expected_sequence {
            None => {
                self.expected_sequence = Some(seq + 1);
                None
            }
            Some(expected) => {
                self.expected_sequence = Some(seq + 1);
                if seq != expected {
                    Some((expected, seq))
                } else {
                    None
                }
            }
        }
    }
}

/// Creates a bounded event delivery channel and atomic sequence counter.
///
/// Returns an [`EventSender`] and [`EventReceiver`] pair with the specified buffer capacity.
pub fn event_channel(capacity: usize) -> (EventSender, EventReceiver) {
    let (tx, rx) = mpsc::channel(capacity);
    let counter = Arc::new(AtomicU64::new(1));
    (
        EventSender::new(tx, counter),
        EventReceiver::new(rx),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_channel_send_and_receive() {
        let (sender, mut receiver) = event_channel(16);
        let task_id = TaskId::new();

        let event = sender
            .try_emit(
                task_id,
                EventKind::StateChanged,
                EventPayload::StateChanged {
                    from: DownloadState::Queued,
                    to: DownloadState::Probing,
                },
            )
            .expect("try_emit should succeed");

        assert_eq!(event.id, 1);
        assert_eq!(event.task_id, task_id);
        assert_eq!(event.kind, EventKind::StateChanged);

        let received = receiver.try_recv().expect("try_recv should succeed");
        assert_eq!(received.id, 1);
        assert_eq!(received.task_id, task_id);
        assert_eq!(receiver.last_sequence(), Some(1));
    }

    #[test]
    fn test_sequence_monotonicity() {
        let (sender, mut receiver) = event_channel(10);
        let task_id = TaskId::new();

        for i in 1..=5 {
            let ev = sender
                .try_emit(
                    task_id,
                    EventKind::Progress,
                    EventPayload::Progress {
                        downloaded_bytes: i * 1024,
                        total_bytes: Some(10240),
                        speed_bps: 512,
                    },
                )
                .expect("try_emit should succeed");
            assert_eq!(ev.id, i);
        }

        for i in 1..=5 {
            let received = receiver.try_recv().expect("try_recv should succeed");
            assert_eq!(received.id, i);
        }
    }

    #[test]
    fn test_gap_detector() {
        let mut detector = SequenceGapDetector::new();

        assert_eq!(detector.record(1), None);
        assert_eq!(detector.record(2), None);
        assert_eq!(detector.record(3), None);

        // Gap: skip 4, arrive at 5
        let gap = detector.record(5);
        assert_eq!(gap, Some((4, 5)));

        // Continue from 6
        assert_eq!(detector.record(6), None);
    }

    #[test]
    fn test_all_event_payloads() {
        let task_id = TaskId::new();

        let payloads = vec![
            (
                EventKind::StateChanged,
                EventPayload::StateChanged {
                    from: DownloadState::Downloading,
                    to: DownloadState::Verifying,
                },
            ),
            (
                EventKind::Progress,
                EventPayload::Progress {
                    downloaded_bytes: 500,
                    total_bytes: Some(1000),
                    speed_bps: 250,
                },
            ),
            (
                EventKind::Checkpoint,
                EventPayload::Checkpoint {
                    durable_bytes: 500,
                    total_bytes: Some(1000),
                },
            ),
            (
                EventKind::Split,
                EventPayload::Split {
                    parent_segment_id: 1,
                    child_segment_id: 2,
                    range_start: 500,
                    range_end: 1000,
                },
            ),
            (
                EventKind::Retry,
                EventPayload::Retry {
                    attempt: 1,
                    max_attempts: 5,
                    delay_ms: 2000,
                    reason: "timeout".into(),
                },
            ),
            (
                EventKind::Error,
                EventPayload::Error {
                    error: "404 Not Found".into(),
                    retryable: false,
                },
            ),
            (
                EventKind::Cancelled,
                EventPayload::Cancelled {
                    reason: "user".into(),
                },
            ),
            (
                EventKind::Completed,
                EventPayload::Completed {
                    total_bytes: 1000,
                    duration_ms: 4000,
                },
            ),
        ];

        for (i, (kind, payload)) in payloads.into_iter().enumerate() {
            let event = Event::new(i as u64 + 1, task_id, kind, DateTime::now(), payload);
            assert_eq!(event.kind, kind);
        }
    }
}
