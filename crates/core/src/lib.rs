//! # sayl-core
//!
//! Core domain models, state machine, error taxonomy, event bus, and idempotency
//! contracts for the SAYL Download Manager engine.
//!
//! ## Overview
//! - [`state`]: 11-state download lifecycle state machine ([`DownloadState`]) with guarded [`transition`] logic.
//! - [`error`]: 7-category error taxonomy ([`CoreError`]) with classification for retries, fatal errors, and cancellations.
//! - [`event`]: 8-kind event stream ([`Event`]) backed by bounded mpsc delivery ([`event_channel`]) and sequence numbering.
//! - [`idempotency`]: In-memory deduplication store ([`IdempotencyStore`]) keyed by UUID ([`IdempotencyKey`]) with configurable TTL.
//! - [`id`]: Strongly-typed time-ordered task identifier ([`TaskId`]) based on UUID v7.

pub mod error;
pub mod event;
pub mod id;
pub mod idempotency;
pub mod state;

// Re-export primary contracts at crate root
pub use error::CoreError;
pub use event::{
    event_channel, DateTime, Event, EventKind, EventPayload, EventReceiver, EventSender,
    SequenceGapDetector, Utc,
};
pub use id::TaskId;
pub use idempotency::{
    IdempotencyKey, IdempotencyRecord, IdempotencyStore, DEFAULT_IDEMPOTENCY_TTL,
};
pub use state::{transition, transition_for_task, DownloadState, StateTransitionError};
