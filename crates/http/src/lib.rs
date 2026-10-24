//! # sayl-http
//!
//! HTTP transport client, capability probing, safe redirect enforcement,
//! and sequential streaming for the SAYL Download Manager.
//!
//! ## Modules
//! - [`error`]: Comprehensive [`HttpError`] classification (retryable, fatal, cancelled) and mapping to `sayl_core::CoreError`.
//! - [`redirect`]: Strict [`RedirectPolicy`] enforcing a 5-hop cap, credential stripping on cross-origin hops, scheme validation, and loop detection.
//! - [`range`]: [`RangeSupport`] capability verification (`bytes=0-0` subrequests), Content-Range validation, and downgrade rules.
//! - [`filename`]: Safe filename derivation from `Content-Disposition` (RFC 5987 / 6266) and URLs, Windows reserved character/device name sanitization, 255-character cap, and Arabic Unicode NFC preservation.
//! - [`probe`]: Automated resource probing via HEAD with fallback to ranged GET, returning [`ProbeResult`].
//! - [`stream`]: Incremental sequential streaming write to local disk with truncation detection ([`StreamOutcome`]), cooperative cancellation, and durable `fsync`.

pub mod error;
pub mod filename;
pub mod probe;
pub mod range;
pub mod redirect;
pub mod resume;
pub mod retry;
pub mod stream;

pub use error::HttpError;
pub use filename::{
    derive_filename, extract_filename_from_url, parse_content_disposition, sanitize_filename,
    DEFAULT_FALLBACK_FILENAME, MAX_FILENAME_LENGTH,
};
pub use probe::{
    parse_content_length, probe, probe_with_options, ProbeOptions, ProbeResult,
};
pub use range::{format_byte_range, verify_range_probe_response, ContentRange, RangeSupport};
pub use redirect::{
    RedirectPolicy, RedirectTracker, DEFAULT_MAX_REDIRECT_HOPS,
};
pub use resume::{
    if_range_value, is_weak_etag, reconcile_partial_len, validator_kind, validate_resume,
    ResumeDecision, ResumeInfo, ValidatorKind,
};
pub use retry::{
    is_retryable_status, should_retry, status_of, RetryPolicy,
};
pub use stream::{
    stream_to_file, stream_to_file_with_options, CancellationToken, StreamOptions, StreamOutcome,
    DEFAULT_CHUNK_SIZE,
};
