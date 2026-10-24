# sayl-http

HTTP transport client, multi-chunk range downloads, capability probing, safe redirect enforcement, and sequential streaming for the SAYL Download Manager.

Part of the **SAYL** project (High-performance Windows-first download manager in Rust).

---

## Architecture and Subsystems

`sayl-http` implements the transport layer defined in `docs/MASTER-PLAN.md` §5–§7 and `docs/tasks/HTTP-001.md`:

- **`error.rs` (`HttpError`)**:
  Classifies failures into transient/retryable (e.g. 429, 503, connection drops, body truncation), fatal (401, 403, 404, validation errors), and cancellation. Automatically maps to `sayl_core::CoreError`.

- **`redirect.rs` (`RedirectPolicy`, `RedirectTracker`)**:
  - Maximum 5 hops by default (`DEFAULT_MAX_REDIRECT_HOPS`).
  - Strips `Authorization`, `Cookie`, and `Proxy-Authorization` on cross-origin hops (scheme + host + port).
  - Enforces strict scheme whitelist: permits only `http` and `https`, rejecting dangerous schemes such as `file:`, `data:`, `ftp:`.
  - Loop detection via visited URL set.
  - Exposes the final canonical resolved URL.

- **`range.rs` (`RangeSupport`, `ContentRange`)**:
  - Classifies capability into `Supported`, `Ignored`, `Unsupported`, and `Unknown`.
  - Actively verifies range support using `Range: bytes=0-0` subrequests.
  - Strict validation of status 206 and `Content-Range: bytes start-end/total`.
  - Automatically downgrades capability to `Ignored` (if origin returned 200) or `Unsupported` on mismatch or malformed headers.
  - Treats 416 Range Not Satisfiable as unsupported/empty-file, never as a fatal error.

- **`filename.rs` (`derive_filename`, `sanitize_filename`)**:
  - Extracts filenames from `Content-Disposition` (with RFC 5987 / RFC 6266 `filename*=UTF-8''...` support), falling back to URL path segments, then default fallback.
  - Neutralizes directory traversal (`..`) and path separators (`/`, `\`).
  - Replaces Windows-reserved characters (`<>:"/\|?*`) and ASCII control characters with `_`.
  - Neutralizes Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) by prefixing `_` while preserving extensions (e.g. `CON.txt` -> `_CON.txt`).
  - Preserves file extensions and caps total length to 255 characters.
  - Full Unicode support with NFC normalization, ensuring Arabic filenames (e.g. `تقرير_سيل.pdf`) survive intact.

- **`probe.rs` (`probe`, `ProbeResult`)**:
  - Executes HEAD request first; falls back to ranged GET (`Range: bytes=0-0`) when HEAD is rejected (405/501) or omits `Content-Length`.
  - Validates and rejects negative or malformed `Content-Length`.
  - Returns `ProbeResult` with `final_url`, `status`, `content_length`, `accept_ranges`, `range_support`, `etag`, `last_modified`, `content_type`, and `filename`.

- **`stream.rs` (`stream_to_file`, `StreamOutcome`)**:
  - Incremental disk writing with configurable chunk buffers (default 64 KiB).
  - Verifies written bytes equal expected `Content-Length` before declaring success (prevents false complete).
  - Returns `StreamOutcome::Incomplete { written }` upon truncation or connection drop instead of a fatal error.
  - Honors cooperative cancellation via `CancellationToken`.
  - Executes `fsync` (`File::sync_all`) before declaring `StreamOutcome::Success`.

---

## Toolchain and Compilation

Per `docs/TOOLCHAIN.md`, on Windows environments without the Windows SDK, the project compiles using the GNU toolchain with `rust-lld`:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
cd "C:\Users\Dragonfly\Downloads\Download Manager"
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http
```

### Dependency Notes
`reqwest` is configured with `default-features = false, features = ["stream"]`. Avoiding `rustls-tls` / `ring` ensures 100% pure Rust HTTP compilation without external C compiler requirements (`gcc.exe` / MSVC BuildTools C runtime) on offline test systems.

---

## Integration Tests

Integration tests in `crates/http/tests/` verify scenarios against the local deterministic fixture server `sayl-test-http`:

1. `Ok206` — Range support detected as `Supported`, probe length matches total.
2. `NoAcceptRanges` — Server returns 200 without range headers, downgraded to `Ignored` / `Unsupported`.
3. `IgnoredRange200` — Server ignores `Range` header and sends 200 full body, correctly classified as `RangeSupport::Ignored`.
4. `BadContentRange` — Corrupted `Content-Range` header detected and downgraded to `Unsupported`.
5. `BodyTruncation` — Severed stream returns `StreamOutcome::Incomplete`, never false `Success`.
6. `HeadRejected` — Server returning 405/501 on HEAD falls back to ranged GET successfully.
7. `MaliciousFilename` — Neutralizes traversal (`..\..\evil.exe`), replaces reserved device names (`CON`, `NUL`), preserves Arabic characters.
8. `UnknownSize` — Close-delimited transfer without Content-Length handled safely.
9. `EmptyFile` — Content-Length: 0 handled cleanly without panics or false completions.
10. `RedirectSecurity` — Origin change strips credentials; invalid schemes (`file:`, `data:`) and redirect loops are rejected; max 5 hops enforced.
