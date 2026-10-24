# `sayl-test-http` — Deterministic HTTP Fixture Server

`sayl-test-http` is an in-memory, deterministic local HTTP fixture server designed for SAYL integration and unit tests without external internet connectivity. It binds exclusively to the local loopback interface (`127.0.0.1:0`) and accurately simulates all 27 download failure modes and protocol edge cases specified in **MASTER-PLAN §8**.

---

## Features

- **Strictly Local Loopback**: Automatically binds to an ephemeral port on `127.0.0.1:0` (`start() -> Result<SocketAddr>`).
- **All 27 Failure Modes**: Full simulation of RFC 7233 partial content, corrupted headers, TCP resets, connection drops, rate limiting, and origin concurrency limits.
- **Deterministic Body Generation**: Sequential byte generator (`0x00..=0xFF` repeated) with pattern verification, offset slicing, and reference fixture compatibility (`fixtures/sample.bin`).
- **Programmable Throttling & Streaming**: Fine-grained artificial delays (`set_delay`) and configurable chunk streaming sizes (`set_chunk_size`).
- **Request Auditing**: In-memory request recording (`record_request`) and declarative sequence verification (`verify_requests`).
- **Clean Tokio Async Lifecycle**: Graceful shutdown and cancellation via `shutdown()`.

---

## Module Overview

| Module | Description |
|---|---|
| [`server`](src/server.rs) | `HttpFixtureServer` providing HTTP/1.1 parsing, scenario response dispatch, request audit log, and connection lifecycle management. |
| [`scenarios`](src/scenarios.rs) | Enum of all 27 failure modes (`Scenario`), aliases, human-readable descriptions, and dynamic configuration (`ScenarioConfig`). |
| [`body_gen`](src/body_gen.rs) | Deterministic sequential byte generator (`generate_bytes`, `generate_bytes_offset`), range slicing (`BodyGenerator`), and pattern checker (`verify_pattern`). |
| [`lib`](src/lib.rs) | Top-level re-exports of primary APIs and types. |

---

## All 27 Failure Modes (MASTER-PLAN §8)

| # | Scenario Identifier | Status Code | Key Headers & Behavior | Description |
|---|---|---|---|---|
| 1 | `Ok206` | `206 Partial Content` | `Content-Range: bytes S-E/T`, `Accept-Ranges: bytes` | Standard valid partial content response for valid byte range requests. |
| 2 | `NoAcceptRanges` | `200 OK` | Omits `Accept-Ranges` header | Ignores client `Range` request and serves full resource. |
| 3 | `IgnoredRange200` | `200 OK` | Omits `Content-Range` header | Returns full 200 OK body despite an explicit `Range` header. |
| 4 | `BadContentRange` | `206 Partial Content` | Corrupted `Content-Range: bytes */100` | Malformed or unparseable Content-Range header. |
| 5 | `BodyTruncation` | `200 OK` | Shorter body than declared `Content-Length` | Abruptly closes stream halfway through transmission. |
| 6 | `ConnectionReset` | N/A (TCP RST) | Immediate socket closure | Immediate TCP connection reset without valid HTTP response. |
| 7 | `ETagChanged` | `200` / `206` | ETag `v1` on 1st request, `v2` on subsequent | ETag changes between probe and subsequent ranged chunks. |
| 8 | `WeakETagOnly` | `200` / `206` | `ETag: W/"..."` | Weak validator returned without strong validator. |
| 9 | `NoValidator` | `200` / `206` | Neither `ETag` nor `Last-Modified` | Missing cache validators. |
| 10 | `416RangeNotSatisfiable` | `416 Range Not Satisfiable` | `Content-Range: bytes */T` | Requested range is beyond resource boundaries. |
| 11 | `HeadRejected` | `405 Method Not Allowed` | `Allow: GET` for `HEAD` requests | Rejects initial probe `HEAD` request. |
| 12 | `UnknownSize` | `HTTP/1.0 200 OK` | No `Content-Length`, no chunked encoding | Stream closes upon EOF (HTTP/1.0 close-delimited). |
| 13 | `EmptyFile` | `200 OK` | `Content-Length: 0` | 0-byte resource response. |
| 14 | `UnexpectedEncoding` | `200 OK` | `Content-Encoding: gzip` | Unexpected compression header when identity requested. |
| 15 | `429RetryAfter` | `429 Too Many Requests` | `Retry-After: 5` | Rate limiting response indicating client backoff time. |
| 16 | `RedirectWithSecrets` | `302 Found` | `Location: http://...` | Cross-domain redirect testing secret leak prevention. |
| 17 | `NoSpace` | `507 Insufficient Storage` | Storage exhaustion simulation | Simulated storage space failure response. |
| 18 | `PermissionDenied` | `403 Forbidden` | Access control simulation | Simulated permission denial. |
| 19 | `MaliciousFilename` | `200 OK` | Traversal path & Arabic Unicode in `Content-Disposition` | Path traversal (`../../etc/passwd`) and UTF-8 filename testing. |
| 20 | `PauseCancelDuringProbe` | N/A (Timeout) | Stalls response stream | Tests client probe cancellation and pause transitions. |
| 21 | `SlowWorker` | `200 OK` | Small chunks with inter-chunk delay | Simulates slow or throttled remote worker. |
| 22 | `OldLease` | `412 Precondition Failed` | Lease expiration response | Expired chunk lease simulation. |
| 23 | `RecoveryAtFinalize` | `200` -> `206` | Drops near end on 1st req, resumes on 2nd | Tests crash/recovery near finalization. |
| 24 | `ProxyTLS` | `407 Proxy Auth Required` | `Proxy-Authenticate: Basic`, `Via: 1.1 sayl-proxy` | Proxy challenge and proxy header handling. |
| 25 | `LimitsOrigin` | `503 Service Unavailable` | `Retry-After: 10` on concurrency > 1 | Exceeded origin concurrency limit. |
| 26 | `FileTruncation` | `206` / `416` | Shrinks total size on resume request | Simulates remote file truncation during active download. |
| 27 | `Pooling` | `200 OK` | `Connection: keep-alive` | Persistent connection reuse across sequential requests. |

---

## Usage Examples

### 1. Basic Server Setup with Range Support

```rust
use sayl_test_http::{HttpFixtureServer, Scenario, ExpectedRequest};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    server.set_resource_size(262_144); // 256 KiB
    server.set_chunk_size(4096);       // 4 KiB chunks
    server.set_delay(Duration::from_millis(10));

    let addr = server.start().await?;
    println!("Server running at http://{}/", addr);

    // Perform downloads via sayl client ...

    // Verify recorded requests
    server.verify_requests(&[
        ExpectedRequest::head("/sample.bin"),
        ExpectedRequest::get("/sample.bin").with_range("bytes=0-4095"),
    ]).await?;

    server.shutdown();
    Ok(())
}
```

### 2. Testing Rate Limiting (429 Retry-After)

```rust
use sayl_test_http::{HttpFixtureServer, Scenario};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = HttpFixtureServer::with_scenario(Scenario::RetryAfter429);
    let addr = server.start().await?;

    // Client requests receive HTTP 429 with Retry-After: 5
    // Client should back off accordingly.

    server.shutdown();
    Ok(())
}
```

### 3. Deterministic Body Verification

```rust
use sayl_test_http::{BodyGenerator, verify_pattern, generate_bytes};

fn test_data_integrity() {
    let gen = BodyGenerator::new(1024);
    let chunk = gen.slice_http_range(100, 199);
    assert_eq!(chunk.len(), 100);

    // Verify pattern matching
    assert!(verify_pattern(&chunk, 100));
}
```

---

## Toolchain & Verification Notes

- **Type Checking**: Validated using `cargo check -p sayl-test-http` and `cargo check --tests -p sayl-test-http`.
- **MSVC Toolchain Linker Blocker**: On Windows systems without the Visual C++ build tools (`link.exe`), compiling test runner binaries via `cargo test` is blocked by the linker. All structural contracts, module logic, and unit tests compile and type-check cleanly without linker dependencies.
