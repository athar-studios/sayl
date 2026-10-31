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
//! - [`client`]: Centralized [`ClientConfig`] and [`build_client`] for connection pooling, proxy, timeouts, and redirect policy.
//! - [`headers`]: Header injection defence, RFC 9110 token validation, and safe request header construction.
//! - [`budget`]: [`OriginBudget`] for per-origin concurrency and rate limiting with testable time source.

pub mod budget;
pub mod client;
pub mod error;
pub mod filename;
pub mod headers;
pub mod probe;
pub mod range;
pub mod redirect;
pub mod resume;
pub mod retry;
pub mod stream;

pub use budget::{
    origin_from_url, ManualTimeHandle, ManualTimeSource, OriginBudget, SystemTimeSource, TimeSource,
};
pub use client::{
    build_client, user_agent_default, ClientConfig, ProxyConfig, ProxyMode,
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_CONNECTIONS_PER_HOST, DEFAULT_MAX_IDLE_PER_HOST,
    DEFAULT_MAX_RESPONSE_HEADERS_BYTES, DEFAULT_POOL_IDLE_TIMEOUT, DEFAULT_READ_TIMEOUT,
};
pub use error::HttpError;
pub use filename::{
    derive_filename, extract_filename_from_url, generate_unique_filename, next_numbered_filename,
    parse_content_disposition, sanitize_filename, split_stem_extension, DEFAULT_FALLBACK_FILENAME,
    MAX_FILENAME_LENGTH,
};
pub use headers::{
    build_request_headers, validate_header_name, validate_header_value,
    validate_no_header_injection, MAX_HEADER_VALUE_LEN,
};
pub use probe::{parse_content_length, probe, probe_with_options, ProbeOptions, ProbeResult};
pub use range::{format_byte_range, verify_range_probe_response, ContentRange, RangeSupport};
pub use redirect::{RedirectPolicy, RedirectTracker, DEFAULT_MAX_REDIRECT_HOPS};
pub use resume::{
    if_range_value, is_weak_etag, reconcile_partial_len, validate_resume, validator_kind,
    ResumeDecision, ResumeInfo, ValidatorKind,
};
pub use retry::{is_retryable_status, should_retry, status_of, RetryPolicy};
pub use stream::{
    stream_to_file, stream_to_file_with_options, CancellationToken, StreamOptions, StreamOutcome,
    DEFAULT_CHUNK_SIZE,
};
