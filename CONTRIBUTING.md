# Contributing to SAYL

Thank you for considering contributing to SAYL. This document describes the development setup, testing workflow, commit style, and PR checklist.

## Development Environment

### Toolchain (Windows)

The project uses the **GNU toolchain with `rust-lld`** for development and testing. The MSVC toolchain requires the Windows SDK (not installed in the default CI environment) and is reserved for production Tauri builds.

**Required:**
- Rust `stable-x86_64-pc-windows-gnu` toolchain
- `rust-lld` linker (bundled with the toolchain)

**Setup:**
```powershell
# Install the GNU toolchain
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy --toolchain stable-x86_64-pc-windows-gnu

# Set environment for the session
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:RUSTFLAGS = "-C linker=rust-lld"
```

### Verify Setup

```powershell
cd "<path-to-repo>"

# Type-check all crates
cargo +stable-x86_64-pc-windows-gnu check --workspace

# Run all tests
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

## Testing

### Deterministic HTTP Fixture Tests (No Network)

All HTTP integration tests run against the local `sayl-test-http` fixture server. **No external network access is required or used.**

```powershell
# Run test-http crate tests (fixture server itself)
cargo +stable-x86_64-pc-windows-gnu test -p sayl-test-http

# Run HTTP client tests against fixture server
cargo +stable-x86_64-pc-windows-gnu test -p sayl-http

# Run all workspace tests
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

The fixture server (`sayl-test-http`) simulates 27 failure modes including:
- Valid 206 Partial Content
- Missing/ignored Range headers
- Corrupted Content-Range
- Body truncation / TCP reset
- ETag changes (strong/weak/missing)
- 416 Range Not Satisfiable
- HEAD rejection (405)
- Unknown size (HTTP/1.0 close-delimited)
- Empty files
- Unexpected Content-Encoding
- Rate limiting (429 Retry-After)
- Cross-origin redirects with secret stripping
- Storage/permission errors
- Malicious filenames (path traversal, Unicode)
- Probe cancellation, slow workers, lease expiry
- Crash recovery near finalize
- Proxy authentication
- Origin concurrency limits
- Remote file truncation
- Connection pooling

### Test Commands Summary

| Command | Purpose |
|---------|---------|
| `cargo +stable-x86_64-pc-windows-gnu check -p <crate>` | Type-check a specific crate |
| `cargo +stable-x86_64-pc-windows-gnu test -p sayl-core` | Core model/state/event tests (33) |
| `cargo +stable-x86_64-pc-windows-gnu test -p sayl-test-http` | Fixture server tests (25) |
| `cargo +stable-x86_64-pc-windows-gnu test -p sayl-http` | HTTP client integration tests (65) |
| `cargo +stable-x86_64-pc-windows-gnu test -p sayl-storage` | Storage WAL/snapshot tests (18) |
| `cargo +stable-x86_64-pc-windows-gnu test --workspace` | All tests |

## Code Style

### Formatting & Linting

```powershell
# Check formatting (runs in CI)
cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check

# Check clippy lints (runs in CI, -D warnings = deny)
cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
```

### Rust Conventions

- Use `rustfmt` (default config)
- Clippy warnings are errors (`-D warnings`)
- Prefer explicit types in public APIs
- Document public items with `///` comments
- Use `anyhow` for application errors, `thiserror` for library errors
- No `unwrap()`/`expect()` in production paths — handle `Result` properly

## Commit Style

**All commit messages must be in English, imperative mood.**

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code restructuring without behavior change |
| `perf` | Performance improvement |
| `test` | Adding or modifying tests |
| `docs` | Documentation only |
| `chore` | Maintenance, tooling, dependencies |
| `build` | Build system changes |
| `ci` | CI configuration changes |

### Examples

```
feat(http): add range validation for 206 responses

Implement active range probing via HEAD then GET bytes=0-0.
Classify server capability as Supported/Ignored/Unsupported/Unknown.

Closes #123
```

```
fix(storage): handle truncated journal tail on recovery

Ignore incomplete last line in journal.jsonl during replay.
Log warning with byte offset.

Fixes #456
```

```
test(core): add property tests for state machine transitions

Use proptest with saved seeds to verify:
- Valid boundaries
- Single active owner per byte
- Durable coverage never exceeds written
```

### Scope

Use the crate or subsystem name: `core`, `http`, `test-http`, `storage`, `app`, `cli`, `media`, `torrent`, `plugin-host`, `desktop`, `website`, `ci`, `docs`.

## Git Rules

### No Destructive Git Operations

**Never** use the following without explicit owner approval:
- `git reset --hard`
- `git checkout -- <file>` (discarding uncommitted changes)
- `git push --force` / `git push --force-with-lease`
- `git rebase` on shared branches
- Deleting branches remotely

Local commits only. Push/Publish requires owner approval.

### Branch Naming

- `feat/<short-description>`
- `fix/<short-description>`
- `refactor/<short-description>`
- `test/<short-description>`
- `docs/<short-description>`

## Pull Request Checklist

Before opening a PR, ensure:

- [ ] **Branch** targets `main` (or the current development branch)
- [ ] **Commits** follow the commit style (English, imperative, typed)
- [ ] **Formatting** passes: `cargo fmt --all -- --check`
- [ ] **Clippy** passes: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] **Tests** pass locally: `cargo test --workspace` (GNU toolchain)
- [ ] **No network calls** in tests — all HTTP tests use `sayl-test-http`
- [ ] **No external dependencies** added without justification (avoid C deps, prefer pure Rust)
- [ ] **Documentation** updated if public API changed (README, crate docs, architecture docs if architectural)
- [ ] **No secrets** in code, logs, or commit history
- [ ] **No telemetry/tracking** code added
- [ ] **Accessibility** considered for UI changes (WCAG AA, RTL, focus rings)
- [ ] **Privacy** impact assessed — no new data collection or external connections

### PR Description Template

```markdown
## Summary
Brief description of the change.

## Type
feat / fix / refactor / perf / test / docs / chore / build / ci

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass (sayl-test-http)
- [ ] Manual verification steps (if applicable)

## Checklist
- [ ] fmt / clippy / test pass
- [ ] No destructive git history changes
- [ ] No secrets
- [ ] Docs updated
```

## Architecture Guidelines

### Crate Boundaries

- `sayl-core` — **No I/O**. Pure domain models, state machine, errors, events, idempotency. Zero dependencies on network/storage.
- `sayl-storage` — **Only crate with file I/O**. WAL, snapshots, directory locking. No network.
- `sayl-http` — **Only crate with network I/O**. HTTP client, range validation, redirects, filenames. Uses `reqwest` (stream feature only, no `ring`/`rustls-tls`).
- `sayl-test-http` — **Test-only**. Local loopback fixture server. No external deps beyond Tokio.
- `sayl-app` — Application services, Tauri commands/events. Coordinates core/storage/http.
- `apps/desktop` — Tauri + Svelte UI. **No direct DB/shell access**. Uses Tauri commands only.

### Safety Invariants

- Ranges are `[start, end_exclusive)` internally, `u64` with checked arithmetic
- Writer actor owns file + bounded queue; every write carries segment ID + lease generation
- Checkpoint: `write_all` → writer barrier → `fsync` → SQLite/JSON transaction → commit
- No silent overwrite, no false completion on truncated bodies
- Strong ETag + If-Range for safe resume/parallel; weak ETag never used in If-Range

## Questions?

Open a [GitHub Issue](https://github.com/SAYL/SAYL/issues) with the `question` label, or check existing discussions.

---

*This document is part of the SAYL project. See [README.md](README.md) for project overview.*