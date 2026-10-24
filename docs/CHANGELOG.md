# Changelog

All notable changes to the **SAYL ("سيل")** download manager project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.1.0] - (Windows-only, unreleased / in-progress)

> **Status:** In active development (Windows-only). Not released. Packaging and installers are blocked pending Windows SDK availability.

### Scope Summary
- **Core Engine:** Pure Rust multi-part download engine featuring parallel ranged downloads, validator-safe resume (`ETag` / `If-Range`), crash recovery via append-only journaling (`journal.jsonl`) and atomic snapshot compaction, and concurrency scheduling.
- **Command-Line Interface (`sayl-cli`):** Standalone `sayl` CLI supporting `add`, `start` (with resume capability), `list`, `info`, `pause`, `cancel`, `remove`, `recover`, and live JSON `events` streaming.
- **Tauri Desktop UI (`apps/desktop`):** Tauri 2 + Svelte 5 application supporting link addition, lifecycle controls (pause / resume / cancel), live progress streaming, configuration settings, and native Arabic (RTL) alongside English.
- **Browser Extension:** Chromium / Microsoft Edge Manifest V3 extension integrating with the local application via secure Native Messaging.
- **Packaging & Distribution:** NSIS and WiX MSI installer configurations alongside `SHA256SUMS.txt` manifest generation (packaging currently blocked on the missing Windows SDK environment).
- **Website & Documentation:** Zero-tracking static documentation and download portal at `sayl.site` with governance and usage guides.

### Added

#### CORE-001: Core Domain Engine & State Machine
- Implemented `sayl-core` domain foundation with discrete 11-state lifecycle state machine (`Queued`, `Probing`, `Downloading`, `Verifying`, `Finalizing`, `Completed`, `Pausing`, `Paused`, `Cancelling`, `Cancelled`, `Failed`).
- Enforced strict guarded transition matrix with 33 valid transitions (including 8 idempotent non-terminal self-transitions) and 88 rejected transitions out of 121 combinations.
- Implemented bounded event channel with `EventKind` types and `SequenceGapDetector` for network and UI synchronization.
- Added time-ordered `TaskId` (UUID v7) and TTL-governed `IdempotencyStore`.
- Verified with 33 passing unit and invariant tests.

#### TEST-001: Deterministic HTTP Fixture Server
- Implemented `sayl-test-http` in-memory test fixture server binding strictly to `127.0.0.1:0`.
- Simulated all 27 failure modes and protocol edge cases specified in MASTER-PLAN §8 (including `Ok206`, `NoAcceptRanges`, `IgnoredRange200`, `BadContentRange`, `BodyTruncation`, `ConnectionReset`, `ETagChanged`, `WeakETagOnly`, `NoValidator`, `416RangeNotSatisfiable`, `HeadRejected`, `UnknownSize`, `EmptyFile`, `UnexpectedEncoding`, `429RetryAfter`, `RedirectWithSecrets`, `NoSpace`, `PermissionDenied`, `MaliciousFilename`, `PauseCancelDuringProbe`, `SlowWorker`, `OldLease`, `RecoveryAtFinalize`, `ProxyTLS`, `LimitsOrigin`, `FileTruncation`, `Pooling`).
- Implemented deterministic byte generator `BodyGenerator` with pattern verification and request audit log.
- Verified with 25 passing unit and integration tests.

#### STORE-001: Crash-Safe Store & Journaling
- Implemented `sayl-storage` pure Rust durable task and checkpoint store without embedded C/SQLite dependencies.
- Added append-only `journal.jsonl` (WAL) with per-operation flushing and tolerance for truncated/corrupted trailing lines.
- Added atomic snapshot compaction (`snapshot.json.tmp` -> `fsync` -> atomic rename -> truncate journal).
- Enforced single-writer directory locking using Windows OS file share locking (`share_mode(0)`) combined with an in-process thread-safe registry.
- Verified with 18 passing tests covering power-loss simulations and truncated-tail recovery.

#### HTTP-001: Probe, Active Range Verification, Safe Redirects & Filenames
- Implemented `sayl-http` transport client utilizing `native-tls` / Windows Schannel.
- Added active capability probing via `HEAD` with automatic fallback to ranged `GET bytes=0-0`.
- Added strict range capability classification (`Supported`, `Ignored`, `Unsupported`, `Unknown`).
- Implemented safe redirect tracker: 5-hop cap, credential stripping on cross-origin hops, scheme whitelist (`http`/`https`), and redirect loop detection.
- Implemented RFC 5987 / RFC 6266 filename extractor with directory traversal neutralization, Windows reserved device name prefixing (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`), Unicode NFC normalization, and 255 character limit.
- Implemented streaming to disk with 64 KiB chunk buffers, strict byte-length equality verification before success, and mandatory `fsync`.
- Verified with 65 passing integration and unit tests.

#### HTTP-002: Validator-Safe Resume Rules & Retry Policy
- Implemented validator-safe resume logic: strong ETag validation authorizing `If-Range`.
- Enforced HTTP specification rule: weak ETags (`W/"..."`) never authorize `If-Range` requests.
- Added safe fallback to fresh sequential download from offset 0 when validators are untrusted or missing.
- Added exponential backoff retry policy with jitter, honoring HTTP `429` and `Retry-After` headers.

#### APP-001: Download Engine Orchestration
- Implemented `sayl-app` orchestrating storage, transport, and domain state machines.
- Added concurrency regulation via `tokio::sync::Semaphore` (`max_concurrent_tasks`).
- Implemented automatic crash recovery (`recover()`) resetting interrupted active tasks to `Paused`.
- Added cooperative cancellation and pause handling via `CancellationToken`.
- Verified with 9 passing integration tests.

#### SITE-001: Website & Repository Governance
- Built lightweight, zero-tracking static download and documentation website in `apps/website`.
- Established repository standards, contributing guidelines, security policy, and CI workflows.

#### UI-001: Tauri 2 + Svelte Desktop Application
- Built modern desktop frontend in `apps/desktop` using Svelte 5 and TypeScript with native RTL support.
- Implemented Tauri command bridges (`add_task`, `start_task`, `pause_task`, `cancel_task`, `list_tasks`, `get_settings`, `set_settings`, `open_download_folder`, `reveal_task`).
- Implemented live progress stream listener (`sayl://progress`) and domain task event forwarder (`sayl://task`).
- Verified with 21 passing Vitest unit and component tests.
